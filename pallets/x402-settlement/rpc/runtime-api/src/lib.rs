#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::Codec;
use sp_std::prelude::Vec;

sp_api::decl_runtime_apis! {
    pub trait X402SettlementApi<AccountId, Balance> where
        AccountId: Codec,
        Balance: Codec
    {
        /// Query a payment intent by ID, returns SCALE-encoded PaymentIntent or None
        fn get_payment_intent(intent_id: u64) -> Option<Vec<u8>>;

        /// Query a settlement receipt by ID, returns SCALE-encoded SettlementReceipt or None
        fn get_settlement_receipt(intent_id: u64) -> Option<Vec<u8>>;

        /// Check if a nonce has been used for an account
        fn is_nonce_used(account: AccountId, nonce: u64) -> bool;

        /// Get the next intent ID
        fn get_next_intent_id() -> u64;

        /// Get number of pending payment intents
        fn get_pending_intents_count() -> u64;
    }
}
