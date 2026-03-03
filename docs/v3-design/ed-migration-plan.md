# ED Migration Plan (0 -> 1)

Date: March 3, 2026

## Decision

Adopt `ExistentialDeposit = 1` in runtime and remove `insecure_zero_ed`.

Rationale:
- `ED=0` is outside the upstream-tested path and keeps reintroducing edge cases.
- Recent staking benchmark/test fixes were needed only to keep `ED=0` behavior working.
- `ED=1` aligns with most Substrate runtimes and reduces long-term maintenance risk.

## Code Changes

1. Runtime config
- `runtime/src/lib.rs`: change `ExistentialDeposit` from `0` to `1`.
- `runtime/Cargo.toml`: remove `pallet-balances` feature `insecure_zero_ed`.

2. Existing ED compatibility patches
- Keep current staking fixes (they are ED-safe and do not conflict with `ED=1`):
  - `withdraw_unbonded_kill` and `reap_stash` reachability logic.
  - benchmark setup using `saturating_sub`.

3. Optional cleanup migration (recommended but not mandatory for correctness)
- Add a runtime migration to prune truly empty accounts:
  - target only accounts with zero balance and no refs/locks/holds/reserves.
  - do not touch any account with non-zero providers/consumers/sufficients.
- If account cardinality is large, run in bounded chunks across releases instead of one-shot.

## Rollout Sequence

1. Pre-upgrade checks (testnet)
- Build and run:
  - `cargo check -p dbc-runtime`
  - `cargo test -p pallet-staking --features runtime-benchmarks`
  - `cargo test -p dbc3-integration-tests`
- Run benchmark subset that failed under `ED=0` and confirm pass.

2. Runtime release N
- Flip ED and remove `insecure_zero_ed`.
- Publish runtime artifact and perform staged rollout (staging -> testnet -> mainnet).

3. Runtime release N+1 (optional)
- Execute bounded account-pruning migration if state-bloat warrants it.

## Risk Controls

- Main risk: accidental reaping of accounts that still have references.
- Mitigation:
  - conservative migration predicate (zero balances + zero refs only),
  - dry-run with `try-runtime` snapshot,
  - cap processed items per execution,
  - keep migration idempotent.

## Acceptance Criteria

- Runtime compiles without `insecure_zero_ed`.
- Staking benchmark paths pass (`withdraw_unbonded_kill`, `reap_stash`, `payout_stakers*`, `payout_all`).
- Integration tests pass (`dbc3-integration-tests`).
- No unexpected account loss in staging snapshot verification.
