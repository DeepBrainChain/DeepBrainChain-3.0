# DBC 3.0: Benchmark Verification + Economic Game Theory

## 1. Problem Statement

LLM inference is inherently non-deterministic — the same input can produce different outputs across runs. This makes traditional ZK (Zero-Knowledge) verification impractical: there is no fixed computation trace to prove. Unlike matrix multiplications (handled by `pallet-zk-compute`), LLM token generation cannot be verified through cryptographic proofs alone.

**Challenge**: How do we ensure miners honestly report their AI inference capabilities without deterministic verification?

## 2. Solution: Self-Report + Stake + Economic Game Theory

Inspired by DBC 2.0's proven patterns, we combine:

| Mechanism | DBC 2.0 Origin | DBC 3.0 Application |
|-----------|---------------|---------------------|
| **PendingSlash + Appeal Window** | maintain-committee (2-day review) | Complaint slash + `SlashGracePeriod` (600 blocks ~1hr) |
| **Reporter Stake** | maintain-committee reporter_stake | Challenge/complaint deposits prevent spam |
| **Burn + Reward Split** | rent-machine (30/70) | 50% burned + 50% reward to honest party |
| **Reputation System** | online-profile reputation | Pool reputation affected by complaints |
| **Escalating Penalties** | online-profile slash (duration-based) | Multiple complaints compound reputation loss |

### Core Principle

> The cost of cheating must exceed the benefit. Rational actors will report honestly when the expected penalty for dishonesty outweighs potential gains.

## 3. Feature 1: Benchmark Claims (`pallet-agent-attestation`)

### Flow

```
Miner registers node → submits benchmark claim (model + score + stake)
    → Active claim boosts scheduling priority
    → Anyone can challenge (with counter-stake)
        → AdminOrigin resolves:
            Guilty  → Miner loses stake (50% burn + 50% challenger reward)
            Innocent → Challenger loses stake (50% burn + 50% miner reward)
```

### Extrinsics

| # | Extrinsic | Caller | Description |
|---|-----------|--------|-------------|
| 7 | `submit_benchmark_claim` | Registered miner | Self-report `model_id` + `score` (tokens/sec), stake `BenchmarkDeposit` (500 DBC). Replaces old claim for same model (old stake refunded). |
| 8 | `challenge_benchmark` | Anyone (not self) | Challenge a claim by staking `BenchmarkChallengeDeposit` (200 DBC). |
| 9 | `resolve_benchmark` | AdminOrigin (root) | Resolve with `claim_is_valid: bool`. Slash loser, reward winner. |
| 10 | `update_benchmark_claim` | Claim owner | Update score for Active claim (no additional stake). |

### Storage

- `BenchmarkClaims`: `ClaimId → BenchmarkClaim` (claimer, model, score, deposit, status, challenger)
- `MinerBenchmark`: `(AccountId, ModelId) → ClaimId` (latest claim per miner per model)
- `MinerClaimCount`: `AccountId → u32` (limit: `MaxBenchmarkClaims = 10`)

### Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `BenchmarkDeposit` | 500 DBC | High enough to deter frivolous claims |
| `BenchmarkChallengeDeposit` | 200 DBC | Lower than claim deposit (encourage honest challenges) |
| `MaxBenchmarkClaims` | 10 | Per-miner limit prevents storage bloat |

### Cross-Pallet Integration

```rust
pub trait BenchmarkScoreProvider {
    type AccountId;
    fn get_benchmark_score(miner: &Self::AccountId, model_id: &[u8]) -> Option<u32>;
    fn has_slashed_claims(miner: &Self::AccountId) -> bool;
}
```

Implemented by `pallet-agent-attestation`, consumed by `pallet-compute-pool-scheduler` for pool scoring.

## 4. Feature 2: Complaint Mechanism (`pallet-compute-pool-scheduler`)

### Flow

```
User completes task → files complaint (with stake + reason)
    → AdminOrigin resolves:
        Valid   → PendingSlash created, complainant refunded
                → Pool owner has SlashGracePeriod to appeal
                    → No appeal → on_initialize auto-executes slash
                    → Appeal → AdminOrigin re-evaluates
        Invalid → Complainant stake slashed (50% burn + 50% pool)

    Cancel  → 90% refund, 10% burned (anti-spam penalty)
```

### Extrinsics

| # | Extrinsic | Caller | Description |
|---|-----------|--------|-------------|
| 10 | `file_complaint` | Task user | Complain about completed+verified task. Stake `ComplaintDeposit` (100 DBC). |
| 11 | `resolve_complaint` | AdminOrigin (root) | Resolve with `valid: bool`. |
| 12 | `cancel_complaint` | Complainant | Withdraw complaint. 90% refund, 10% burned. |
| 13 | `appeal_complaint` | Pool owner | Appeal within grace period. |

### Storage

- `Complaints`: `ComplaintId → Complaint` (complainant, pool, task, deposit, status, reason)
- `TaskComplaint`: `TaskId → ComplaintId` (one complaint per task)
- `PoolOpenComplaints`: `PoolId → u32` (limit: `MaxOpenComplaints = 10`)
- `PendingComplaintSlash`: `ComplaintId → (PoolId, Amount, ExecuteAfterBlock)`

### Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `ComplaintDeposit` | 100 DBC | Low enough to encourage legitimate complaints |
| `ComplaintSlashPercent` | 20% | Pool deposit slash on valid complaint |
| `MaxOpenComplaints` | 10 | Per-pool limit prevents DoS |
| `SlashGracePeriod` | 600 blocks (~1 hr) | Enough time for pool owner to respond |

### `on_initialize` Auto-Execution

Each block, the runtime checks `PendingComplaintSlash` for entries past their grace period:
- If status is `ResolvedValid` (not appealed) and block > execute_after → execute slash
- Slash amount: `ComplaintSlashPercent` of pool deposit
- Distribution: 50% burned + 50% to complainant
- Pool reputation reduced by 5

## 5. Economic Analysis

### For Miners (Benchmark Claims)

| Action | Cost | Benefit | Expected Value |
|--------|------|---------|----------------|
| Honest claim | 500 DBC stake (refundable) | Higher scheduling priority | Positive (stake returned) |
| Inflated claim | 500 DBC stake (at risk) | Short-term priority boost | Negative (expected loss = P(caught) × 500) |
| Challenge honest miner | 200 DBC stake | 0 | Negative (-200 DBC) |
| Challenge dishonest miner | 200 DBC stake | 250 DBC reward | Positive (+50 DBC) |

### For Users (Complaints)

| Action | Cost | Benefit | Expected Value |
|--------|------|---------|----------------|
| Legitimate complaint | 100 DBC (refunded if valid) | Quality enforcement | Positive |
| Frivolous complaint | 100 DBC (lost) | None | Negative (-100 DBC) |
| Cancel complaint | 10 DBC (10% penalty) | Recover 90% | Context-dependent |

### Sybil Resistance

- Stake requirements create economic barriers against mass-fabricated complaints/challenges
- Each challenge requires 200 DBC locked capital
- Self-challenge is explicitly blocked (cannot challenge own benchmark)

## 6. Test Coverage

| Suite | Tests | Coverage |
|-------|-------|----------|
| Unit tests (agent-attestation) | 25 | All benchmark extrinsics + edge cases |
| Unit tests (compute-pool-scheduler) | 29 | All complaint extrinsics + on_initialize |
| Unit tests (other 3 pallets) | 41 | Existing functionality |
| E2E integration | 40 | Full 7-pallet workflow on dev chain |
| **Total** | **135** | **All features verified** |

## 7. Future Upgrades (v2)

### Decentralized Verification
Replace `AdminOrigin` with a committee-based system:
1. **Committee Selection**: Random selection from registered verifiers (like DBC 2.0 online-committee)
2. **Hash-Reveal Scheme**: Prevent collusion via two-phase commitment
3. **Minimum Committee Size**: 3-5 verifiers per challenge

### Automated Benchmarking
1. Standardized benchmark suites published on-chain
2. Automated benchmark execution via TEE (Trusted Execution Environment)
3. Statistical comparison: claimed score vs measured score (within tolerance band)

### Reputation-Weighted Scheduling
1. Benchmark scores directly influence pool scoring weight
2. Slashed miners receive scheduling penalty (reduced task assignment probability)
3. Complaint history affects pool trust score

---

*Document version: 1.0 — 2026-02-22*
*Pallets: pallet-agent-attestation (call_index 7-10), pallet-compute-pool-scheduler (call_index 10-13)*
