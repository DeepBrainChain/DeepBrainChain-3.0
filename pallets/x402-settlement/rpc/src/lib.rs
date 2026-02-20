#![warn(unused_crate_dependencies)]

use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use parity_scale_codec::Codec;
pub use x402_settlement_runtime_api::X402SettlementApi as X402SettlementRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

#[rpc(client, server)]
pub trait X402SettlementRpcApi<BlockHash, AccountId> {
    /// Get payment intent by ID (returns hex-encoded SCALE bytes)
    #[method(name = "x402_getPaymentIntent")]
    fn get_payment_intent(&self, intent_id: u64, at: Option<BlockHash>) -> RpcResult<Option<String>>;

    /// Get settlement receipt by ID (returns hex-encoded SCALE bytes)
    #[method(name = "x402_getSettlementReceipt")]
    fn get_settlement_receipt(&self, intent_id: u64, at: Option<BlockHash>) -> RpcResult<Option<String>>;

    /// Check if a nonce has been used
    #[method(name = "x402_isNonceUsed")]
    fn is_nonce_used(&self, account: AccountId, nonce: u64, at: Option<BlockHash>) -> RpcResult<bool>;

    /// Get the next intent ID
    #[method(name = "x402_getNextIntentId")]
    fn get_next_intent_id(&self, at: Option<BlockHash>) -> RpcResult<u64>;

    /// Get number of pending payment intents
    #[method(name = "x402_getPendingIntentsCount")]
    fn get_pending_intents_count(&self, at: Option<BlockHash>) -> RpcResult<u64>;
}

pub struct X402Settlement<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> X402Settlement<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId> X402SettlementRpcApiServer<<Block as BlockT>::Hash, AccountId>
    for X402Settlement<C, Block>
where
    Block: BlockT,
    AccountId: Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: X402SettlementRuntimeApi<Block, AccountId, u128>,
{
    fn get_payment_intent(
        &self,
        intent_id: u64,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<String>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        api.get_payment_intent(at_hash, intent_id)
            .map(|opt| opt.map(|bytes| format!("0x{}", hex::encode(bytes))))
            .map_err(|e| {
                CallError::Custom(ErrorObject::owned(1, "Runtime error", Some(e.to_string()))).into()
            })
    }

    fn get_settlement_receipt(
        &self,
        intent_id: u64,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<String>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        api.get_settlement_receipt(at_hash, intent_id)
            .map(|opt| opt.map(|bytes| format!("0x{}", hex::encode(bytes))))
            .map_err(|e| {
                CallError::Custom(ErrorObject::owned(1, "Runtime error", Some(e.to_string()))).into()
            })
    }

    fn is_nonce_used(
        &self,
        account: AccountId,
        nonce: u64,
        at: Option<Block::Hash>,
    ) -> RpcResult<bool> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        api.is_nonce_used(at_hash, account, nonce)
            .map_err(|e| {
                CallError::Custom(ErrorObject::owned(1, "Runtime error", Some(e.to_string()))).into()
            })
    }

    fn get_next_intent_id(
        &self,
        at: Option<Block::Hash>,
    ) -> RpcResult<u64> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        api.get_next_intent_id(at_hash)
            .map_err(|e| {
                CallError::Custom(ErrorObject::owned(1, "Runtime error", Some(e.to_string()))).into()
            })
    }

    fn get_pending_intents_count(
        &self,
        at: Option<Block::Hash>,
    ) -> RpcResult<u64> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        api.get_pending_intents_count(at_hash)
            .map_err(|e| {
                CallError::Custom(ErrorObject::owned(1, "Runtime error", Some(e.to_string()))).into()
            })
    }
}
