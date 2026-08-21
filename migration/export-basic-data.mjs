#!/usr/bin/env node
// DBC 3.0 relaunch — basic-data exporter.
//
// Exports Sudo / Session (initial validators + their session keys) / Identity /
// Proxy / price-oracle seed from the old mainnet for the 3.0 genesis.
//
// Notes:
//   - NO Vesting pallet exists on DBC mainnet → nothing to migrate there.
//   - Multisig accounts are DETERMINISTIC (derived from signatories+threshold),
//     not stored; the `multisig.multisigs` storage is only in-flight pending
//     calls (transient) → intentionally not migrated. Multisig account balances
//     are already in the balance snapshot.
//   - Identity/proxy deposits are `reserved` → already in the balance snapshot;
//     on the new chain they unbond to free, and we re-seed the registrations.
//   - Session keys are exported so genesis can seed the same initial authorities.
//
// Usage: node export-basic-data.mjs [--endpoint ...] [--block finalized] [--out basic-data.json]

import { ApiPromise, WsProvider } from '@polkadot/api'
function arg(n,d){const i=process.argv.indexOf(`--${n}`);return i>-1&&process.argv[i+1]?process.argv[i+1]:d}
const ENDPOINT=arg('endpoint','wss://rpc.dbcwallet.io'), BLOCK=arg('block','finalized'), OUT=arg('out','basic-data.json')

async function entriesJSON(at,pallet,item){
  const out=[]; let sk
  for(;;){ const pg=await at.query[pallet][item].entriesPaged({args:[],pageSize:500,startKey:sk}); if(!pg.length)break
    for(const [k,v] of pg) out.push([k.args.map(a=>a.toJSON()), v.toJSON()]); sk=pg[pg.length-1][0].toString() }
  return out
}

async function main(){
  console.error(`[basic] connecting ${ENDPOINT}`)
  const api=await ApiPromise.create({provider:new WsProvider(ENDPOINT)})
  let bh; if(BLOCK==='finalized')bh=await api.rpc.chain.getFinalizedHead()
  else if(/^0x[0-9a-fA-F]{64}$/.test(BLOCK))bh=BLOCK; else bh=await api.rpc.chain.getBlockHash(Number(BLOCK))
  const at=await api.at(bh); const bn=(await api.rpc.chain.getHeader(bh)).number.toNumber()
  console.error(`[basic] block #${bn}`)

  const d={}
  // Sudo
  d.sudo={ key:(await at.query.sudo.key()).toJSON() }

  // Session — initial validators + their session keys (queuedKeys = next-era authorities)
  d.session={
    validators:(await at.query.session.validators()).toJSON(),
    queuedKeys:(await at.query.session.queuedKeys()).toJSON(),   // [[validator, keys]]
    currentIndex:(await at.query.session.currentIndex()).toJSON(),
  }
  // also snapshot Staking active validator set (for reference; bonds NOT migrated — become free balance)
  try { d.staking={ validatorCount:(await at.query.staking.validatorCount()).toJSON(), currentEra:(await at.query.staking.currentEra()).toJSON() } } catch(e){}

  // Identity
  d.identity={
    identityOf:await entriesJSON(at,'identity','identityOf'),
    subsOf:await entriesJSON(at,'identity','subsOf'),
    superOf:await entriesJSON(at,'identity','superOf'),
    registrars:(await at.query.identity.registrars()).toJSON(),
  }
  // Proxy
  d.proxy={
    proxies:await entriesJSON(at,'proxy','proxies'),        // [[account, [ [proxies], deposit ]]]
    announcements:await entriesJSON(at,'proxy','announcements'),
  }

  // Price oracle seed (best-effort — new chain can also re-feed). Export all non-map value items.
  d.oracle={}
  for(const p of ['dbcPriceOCW','dlcPriceOCW']){
    const q=api.query[p]; if(!q){continue}
    d.oracle[p]={}
    for(const item of Object.keys(q)){
      try{ const v=await at.query[p][item](); d.oracle[p][item]=v.toJSON() }catch(e){ /* map/needs-args, skip */ }
    }
  }

  const out={endpoint:ENDPOINT,block:{number:bn,hash:bh.toString()},vestingPallet:false,multisigNote:'deterministic accounts, not migrated (in-flight ops transient)',...d}
  const fs=await import('node:fs'); fs.writeFileSync(OUT,JSON.stringify(out,null,0))
  console.error(`[basic] DONE sudo=${d.sudo.key} validators=${(d.session.validators||[]).length} queuedKeys=${(d.session.queuedKeys||[]).length} identities=${d.identity.identityOf.length} proxies=${d.proxy.proxies.length} registrars=${(d.identity.registrars||[]).length}`)
  console.error(`[basic] written ${OUT}`)
  await api.disconnect()
}
main().catch(e=>{console.error('[basic] FATAL',e);process.exit(1)})
