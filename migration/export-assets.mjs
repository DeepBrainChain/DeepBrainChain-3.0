#!/usr/bin/env node
// DBC 3.0 relaunch — pallet_assets exporter (native on-chain tokens).
//
// CRITICAL: DBC mainnet has native pallet_assets tokens, incl. DeepLink Coin
// (DLC, asset 88) with ~1.78M holders. These are SEPARATE from native DBC
// (System.Account) and from EVM ERC20s — they must be migrated or 1.78M users
// lose their DLC. (Found by feng's due-diligence 2026-08-21.)
//
// Exports DECODED JSON (values re-encoded with the NEW runtime's assets types
// by the genesis builder — cross-SDK safe):
//   assets.asset(id)                 -> AssetDetails (owner/supply/minBalance/...)
//   assets.metadata(id)              -> {name,symbol,decimals,...}
//   assets.account((id,who))         -> {balance,status,reason} per holder  [BIG]
//   assets.approvals((id,owner,del)) -> allowances
//
// Run on .50 vs localhost node for the 1.78M-holder scale:
//   node export-assets.mjs --endpoint ws://127.0.0.1:9983 --block <n> --out dbc-assets.json

import { ApiPromise, WsProvider } from '@polkadot/api'
function arg(n,d){const i=process.argv.indexOf(`--${n}`);return i>-1&&process.argv[i+1]?process.argv[i+1]:d}
const ENDPOINT=arg('endpoint','wss://rpc.dbcwallet.io'),BLOCK=arg('block','finalized'),OUT=arg('out','dbc-assets.json')

async function pagedTuple(at,item){ // returns [[argsJSON...], valueJSON]
  const out=[]; let sk
  for(;;){ const pg=await at.query.assets[item].entriesPaged({args:[],pageSize:1000,startKey:sk}); if(!pg.length)break
    for(const [k,v] of pg) out.push([k.args.map(a=>a.toJSON()), v.toJSON()]); sk=pg[pg.length-1][0].toString()
    if(out.length%200000===0) console.error(`[assets] ${item}: ${out.length}`) }
  return out
}
async function main(){
  console.error(`[assets] connecting ${ENDPOINT}`)
  const api=await ApiPromise.create({provider:new WsProvider(ENDPOINT)})
  let bh; if(BLOCK==='finalized')bh=await api.rpc.chain.getFinalizedHead()
  else if(/^0x[0-9a-fA-F]{64}$/.test(BLOCK))bh=BLOCK; else bh=await api.rpc.chain.getBlockHash(Number(BLOCK))
  const at=await api.at(bh); const bn=(await api.rpc.chain.getHeader(bh)).number.toNumber()
  console.error(`[assets] block #${bn}`)

  const asset=await pagedTuple(at,'asset')
  const metadata=await pagedTuple(at,'metadata')
  console.error(`[assets] assets=${asset.length} metadata=${metadata.length}; scanning holders (big)...`)
  const account=await pagedTuple(at,'account')
  const approvals=await pagedTuple(at,'approvals')

  const out={endpoint:ENDPOINT,block:{number:bn,hash:bh.toString()},
    counts:{assets:asset.length,metadata:metadata.length,holders:account.length,approvals:approvals.length},
    asset,metadata,account,approvals}
  const fs=await import('node:fs'); fs.writeFileSync(OUT,JSON.stringify(out))
  console.error(`[assets] DONE assets=${asset.length} holders=${account.length} approvals=${approvals.length} -> ${OUT} (${(fs.statSync(OUT).size/1e6).toFixed(0)}MB)`)
  await api.disconnect()
}
main().catch(e=>{console.error('[assets] FATAL',e);process.exit(1)})
