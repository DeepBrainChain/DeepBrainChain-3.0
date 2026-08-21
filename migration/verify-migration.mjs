#!/usr/bin/env node
// DBC 3.0 relaunch — dry-run verification. Waits for the test node (launched
// from the raw genesis) to come up, then checks migrated data matches the
// snapshot: block production, DBC balances, DLC(asset 88) balances, council,
// EVM contract code (DBCSwap).
// Usage: node verify-migration.mjs --rpc ws://127.0.0.1:9966 [--balances ..] [--assets ..] [--evm ..] [--gov ..]

import { ApiPromise, WsProvider } from '@polkadot/api'
import fs from 'node:fs'
function arg(n,d){const i=process.argv.indexOf(`--${n}`);return i>-1&&process.argv[i+1]?process.argv[i+1]:d}
const RPC=arg('rpc','ws://127.0.0.1:9966')
const F={balances:arg('balances','dbc-balances-snapshot.json'),assets:arg('assets','dbc-assets.json'),
  evm:arg('evm','dbc-evm-state.json'),gov:arg('gov','dbc-governance.json')}
const readJSON=p=>fs.existsSync(p)?JSON.parse(fs.readFileSync(p,'utf8')):null
const sleep=ms=>new Promise(r=>setTimeout(r,ms))

async function main(){
  // wait for node RPC
  let api
  for(let i=0;i<120;i++){
    try{ api=await ApiPromise.create({provider:new WsProvider(RPC)}); await api.isReadyOrError; break }
    catch(e){ if(api){try{await api.disconnect()}catch{}} console.error(`[verify] waiting for node... (${i})`); await sleep(5000) }
  }
  if(!api){ console.error('[verify] node never came up'); process.exit(1) }
  console.error('[verify] connected')

  let pass=0,fail=0
  const ok=(c,m)=>{ console.log(`${c?'✅':'❌'} ${m}`); c?pass++:fail++ }

  // 1) block production
  const h1=(await api.rpc.chain.getHeader()).number.toNumber()
  await sleep(4000)
  const h2=(await api.rpc.chain.getHeader()).number.toNumber()
  ok(h2>h1, `block production: #${h1} -> #${h2} in ~4s (1s blocks => expect ~+3/4)`)

  // 2) DBC balances (System.Account) — sample from snapshot
  const bal=readJSON(F.balances)
  if(bal){
    // pick 3 richest-ish (non-zero) samples deterministically
    const samples=bal.balances.slice(0,3)
    for(const [addr,amt] of samples){
      const acc=await api.query.system.account(addr)
      const onchain=acc.data.free.toBigInt()+acc.data.reserved.toBigInt()
      ok(onchain.toString()===amt, `DBC balance ${addr.slice(0,10)}… on-chain=${onchain} snapshot=${amt}`)
    }
  }
  // 3) DLC (asset 88) balances — sample from assets export
  const assets=readJSON(F.assets)
  if(assets){
    const dlc=assets.account.filter(a=>a[0][0]===88).slice(0,3)
    for(const [args,val] of dlc){
      const a=await api.query.assets.account(88,args[1])
      const onchain=a.isSome?a.unwrap().balance.toBigInt().toString():'MISSING'
      ok(onchain===String(val.balance), `DLC balance ${args[1].slice(0,10)}… on-chain=${onchain} snapshot=${val.balance}`)
    }
    const info=await api.query.assets.asset(88)
    ok(info.isSome, `asset 88 (DLC) definition present, supply=${info.isSome?info.unwrap().supply.toString():'?'}`)
  }
  // 4) council
  const gov=readJSON(F.gov)
  if(gov){
    const cm=(await api.query.council.members()).toJSON()
    ok(cm.length===(gov.council.members||[]).length, `council members on-chain=${cm.length} snapshot=${(gov.council.members||[]).length}`)
  }
  // 5) EVM contract code (DBCSwap etc.)
  const evm=readJSON(F.evm)
  if(evm&&evm.contracts){
    // pick the contract with the most storage (likely a core one) + one more
    const sorted=[...evm.contracts].sort((a,b)=>(b.storage?.length||0)-(a.storage?.length||0))
    for(const c of [sorted[0],sorted[1]]){
      const code=(await api.query.evm.accountCodes(c.address)).toHex()
      ok(code===c.code && code!=='0x', `EVM contract ${c.address.slice(0,12)}… code present (${(code.length-2)/2} bytes) matches`)
    }
  }

  console.log(`\n[verify] RESULT: ${pass} passed, ${fail} failed`)
  await api.disconnect()
  process.exit(fail?1:0)
}
main().catch(e=>{console.error('[verify] FATAL',e);process.exit(1)})
