#!/usr/bin/env node
// DBC 3.0 relaunch — EVM (Frontier) contract-state exporter.
//
// Exports EVM CONTRACT CODE + STORAGE from the old mainnet so DBCSwap / tokens
// / staking contracts keep working on the new chain.
//
// IMPORTANT — no balances here (by design, avoids double-counting):
//   The old chain configures pallet_evm with
//       AddressMapping = HashedAddressMapping<BlakeTwo256>,  Currency = Balances
//   so an EVM address's DBC balance IS the balance of its mapped Substrate
//   AccountId (blake2_256("evm:"|H160)) in system.account — already captured by
//   export-balances.mjs. As long as the NEW chain uses the same mapping, seeding
//   those Substrate balances reproduces every EVM balance. So this exporter emits
//   ONLY code + storage (+ nonce for contracts, for CREATE-address correctness).
//
// pallet_evm storage read:
//   evm.accountCodes(H160)            -> contract bytecode (empty for EOAs)
//   evm.accountStorages(H160, H256)   -> storage slot value (double map)
//   evm.accountCodesMetadata(H160)    -> { size, hash }  (optional)
//   nonce: system.account(map(H160)).nonce
//
// Usage:
//   node export-evm-state.mjs [--endpoint wss://rpc.dbcwallet.io] [--block finalized]
//                             [--out evm-state.json] [--count-only]
//
// --count-only: enumerate contracts + total storage-slot count without dumping
//               (fast sanity pass; the full dump can be large).

import { ApiPromise, WsProvider } from '@polkadot/api'
import { blake2AsU8a } from '@polkadot/util-crypto'
import { u8aConcat, u8aToU8a, hexToU8a } from '@polkadot/util'

function arg(name, def) {
  const i = process.argv.indexOf(`--${name}`)
  if (i > -1 && (name === 'count-only')) return true
  return i > -1 && process.argv[i + 1] ? process.argv[i + 1] : def
}
const ENDPOINT = arg('endpoint', 'wss://rpc.dbcwallet.io')
const BLOCK = arg('block', 'finalized')
const OUT = arg('out', 'evm-state.json')
const COUNT_ONLY = process.argv.includes('--count-only')

// Frontier HashedAddressMapping<BlakeTwo256>: AccountId32 = blake2_256("evm:" ++ H160)
function h160ToAccountId(h160Hex) {
  const prefix = new TextEncoder().encode('evm:')
  const addr = hexToU8a(h160Hex)
  return blake2AsU8a(u8aConcat(prefix, addr), 256)
}

async function main() {
  console.error(`[export-evm] connecting to ${ENDPOINT} ...`)
  const api = await ApiPromise.create({ provider: new WsProvider(ENDPOINT) })

  let blockHash
  if (BLOCK === 'finalized') blockHash = await api.rpc.chain.getFinalizedHead()
  else if (BLOCK === 'latest') blockHash = (await api.rpc.chain.getHeader()).hash
  else if (/^0x[0-9a-fA-F]{64}$/.test(BLOCK)) blockHash = BLOCK
  else blockHash = await api.rpc.chain.getBlockHash(Number(BLOCK))
  const at = await api.at(blockHash)
  const blockNumber = (await api.rpc.chain.getHeader(blockHash)).number.toNumber()
  console.error(`[export-evm] block #${blockNumber} ${blockHash.toString()}`)

  // 1) enumerate contracts via accountCodes (non-empty code)
  console.error(`[export-evm] enumerating contracts (evm.accountCodes) ...`)
  const contracts = [] // { address, code, nonce }
  let startKey
  let scanned = 0
  for (;;) {
    const page = await at.query.evm.accountCodes.entriesPaged({ args: [], pageSize: 500, startKey })
    if (page.length === 0) break
    for (const [key, codeRaw] of page) {
      scanned++
      const addr = key.args[0].toHex()
      const code = codeRaw.toHex()
      if (code && code !== '0x') contracts.push({ address: addr, code })
    }
    startKey = page[page.length - 1][0].toString()
  }
  console.error(`[export-evm] accountCodes entries: ${scanned}, contracts with code: ${contracts.length}`)

  // 2) per-contract: nonce + storage slots
  let totalSlots = 0
  for (const c of contracts) {
    // nonce from mapped substrate account
    const acctId = h160ToAccountId(c.address)
    const info = await at.query.system.account(acctId)
    c.nonce = info.nonce.toNumber()

    if (COUNT_ONLY) {
      // just count storage slots
      let sk, n = 0
      for (;;) {
        const pg = await at.query.evm.accountStorages.entriesPaged({ args: [c.address], pageSize: 1000, startKey: sk })
        if (pg.length === 0) break
        n += pg.length
        sk = pg[pg.length - 1][0].toString()
      }
      c.slotCount = n
      totalSlots += n
    } else {
      const storage = []
      let sk
      for (;;) {
        const pg = await at.query.evm.accountStorages.entriesPaged({ args: [c.address], pageSize: 1000, startKey: sk })
        if (pg.length === 0) break
        for (const [k, v] of pg) storage.push([k.args[1].toHex(), v.toHex()])
        sk = pg[pg.length - 1][0].toString()
      }
      c.storage = storage
      totalSlots += storage.length
    }
    if (contracts.indexOf(c) % 25 === 0)
      console.error(`[export-evm]   ${contracts.indexOf(c) + 1}/${contracts.length} contracts, ${totalSlots} slots so far`)
  }

  const summary = {
    endpoint: ENDPOINT, block: { number: blockNumber, hash: blockHash.toString() },
    contractCount: contracts.length, totalStorageSlots: totalSlots,
    note: 'EVM balances NOT here — captured by export-balances.mjs via HashedAddressMapping<BlakeTwo256>. New chain must use the same mapping.',
  }
  console.error(`\n[export-evm] DONE  contracts=${contracts.length} storageSlots=${totalSlots}`)

  if (COUNT_ONLY) {
    console.error(JSON.stringify({ ...summary, top: contracts.map(c => ({ address: c.address, nonce: c.nonce, slotCount: c.slotCount })).sort((a, b) => b.slotCount - a.slotCount).slice(0, 15) }, null, 2))
  } else {
    const fs = await import('node:fs')
    fs.writeFileSync(OUT, JSON.stringify({ ...summary, contracts }, null, 0))
    console.error(`[export-evm] written ${OUT}`)
  }
  await api.disconnect()
}
main().catch((e) => { console.error('[export-evm] FATAL', e); process.exit(1) })
