#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![warn(unused_crate_dependencies)]

extern crate alloc;
use sp_std as _;

use alloc::vec::Vec;
use parity_scale_codec::Codec;
use sp_runtime::traits::MaybeDisplay;

sp_api::decl_runtime_apis! {
    /// Consolidated runtime API for all DBC 3.0 pallets.
    ///
    /// Storage values are returned as SCALE-encoded `Vec<u8>` to avoid coupling
    /// the RPC layer to pallet-internal types. Clients decode with the
    /// corresponding pallet structs.
    pub trait Dbc3Api<AccountId, BlockNumber, Balance> where
        AccountId: Codec,
        BlockNumber: Codec + MaybeDisplay,
        Balance: Codec + MaybeDisplay,
    {
        // ─── Task Mode ───────────────────────────────────────────────

        /// Get a task definition by ID (SCALE-encoded `TaskDefinition`).
        fn get_task_definition(task_id: u64) -> Option<Vec<u8>>;

        /// Get a task order by ID (SCALE-encoded `TaskOrder`).
        fn get_task_order(order_id: u64) -> Option<Vec<u8>>;

        /// List task definitions from 0..=next_task_id (max 100 entries, SCALE-encoded `Vec<(u64, TaskDefinition)>`).
        fn list_task_definitions() -> Vec<u8>;

        /// Get era task stats (SCALE-encoded `EraTaskStats`).
        fn get_era_task_stats(era: u32) -> Option<Vec<u8>>;

        /// Get the current era index.
        fn get_current_era() -> u32;

        // ─── Compute Pool Scheduler ──────────────────────────────────

        /// Get a compute pool by ID (SCALE-encoded `ComputePool`).
        fn get_compute_pool(pool_id: u64) -> Option<Vec<u8>>;

        /// Get all active pool IDs.
        fn get_active_pools() -> Vec<u64>;

        /// Get a compute task by ID (SCALE-encoded `ComputeTask`).
        fn get_compute_task(task_id: u64) -> Option<Vec<u8>>;

        /// Get pool reputation score (0-100).
        fn get_pool_reputation(pool_id: u64) -> Option<u32>;

        /// Get a pool by ID (SCALE-encoded `ComputePool`).
        fn get_pool(pool_id: u64) -> Option<Vec<u8>>;

        /// Get a task by ID (SCALE-encoded `ComputeTask`).
        fn get_task(task_id: u64) -> Option<Vec<u8>>;

        /// Get a pool score by pool ID (SCALE-encoded `PoolScore`).
        fn get_pool_score(pool_id: u64) -> Option<Vec<u8>>;

        /// List active task IDs in a pool (SCALE-encoded `Vec<u64>`).
        fn list_active_tasks(pool_id: u64) -> Vec<u8>;

        // ─── Agent Attestation ───────────────────────────────────────

        /// Get an attestation by ID (SCALE-encoded `Attestation`).
        fn get_attestation(attestation_id: u64) -> Option<Vec<u8>>;

        /// Get node registration for an account (SCALE-encoded `NodeRegistration`).
        fn get_node_registration(who: AccountId) -> Option<Vec<u8>>;

        /// Get pending attestation count.
        fn get_pending_attestation_count() -> u64;

        // ─── X402 Settlement ─────────────────────────────────────────

        /// Get a payment intent by ID (SCALE-encoded `PaymentIntent`).
        fn get_payment_intent(intent_id: u64) -> Option<Vec<u8>>;

        /// Get a settlement receipt by intent ID (SCALE-encoded `SettlementReceipt`).
        fn get_settlement_receipt(intent_id: u64) -> Option<Vec<u8>>;

        // ─── Delegated Staking ─────────────────────────────────────

        /// Get delegated-staking agent ledger by agent account (SCALE-encoded `AgentLedger`).
        fn get_delegated_agent(agent: AccountId) -> Option<Vec<u8>>;

        /// Get delegated-staking delegator record by delegator account (SCALE-encoded `Delegation`).
        fn get_delegator_state(delegator: AccountId) -> Option<Vec<u8>>;

        /// Get total registered delegated-staking agent count.
        fn get_delegated_agent_count() -> u64;

        /// Get total delegated stake amount across all agents.
        fn get_total_delegated_stake() -> Balance;

        /// List delegated-staking agents (SCALE-encoded `Vec<(AccountId, AgentLedger)>`).
        fn list_delegated_agents(limit: u32) -> Vec<u8>;

        /// List delegators for a given agent (SCALE-encoded `Vec<(AccountId, Delegation)>`).
        fn list_agent_delegators(agent: AccountId, limit: u32) -> Vec<u8>;

        // ─── Staking Controller Deprecation ────────────────────────

        /// Get controller account for a stash if it is a legacy stash/controller pair.
        fn get_legacy_controller(stash: AccountId) -> Option<AccountId>;

        /// Get stash account for a controller if it is a legacy stash/controller pair.
        fn get_legacy_stash(controller: AccountId) -> Option<AccountId>;

        /// List legacy stash/controller pairs (SCALE-encoded `Vec<(AccountId, AccountId)>`).
        fn list_legacy_controller_pairs(limit: u32) -> Vec<u8>;

        /// Get count of legacy stash/controller pairs.
        fn get_legacy_controller_count() -> u64;

        // --- Event Indexer Summary ---

        /// Get a summary of all DBC 3.0 pallet activity.
        /// Returns SCALE-encoded Dbc3Summary struct.
        fn get_network_summary() -> Vec<u8>;

        /// Get active node count (nodes with recent heartbeat).
        fn get_active_node_count() -> u64;

        /// Get total staked amount across all pools.
        fn get_total_network_stake() -> Balance;

        /// Get all registered node accounts.
        fn get_registered_nodes() -> Vec<AccountId>;
    }
}
