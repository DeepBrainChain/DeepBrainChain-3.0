#!/usr/bin/env node
// DBC 3.0 relaunch — genesis builder (P3).
//
// Merges the four migration exports into the new chain's genesis `config`
// (the `genesis.runtimeGenesis.config` object of a plain chain-spec) and
// reconciles Σ(all DBC placed) against the old chain's total issuance.
//
// Inputs (from the P2 exporters, all taken at the SAME snapshot block):
//   --balances  dbc-balances-snapshot.json   (export-balances.mjs)
//   --evm       dbc-evm-state.json           (export-evm-state.mjs, full dump)
//   --gov       dbc-governance.json          (export-governance.mjs)
//   --basic     dbc-basic-data.json          (export-basic-data.mjs)
//   --template  base chain-spec json (from `dbc-chain build-spec --chain <x>`)
//   --out       dbc3-genesis-spec.json
//
// EVM double-set avoidance (KEY):
//   pallet_evm uses HashedAddressMapping<BlakeTwo256> + Currency=Balances, so a
//   contract's DBC lives under its mapped Substrate account in system.account
//   (already in the balance snapshot). Frontier's evm genesis ALSO writes a
//   per-account `balance`. To avoid double-counting regardless of add-vs-set
//   semantics we put each CONTRACT's balance in `evm.accounts[H160].balance`
//   and EXCLUDE that contract's mapped ss58 from `balances.balances`.
//   EOAs (no code) are never listed under evm.accounts → their balance stays
//   in `balances` only. Result: every DBC is placed exactly once.
//
// Reconciliation: Σ(balances.balances) + Σ(evm.accounts.balance)  MUST equal
//   the old chain's balances.totalIssuance (from the balances export).

import fs from 'node:fs'
import { blake2AsU8a } from '@polkadot/util-crypto'
import { u8aConcat, hexToU8a, u8aToHex } from '@polkadot/util'
import { encodeAddress } from '@polkadot/keyring'

function arg(n,d){const i=process.argv.indexOf(`--${n}`);return i>-1&&process.argv[i+1]?process.argv[i+1]:d}
const F={balances:arg('balances','dbc-balances-snapshot.json'),evm:arg('evm','dbc-evm-state.json'),
  gov:arg('gov','dbc-governance.json'),basic:arg('basic','dbc-basic-data.json'),
  template:arg('template','devspec.json'),out:arg('out','dbc3-genesis-spec.json')}
const SS58=Number(arg('ss58','42'))

function h160ToSs58(h160Hex){
  const acc=blake2AsU8a(u8aConcat(new TextEncoder().encode('evm:'),hexToU8a(h160Hex)),256)
  return encodeAddress(acc,SS58)
}
const readJSON=p=>JSON.parse(fs.readFileSync(p,'utf8'))

async function main(){
  console.error('[genesis] loading exports ...')
  const bal=readJSON(F.balances)
  const gov=fs.existsSync(F.gov)?readJSON(F.gov):null
  const basic=fs.existsSync(F.basic)?readJSON(F.basic):null
  const evm=fs.existsSync(F.evm)?readJSON(F.evm):null
  const oldTI=BigInt(bal.onchainTotalIssuance)

  // 1) balance map ss58 -> BigInt
  const balMap=new Map()
  for(const [addr,amt] of bal.balances) balMap.set(addr,BigInt(amt))
  console.error(`[genesis] balances loaded: ${balMap.size} accounts, Σ=${bal.sumTotal}`)

  // 2) EVM accounts: pull contract balances out of balMap
  const evmAccounts={}
  let evmBalSum=0n, contractsWithBal=0
  if(evm && evm.contracts){
    for(const c of evm.contracts){
      const mapped=h160ToSs58(c.address)
      let b=0n
      if(balMap.has(mapped)){ b=balMap.get(mapped); balMap.delete(mapped); if(b>0n)contractsWithBal++ }
      evmBalSum+=b
      const storage={}
      for(const [slot,val] of (c.storage||[])) storage[slot]=val
      evmAccounts[c.address]={ nonce:'0x'+(c.nonce||0).toString(16), balance:'0x'+b.toString(16),
        code:c.code, storage }
    }
    console.error(`[genesis] evm: ${evm.contracts.length} contracts, ${contractsWithBal} with balance, Σevm=${evmBalSum}`)
  } else {
    console.error('[genesis] WARNING: no EVM dump provided — evm.accounts left empty (run export-evm-state full dump before final genesis)')
  }

  // 3) balances.balances = remaining (EOAs + non-contract accounts)
  const balancesArr=[]; let balSum=0n
  for(const [addr,amt] of balMap){ balancesArr.push([addr,amt.toString()]); balSum+=amt }

  // 4) reconcile
  const placed=balSum+evmBalSum
  const reconciles=placed===oldTI
  console.error(`[genesis] Σbalances=${balSum} + Σevm=${evmBalSum} = ${placed}`)
  console.error(`[genesis] old totalIssuance=${oldTI}  RECONCILES=${reconciles} (delta ${oldTI-placed})`)

  // 5) governance + basic-data → genesis sections
  const councilMembers=gov?.council?.members||[]
  const techMembers=gov?.technicalCommittee?.members||[]
  const electionsMembers=(gov?.elections?.members||[]).map(m=>Array.isArray(m)?[m[0],String(m[1]??m[2]??0)]:[m.who,String(m.stake||0)])
  const sudoKey=basic?.sudo?.key||null
  // session.keys: [[validator, validator, {keys}]] — from basic.session.queuedKeys
  const sessionKeys=(basic?.session?.queuedKeys||[]).map(([v,keys])=>[v,v,keys])

  // 6) assemble genesis config on top of the template
  const spec=readJSON(F.template)
  const cfg=spec.genesis.runtimeGenesis.config
  if(sudoKey) cfg.sudo={ key:sudoKey }
  cfg.council={ members:councilMembers }
  cfg.technicalCommittee={ members:techMembers }
  cfg.elections={ members:electionsMembers }
  if(sessionKeys.length) cfg.session={ keys:sessionKeys }
  // staking left to template defaults (bonds NOT migrated — validators re-bond;
  // initial authorities come from session.keys). Treasury funds arrive via its
  // pallet account already present in balancesArr.
  const hasEvm=Object.keys(evmAccounts).length>0
  // markers for the two huge sections — stream-written below (genesis exceeds
  // V8's ~512MB max string length, so JSON.stringify on the whole spec fails).
  cfg.balances='@@BAL@@'
  if(hasEvm) cfg.evm='@@EVM@@'

  spec.name='DBC 3.0'; spec.id='dbc3_mainnet'; spec.chainType='Live'
  spec._migration={ snapshotBlock:bal.block, oldTotalIssuance:oldTI.toString(),
    placed:placed.toString(), reconciles, counts:{ balances:balancesArr.length,
    evmContracts:Object.keys(evmAccounts).length, council:councilMembers.length,
    techComm:techMembers.length, elections:electionsMembers.length, validators:sessionKeys.length } }

  // stream-write: split the skeleton on the markers and stream the big arrays
  const skeleton=JSON.stringify(spec)
  const ws=fs.createWriteStream(F.out)
  const W=s=>new Promise(res=>{ if(ws.write(s)) res(); else ws.once('drain',res) })
  const [preB,restB]=skeleton.split('"@@BAL@@"')
  let mid=restB, post=''
  if(hasEvm){ const parts=restB.split('"@@EVM@@"'); mid=parts[0]; post=parts[1] }
  await W(preB)
  await W('{"balances":[')
  for(let i=0;i<balancesArr.length;i++) await W((i?',':'')+JSON.stringify(balancesArr[i]))
  await W(']}')
  if(hasEvm){
    await W(mid); await W('{"accounts":{')
    let f=true
    for(const h of Object.keys(evmAccounts)){ await W((f?'':',')+JSON.stringify(h)+':'+JSON.stringify(evmAccounts[h])); f=false }
    await W('}}'); await W(post)
  } else { await W(mid) }
  await new Promise(r=>ws.end(r))
  console.error(`\n[genesis] DONE  balances=${balancesArr.length} evmContracts=${Object.keys(evmAccounts).length} council=${councilMembers.length} techComm=${techMembers.length} elections=${electionsMembers.length} validators=${sessionKeys.length}`)
  console.error(`[genesis] RECONCILES=${reconciles}  written ${F.out}`)
  if(!reconciles) console.error('[genesis] ⚠️ Σ mismatch — do NOT use until resolved (missing EVM dump? unclaimed rewards? snapshot mismatch?)')
}
main().catch(e=>{console.error('[genesis] FATAL',e);process.exit(1)})
