#!/usr/bin/env node
// DBC 3.0 relaunch — RAW genesis constructor (P3, large-state path).
//
// build-spec --raw can't ingest ~2M accounts (in-WASM genesis builder blows the
// allocator). So build genesis.raw.top DIRECTLY: take a working small dev
// raw-spec as base (correct :code/system/babe/grandpa/session/etc.), strip the
// migrated pallets, splice in directly-computed SCALE storage kv. Node loads raw
// kv straight into the trie — no WASM build.
//
// IMPORTANT: the 1.96M-account + 2.6M-EVM-entry loop is CPU-heavy and blocks the
// node event loop, which kills a live WS connection (RPC timeout). So we connect
// to a 3.0 dev node ONLY to grab storage prefixes + encode the few gov/sudo
// values, then DISCONNECT and run the heavy loop fully OFFLINE (keys via
// blake2_128_concat, AccountInfo hand-encoded — both validated vs the runtime).
//
// dev node: dbc-chain --dev --tmp --rpc-port 9955 --rpc-external
// Inputs: --base dev-raw.json --balances .. --evm .. --gov .. --basic ..
//         --rpc ws://127.0.0.1:9955 --out dbc3-raw-genesis.json
//         --keep-dev-authorities (test: keep base dev accounts so test node authors)

import { ApiPromise, WsProvider } from '@polkadot/api'
import fs from 'node:fs'
import { blake2AsU8a, decodeAddress } from '@polkadot/util-crypto'
import { u8aConcat, hexToU8a, u8aToHex } from '@polkadot/util'

function arg(n,d){const i=process.argv.indexOf(`--${n}`);return i>-1&&process.argv[i+1]?process.argv[i+1]:d}
const F={base:arg('base','dev-raw.json'),balances:arg('balances','dbc-balances-snapshot.json'),
  evm:arg('evm','dbc-evm-state.json'),gov:arg('gov','dbc-governance.json'),basic:arg('basic','dbc-basic-data.json'),
  rpc:arg('rpc','ws://127.0.0.1:9955'),out:arg('out','dbc3-raw-genesis.json')}
const KEEP_DEV_AUTH=process.argv.includes('--keep-dev-authorities')
const readJSON=p=>JSON.parse(fs.readFileSync(p,'utf8'))
const stripHex=h=>h.startsWith('0x')?h.slice(2):h
const h160ToId=h=>blake2AsU8a(u8aConcat(new TextEncoder().encode('evm:'),hexToU8a(h)),256)
// Blake2_128Concat hasher: blake2_128(key) ++ key   (returns hex without 0x)
const b128c=u8a=>u8aToHex(blake2AsU8a(u8a,128)).slice(2)+u8aToHex(u8a).slice(2)

// AccountInfo hand-encode (validated == FrameSystemAccountInfo):
// nonce u32, consumers u32=0, providers u32=1, sufficients u32=0, data{ free u128, reserved 0, frozen 0, flags 0 }
const u32le=n=>{const b=Buffer.alloc(4);b.writeUInt32LE(n>>>0);return b}
const u128le=big=>{const b=Buffer.alloc(16);let x=BigInt(big);for(let i=0;i<16;i++){b[i]=Number(x&0xffn);x>>=8n}return b}
const accountInfoHex=(nonce,free)=>'0x'+Buffer.concat([u32le(nonce),u32le(0),u32le(1),u32le(0),u128le(free),u128le(0),u128le(0),u128le(0)]).toString('hex')

async function main(){
  console.error(`[raw] connecting ${F.rpc} (for prefixes + gov value encoding only)`)
  const api=await ApiPromise.create({provider:new WsProvider(F.rpc)})
  // validate hand-encoding once
  const chk=api.createType('FrameSystemAccountInfo',{nonce:7,consumers:0,providers:1,sufficients:0,data:{free:'123456789000000',reserved:0,frozen:0,flags:0}}).toHex()
  if(chk!==accountInfoHex(7,123456789000000n)){console.error('[raw] FATAL AccountInfo mismatch\n api:',chk,'\n mine:',accountInfoHex(7,123456789000000n));process.exit(1)}
  console.error('[raw] AccountInfo hand-encoding matches runtime ✓')

  // storage prefixes / keys (twox128 pallet+item) from metadata
  const P={ sysAcc:api.query.system.account.keyPrefix(), ti:api.query.balances.totalIssuance.key(),
    evmCodes:api.query.evm.accountCodes.keyPrefix(), evmStore:api.query.evm.accountStorages.keyPrefix(),
    council:api.query.council.members.key(), tech:api.query.technicalCommittee.members.key(),
    elections:api.query.elections.members.key(), sudo:api.query.sudo.key.key() }

  // encode the (small) gov/sudo VALUES while connected
  const gov=fs.existsSync(F.gov)?readJSON(F.gov):null
  const basic=fs.existsSync(F.basic)?readJSON(F.basic):null
  const govKV=[]
  if(gov){
    govKV.push([P.council, api.createType('Vec<AccountId32>',gov.council.members||[]).toHex()])
    govKV.push([P.tech, api.createType('Vec<AccountId32>',gov.technicalCommittee.members||[]).toHex()])
    try{ const em=(gov.elections.members||[]).map(m=>Array.isArray(m)?{who:m[0],stake:String(m[1]||0),deposit:'0'}:{who:m.who,stake:String(m.stake||0),deposit:String(m.deposit||0)})
      govKV.push([P.elections, api.createType('Vec<PalletElectionsPhragmenSeatHolder>',em).toHex()]) }
    catch(e){ console.error('[raw] WARN elections encode:',e.message) }
  }
  if(basic?.sudo?.key) govKV.push([P.sudo, api.createType('Option<AccountId32>',basic.sudo.key).toHex()])

  console.error('[raw] disconnecting api; heavy loop runs offline')
  await api.disconnect()

  // load base + strip overridden pallets
  const base=readJSON(F.base); const top=base.genesis.raw.top
  const bal=readJSON(F.balances)
  const evm=fs.existsSync(F.evm)?readJSON(F.evm):null
  const stripPrefixes=[...(KEEP_DEV_AUTH?[]:[P.sysAcc]),P.ti,P.evmCodes,P.evmStore,P.council,P.tech,P.elections,P.sudo]
  let removed=0
  for(const k of Object.keys(top)){ if(stripPrefixes.some(p=>k.startsWith(p))){delete top[k];removed++} }
  console.error(`[raw] stripped ${removed} base entries`)

  // stream-write raw spec
  base.name='DBC 3.0'; base.id='dbc3_mainnet'; base.chainType='Live'
  base._migration={snapshotBlock:bal.block,totalIssuance:bal.onchainTotalIssuance,keepDevAuthorities:KEEP_DEV_AUTH}
  const shell=JSON.parse(JSON.stringify(base)); shell.genesis.raw.top='@@TOP@@'
  const [pre,post]=JSON.stringify(shell).split('"@@TOP@@"')
  const ws=fs.createWriteStream(F.out)
  const W=s=>new Promise(r=>{if(ws.write(s))r();else ws.once('drain',r)})
  let first=true
  const emit=async(k,v)=>{ await W((first?'':',')+JSON.stringify(k)+':'+JSON.stringify(v)); first=false }
  await W(pre); await W('{')
  // kept base entries
  for(const k of Object.keys(top)) await emit(k,top[k])
  // System.Account for every snapshot account (contract-mapped incl. — balance lives here)
  let n=0
  for(const [addr,amt] of bal.balances){
    const key=P.sysAcc+b128c(decodeAddress(addr))
    await emit(key,accountInfoHex(0,BigInt(amt))); n++
    if(n%300000===0) console.error(`[raw]   accounts ${n}`)
  }
  await emit(P.ti,'0x'+u128le(BigInt(bal.onchainTotalIssuance)).toString('hex'))
  // gov/sudo
  for(const [k,v] of govKV) await emit(k,v)
  // EVM code + storage
  let ec=0,es=0
  if(evm?.contracts){
    for(const c of evm.contracts){
      const h=hexToU8a(c.address)
      // accountCodes value = Bytes (compact-len ++ code)
      const code=hexToU8a(c.code); const clen=compactLen(code.length)
      await emit(P.evmCodes+b128c(h), '0x'+clen+u8aToHex(code).slice(2)); ec++
      for(const [slot,val] of (c.storage||[])){ await emit(P.evmStore+b128c(h)+b128c(hexToU8a(slot)), val); es++ }
      if(ec%500===0) console.error(`[raw]   evm contracts ${ec}, slots ${es}`)
    }
  }
  await W('}'); await W(post); await new Promise(r=>ws.end(r))
  console.error(`\n[raw] DONE accounts=${n} evmContracts=${ec} evmSlots=${es} -> ${F.out} (${(fs.statSync(F.out).size/1e6).toFixed(0)}MB)`)
}
// SCALE compact length prefix (hex, no 0x)
function compactLen(len){
  if(len<64) return Buffer.from([len<<2]).toString('hex')
  if(len<16384) return Buffer.from([((len<<2)|1)&0xff,((len<<2)|1)>>8]).toString('hex')
  const b=Buffer.alloc(4); b.writeUInt32LE(((len<<2)|2)>>>0); return b.toString('hex')
}
main().catch(e=>{console.error('[raw] FATAL',e);process.exit(1)})
