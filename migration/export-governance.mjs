#!/usr/bin/env node
// DBC 3.0 relaunch — governance-state exporter.
//
// Exports Council / Technical Committee / Elections (phragmen) / Democracy /
// Treasury / CouncilReward state from the old mainnet for the 3.0 genesis.
//
// Notes:
//   - Treasury FUNDS live in the Treasury pallet account and are already in the
//     balance snapshot (export-balances.mjs). Here we export only pending
//     proposals/approvals + governance memberships.
//   - Election/council/identity deposits are `reserved` balances → already in
//     the balance snapshot; on the new chain everything unbonds to free, so
//     memberships are re-seeded but their deposits come back as spendable DBC.
//   - Values are emitted as decoded JSON (.toJSON()); genesis-critical lists
//     (members/prime/elected) are also given as plain ss58 arrays.
//
// Usage: node export-governance.mjs [--endpoint ...] [--block finalized] [--out governance.json]

import { ApiPromise, WsProvider } from '@polkadot/api'
function arg(n, d){const i=process.argv.indexOf(`--${n}`);return i>-1&&process.argv[i+1]?process.argv[i+1]:d}
const ENDPOINT=arg('endpoint','wss://rpc.dbcwallet.io'), BLOCK=arg('block','finalized'), OUT=arg('out','governance.json')

async function entriesJSON(at, pallet, item){
  const out=[]; let sk
  for(;;){ const pg=await at.query[pallet][item].entriesPaged({args:[],pageSize:500,startKey:sk}); if(!pg.length)break
    for(const [k,v] of pg) out.push([k.args.map(a=>a.toJSON()), v.toJSON()]); sk=pg[pg.length-1][0].toString() }
  return out
}

async function main(){
  console.error(`[gov] connecting ${ENDPOINT}`)
  const api=await ApiPromise.create({provider:new WsProvider(ENDPOINT)})
  let bh; if(BLOCK==='finalized')bh=await api.rpc.chain.getFinalizedHead()
  else if(/^0x[0-9a-fA-F]{64}$/.test(BLOCK))bh=BLOCK; else bh=await api.rpc.chain.getBlockHash(Number(BLOCK))
  const at=await api.at(bh); const bn=(await api.rpc.chain.getHeader(bh)).number.toNumber()
  console.error(`[gov] block #${bn}`)

  const g={}
  // Council
  g.council={
    members:(await at.query.council.members()).toJSON(),
    prime:(await at.query.council.prime()).toJSON(),
    proposalCount:(await at.query.council.proposalCount()).toJSON(),
    proposals:(await at.query.council.proposals()).toJSON(),
    proposalOf:await entriesJSON(at,'council','proposalOf'),
    voting:await entriesJSON(at,'council','voting'),
  }
  // Technical Committee
  g.technicalCommittee={
    members:(await at.query.technicalCommittee.members()).toJSON(),
    prime:(await at.query.technicalCommittee.prime()).toJSON(),
    proposalCount:(await at.query.technicalCommittee.proposalCount()).toJSON(),
    proposals:(await at.query.technicalCommittee.proposals()).toJSON(),
    proposalOf:await entriesJSON(at,'technicalCommittee','proposalOf'),
    voting:await entriesJSON(at,'technicalCommittee','voting'),
  }
  // Elections (phragmen)
  g.elections={
    members:(await at.query.elections.members()).toJSON(),       // [{who,stake,deposit}]
    runnersUp:(await at.query.elections.runnersUp()).toJSON(),
    candidates:(await at.query.elections.candidates()).toJSON(),
    electionRounds:(await at.query.elections.electionRounds()).toJSON(),
    voting:await entriesJSON(at,'elections','voting'),
  }
  // Democracy (largely transient — best-effort)
  g.democracy={
    publicPropCount:(await at.query.democracy.publicPropCount()).toJSON(),
    referendumCount:(await at.query.democracy.referendumCount()).toJSON(),
    lowestUnbaked:(await at.query.democracy.lowestUnbaked()).toJSON(),
    publicProps:(await at.query.democracy.publicProps()).toJSON(),
    nextExternal:(await at.query.democracy.nextExternal()).toJSON(),
    referendumInfoOf:await entriesJSON(at,'democracy','referendumInfoOf'),
    depositOf:await entriesJSON(at,'democracy','depositOf'),
  }
  // Treasury (funds already in balance snapshot; export pending gov)
  g.treasury={
    proposalCount:(await at.query.treasury.proposalCount()).toJSON(),
    deactivated:(await at.query.treasury.deactivated()).toJSON(),
    approvals:(await at.query.treasury.approvals()).toJSON(),
    proposals:await entriesJSON(at,'treasury','proposals'),
  }
  g.councilReward={ treasury:(await at.query.councilReward.treasury?.() ?? {toJSON:()=>null}).toJSON?.() }

  // convenience ss58 arrays for genesis seeding
  g._genesisSeed={
    councilMembers:g.council.members,
    technicalCommitteeMembers:g.technicalCommittee.members,
    electedCouncil:(g.elections.members||[]).map(m=>Array.isArray(m)?m[0]:(m.who||m)),
  }

  const out={endpoint:ENDPOINT,block:{number:bn,hash:bh.toString()},...g}
  const fs=await import('node:fs'); fs.writeFileSync(OUT,JSON.stringify(out,null,0))
  console.error(`[gov] DONE council=${(g.council.members||[]).length} techComm=${(g.technicalCommittee.members||[]).length} electedCouncil=${g._genesisSeed.electedCouncil.length} activeReferenda=${g.democracy.referendumInfoOf.length} treasuryApprovals=${(g.treasury.approvals||[]).length}`)
  console.error(`[gov] written ${OUT}`)
  await api.disconnect()
}
main().catch(e=>{console.error('[gov] FATAL',e);process.exit(1)})
