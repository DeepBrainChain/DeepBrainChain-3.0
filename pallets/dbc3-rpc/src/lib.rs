#![warn(unused_crate_dependencies)]

use std::sync::Arc;

use dbc3_runtime_api::Dbc3Api as Dbc3StorageRuntimeApi;
use jsonrpsee::{
    core::{Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode, ErrorObject},
};
use parity_scale_codec::Codec;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;

pub use dbc3_runtime_api::Dbc3Api as Dbc3RuntimeApi;

#[rpc(client, server)]
pub trait Dbc3RpcApi<BlockHash, AccountId> {
    // === Task Mode ===
    #[method(name = "dbc3_getTaskDefinition")]
    fn get_task_definition(&self, task_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getTaskOrder")]
    fn get_task_order(&self, order_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getEraTaskStats")]
    fn get_era_task_stats(&self, era: u32, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getCurrentEra")]
    fn get_current_era(&self, at: Option<BlockHash>) -> RpcResult<u32>;

    // === Compute Pool Scheduler ===
    #[method(name = "dbc3_getComputePool")]
    fn get_compute_pool(&self, pool_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getActivePools")]
    fn get_active_pools(&self, at: Option<BlockHash>) -> RpcResult<Vec<u64>>;

    #[method(name = "dbc3_getComputeTask")]
    fn get_compute_task(&self, task_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getPoolReputation")]
    fn get_pool_reputation(&self, pool_id: u64, at: Option<BlockHash>) -> RpcResult<Option<u32>>;

    // === Agent Attestation ===
    #[method(name = "dbc3_getAttestation")]
    fn get_attestation(&self, attestation_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getNodeRegistration")]
    fn get_node_registration(&self, who: AccountId, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getPendingAttestationCount")]
    fn get_pending_attestation_count(&self, at: Option<BlockHash>) -> RpcResult<u64>;

    // === X402 Settlement ===
    #[method(name = "dbc3_getPaymentIntent")]
    fn get_payment_intent(&self, intent_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getSettlementReceipt")]
    fn get_settlement_receipt(&self, intent_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    // === ZK Compute ===
    #[method(name = "dbc3_getZkTask")]
    fn get_zk_task(&self, task_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getMinerScore")]
    fn get_miner_score(&self, miner: AccountId, at: Option<BlockHash>) -> RpcResult<u32>;
}

pub struct Dbc3Storage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> Dbc3Storage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

fn map_err(e: impl std::fmt::Debug) -> JsonRpseeError {
    JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
        ErrorCode::InternalError.code(),
        format!("{e:?}"),
        None::<()>,
    )))
}

impl<C, Block, AccountId>
    Dbc3RpcApiServer<<Block as BlockT>::Hash, AccountId>
    for Dbc3Storage<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: Dbc3StorageRuntimeApi<Block, AccountId, u32, u128>,
    AccountId: Clone + std::fmt::Display + Codec + Send + 'static,
{
    fn get_task_definition(&self, task_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_task_definition(at_hash, task_id).map_err(map_err)
    }

    fn get_task_order(&self, order_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_task_order(at_hash, order_id).map_err(map_err)
    }

    fn get_era_task_stats(&self, era: u32, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_era_task_stats(at_hash, era).map_err(map_err)
    }

    fn get_current_era(&self, at: Option<Block::Hash>) -> RpcResult<u32> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_current_era(at_hash).map_err(map_err)
    }

    fn get_compute_pool(&self, pool_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_compute_pool(at_hash, pool_id).map_err(map_err)
    }

    fn get_active_pools(&self, at: Option<Block::Hash>) -> RpcResult<Vec<u64>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_active_pools(at_hash).map_err(map_err)
    }

    fn get_compute_task(&self, task_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_compute_task(at_hash, task_id).map_err(map_err)
    }

    fn get_pool_reputation(&self, pool_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<u32>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_pool_reputation(at_hash, pool_id).map_err(map_err)
    }

    fn get_attestation(&self, attestation_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_attestation(at_hash, attestation_id).map_err(map_err)
    }

    fn get_node_registration(&self, who: AccountId, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_node_registration(at_hash, who).map_err(map_err)
    }

    fn get_pending_attestation_count(&self, at: Option<Block::Hash>) -> RpcResult<u64> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_pending_attestation_count(at_hash).map_err(map_err)
    }

    fn get_payment_intent(&self, intent_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_payment_intent(at_hash, intent_id).map_err(map_err)
    }

    fn get_settlement_receipt(&self, intent_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_settlement_receipt(at_hash, intent_id).map_err(map_err)
    }

    fn get_zk_task(&self, task_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_zk_task(at_hash, task_id).map_err(map_err)
    }

    fn get_miner_score(&self, miner: AccountId, at: Option<Block::Hash>) -> RpcResult<u32> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_miner_score(at_hash, miner).map_err(map_err)
    }
}
