#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![warn(unused_crate_dependencies)]

use parity_scale_codec::Codec;
use sp_runtime::traits::MaybeDisplay;
use sp_std::prelude::Vec;

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

        // ─── ZK Compute ─────────────────────────────────────────────

        /// Get a ZK task by ID (SCALE-encoded `ZkTask`).
        fn get_zk_task(task_id: u64) -> Option<Vec<u8>>;

        /// Get miner score (0-based).
        fn get_miner_score(miner: AccountId) -> u32;
    }
}
