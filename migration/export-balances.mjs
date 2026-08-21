#!/usr/bin/env node
// DBC 3.0 relaunch — native balance snapshot exporter.
//
// Scans the OLD DBC mainnet (Substrate, spec 41x, ss58 prefix 42) at a chosen
// block and exports each account's TOTAL DBC (free + reserved) so it can be
// seeded as spendable `free` balance in the new 3.0 genesis.
//
// Why free + reserved:
//   - `free` already INCLUDES locked/frozen funds (staking bonds, vesting, and
//     democracy/election locks are `locks`/`frozen` on top of `free`, NOT a
//     separate pot). So `free` captures a bonded validator/nominator's stake.
//   - `reserved` holds deposits (identity, proxy, multisig, council-election,
//     preimage, etc.) — real DBC the account owns → must be returned.
//   => total = free + reserved = every DBC the account controls. On the new
//      chain it all becomes spendable `free` (everything unbonds); miners/
//      validators then re-stake, per feng's scope.
//
// NOT captured here (handled separately / operationally):
//   - Unclaimed staking-era rewards (not in any balance until payout_stakers).
//     Plan: announce "claim all pending rewards before the snapshot block",
//     OR add a rewards-computation pass. This tool exports on-chain balances only.
//   - EVM (H160) balances / contract state — exported by the separate EVM-state
//     exporter. NOTE the EVM<->Substrate account relationship must be handled
//     there to avoid double-counting DBC (EVM balances are held under mapped
//     substrate accounts on the Frontier fork).
//
// Usage:
//   node export-balances.mjs [--endpoint wss://rpc.dbcwallet.io] [--block <hashOrNumber|finalized>] [--out balances.json]
//
// Output JSON: { endpoint, specVersion, block:{number,hash}, ss58Prefix,
//   tokenDecimals, totalAccounts, sumTotal (string), onchainTotalIssuance (string),
//   reconciles (bool), balances: [ [ss58Address, totalPlanck(string)], ... ] }

import { ApiPromise, WsProvider } from '@polkadot/api'

function arg(name, def) {
  const i = process.argv.indexOf(`--${name}`)
  return i > -1 && process.argv[i + 1] ? process.argv[i + 1] : def
}

const ENDPOINT = arg('endpoint', 'wss://rpc.dbcwallet.io')
const BLOCK = arg('block', 'finalized') // 'finalized' | 'latest' | block number | block hash
const OUT = arg('out', 'balances.json')

function fmt(planck, decimals) {
  const s = planck.toString().padStart(decimals + 1, '0')
  const i = s.slice(0, s.length - decimals)
  const f = s.slice(s.length - decimals).replace(/0+$/, '')
  return f ? `${i}.${f}` : i
}

async function main() {
  console.error(`[export-balances] connecting to ${ENDPOINT} ...`)
  const api = await ApiPromise.create({ provider: new WsProvider(ENDPOINT) })

  // Resolve target block hash
  let blockHash
  if (BLOCK === 'finalized') blockHash = await api.rpc.chain.getFinalizedHead()
  else if (BLOCK === 'latest') blockHash = (await api.rpc.chain.getHeader()).hash
  else if (/^0x[0-9a-fA-F]{64}$/.test(BLOCK)) blockHash = BLOCK
  else blockHash = await api.rpc.chain.getBlockHash(Number(BLOCK))

  const at = await api.at(blockHash)
  const header = await api.rpc.chain.getHeader(blockHash)
  const blockNumber = header.number.toNumber()
  const rt = await api.rpc.state.getRuntimeVersion(blockHash)
  const specVersion = rt.specVersion.toNumber()
  const ss58Prefix = api.registry.chainSS58 ?? 42
  const decimals = (api.registry.chainDecimals && api.registry.chainDecimals[0]) || 15

  console.error(`[export-balances] block #${blockNumber} ${blockHash.toString()} spec=${specVersion} ss58=${ss58Prefix} decimals=${decimals}`)
  console.error(`[export-balances] scanning system.account entries (paginated) ...`)

  const balances = []
  let sum = 0n
  let count = 0
  // entriesPaged to bound memory on a large account set
  const PAGE = 1000
  let startKey = undefined
  for (;;) {
    const page = await at.query.system.account.entriesPaged({ args: [], pageSize: PAGE, startKey })
    if (page.length === 0) break
    for (const [key, acc] of page) {
      const addr = key.args[0].toString() // ss58 (prefix from registry)
      const free = acc.data.free.toBigInt()
      const reserved = acc.data.reserved.toBigInt()
      const total = free + reserved
      if (total > 0n) {
        balances.push([addr, total.toString()])
        sum += total
      }
      count++
    }
    startKey = page[page.length - 1][0].toString()
    if (count % 20000 === 0) console.error(`[export-balances]   scanned ${count} accounts, running sum=${fmt(sum, decimals)} DBC`)
  }

  // Reconciliation vs on-chain total issuance
  const onchainTI = (await at.query.balances.totalIssuance()).toBigInt()
  const reconciles = sum === onchainTI

  const out = {
    endpoint: ENDPOINT,
    specVersion,
    block: { number: blockNumber, hash: blockHash.toString() },
    ss58Prefix,
    tokenDecimals: decimals,
    totalAccounts: count,
    accountsWithBalance: balances.length,
    sumTotal: sum.toString(),
    sumTotalDBC: fmt(sum, decimals),
    onchainTotalIssuance: onchainTI.toString(),
    onchainTotalIssuanceDBC: fmt(onchainTI, decimals),
    reconciles,
    reconcileDeltaPlanck: (onchainTI - sum).toString(),
    balances,
  }

  const fs = await import('node:fs')
  fs.writeFileSync(OUT, JSON.stringify(out, null, 0))
  console.error(`\n[export-balances] DONE`)
  console.error(`  accounts scanned:        ${count}`)
  console.error(`  accounts with balance:   ${balances.length}`)
  console.error(`  Σ(free+reserved):        ${out.sumTotalDBC} DBC (${sum} planck)`)
  console.error(`  on-chain totalIssuance:  ${out.onchainTotalIssuanceDBC} DBC (${onchainTI} planck)`)
  console.error(`  RECONCILES:              ${reconciles}  (delta ${out.reconcileDeltaPlanck} planck)`)
  console.error(`  written:                 ${OUT}`)
  if (!reconciles) console.error(`  ⚠️  sum != totalIssuance — investigate (dust below ED, on_initialize mint, etc.) before using for genesis`)
  await api.disconnect()
}

main().catch((e) => {
  console.error('[export-balances] FATAL', e)
  process.exit(1)
})
