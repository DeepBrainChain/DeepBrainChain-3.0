#!/usr/bin/env node
// DBC 3.0 relaunch — pallet_nfts exporter.
// DBC mainnet has 8 NFT collections / 933 items (user-held). Export the core
// storage (decoded JSON; genesis builder re-encodes with new runtime types).
//   collection, item(owner), collectionMetadataOf, itemMetadataOf,
//   collectionConfigOf, itemConfigOf, attribute, collectionRoleOf
// Usage: node export-nfts.mjs [--endpoint ..] [--block finalized] [--out dbc-nfts.json]

import { ApiPromise, WsProvider } from '@polkadot/api'
function arg(n,d){const i=process.argv.indexOf(`--${n}`);return i>-1&&process.argv[i+1]?process.argv[i+1]:d}
const ENDPOINT=arg('endpoint','wss://rpc.dbcwallet.io'),BLOCK=arg('block','finalized'),OUT=arg('out','dbc-nfts.json')

async function main(){
  console.error(`[nfts] connecting ${ENDPOINT}`)
  const api=await ApiPromise.create({provider:new WsProvider(ENDPOINT)})
  let bh; if(BLOCK==='finalized')bh=await api.rpc.chain.getFinalizedHead()
  else if(/^0x[0-9a-fA-F]{64}$/.test(BLOCK))bh=BLOCK; else bh=await api.rpc.chain.getBlockHash(Number(BLOCK))
  const at=await api.at(bh); const bn=(await api.rpc.chain.getHeader(bh)).number.toNumber()
  console.error(`[nfts] block #${bn}`)
  const q=api.query.nfts
  const items=['collection','item','collectionMetadataOf','itemMetadataOf','collectionConfigOf','itemConfigOf','attribute','collectionRoleOf','collectionAccount','account']
  const d={}
  for(const it of items){
    if(!q[it]){ continue }
    const out=[]; let sk
    try{ for(;;){ const pg=await at.query.nfts[it].entriesPaged({args:[],pageSize:1000,startKey:sk}); if(!pg.length)break
      for(const [k,v] of pg) out.push([k.args.map(a=>a.toJSON()), v.toJSON()]); sk=pg[pg.length-1][0].toString() } }catch(e){ console.error(`[nfts] ${it} skip: ${e.message}`) }
    d[it]=out
  }
  const out={endpoint:ENDPOINT,block:{number:bn,hash:bh.toString()},
    counts:Object.fromEntries(Object.entries(d).map(([k,v])=>[k,v.length])),...d}
  const fs=await import('node:fs'); fs.writeFileSync(OUT,JSON.stringify(out))
  console.error(`[nfts] DONE collections=${(d.collection||[]).length} items=${(d.item||[]).length} -> ${OUT}`)
  await api.disconnect()
}
main().catch(e=>{console.error('[nfts] FATAL',e);process.exit(1)})
