#![warn(unused_crate_dependencies)]

use std::sync::Arc;

use dbc3_runtime_api::Dbc3Api as Dbc3StorageRuntimeApi;
use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::error::{ErrorCode, ErrorObject},
};
use parity_scale_codec::Codec;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;

pub use dbc3_runtime_api::Dbc3Api as Dbc3RuntimeApi;


#[rpc(client, server)]
pub trait Dbc3RpcApi<BlockHash, AccountId> {
    // === Task Mode ===
    #[method(name = "taskMode_getTaskDefinition")]
    fn get_task_definition(&self, task_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "taskMode_getTaskOrder")]
    fn get_task_order(&self, order_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "taskMode_listTaskDefinitions")]
    fn list_task_definitions(&self, at: Option<BlockHash>) -> RpcResult<Vec<u8>>;

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

    #[method(name = "poolScheduler_getPool")]
    fn get_pool(&self, pool_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "poolScheduler_getTask")]
    fn get_task(&self, task_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "poolScheduler_getPoolScore")]
    fn get_pool_score(&self, pool_id: u64, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "poolScheduler_listActiveTasks")]
    fn list_active_tasks(&self, pool_id: u64, at: Option<BlockHash>) -> RpcResult<Vec<u8>>;

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

    // === Delegated Staking ===
    #[method(name = "dbc3_getDelegatedAgent")]
    fn get_delegated_agent(&self, agent: AccountId, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getDelegatorState")]
    fn get_delegator_state(&self, delegator: AccountId, at: Option<BlockHash>) -> RpcResult<Option<Vec<u8>>>;

    #[method(name = "dbc3_getDelegatedAgentCount")]
    fn get_delegated_agent_count(&self, at: Option<BlockHash>) -> RpcResult<u64>;

    #[method(name = "dbc3_getTotalDelegatedStake")]
    fn get_total_delegated_stake(&self, at: Option<BlockHash>) -> RpcResult<String>;

    #[method(name = "dbc3_listDelegatedAgents")]
    fn list_delegated_agents(&self, limit: u32, at: Option<BlockHash>) -> RpcResult<Vec<u8>>;

    #[method(name = "dbc3_listAgentDelegators")]
    fn list_agent_delegators(&self, agent: AccountId, limit: u32, at: Option<BlockHash>) -> RpcResult<Vec<u8>>;

    // === Staking Controller Deprecation ===
    #[method(name = "dbc3_getLegacyController")]
    fn get_legacy_controller(&self, stash: AccountId, at: Option<BlockHash>) -> RpcResult<Option<AccountId>>;

    #[method(name = "dbc3_getLegacyStash")]
    fn get_legacy_stash(&self, controller: AccountId, at: Option<BlockHash>) -> RpcResult<Option<AccountId>>;

    #[method(name = "dbc3_listLegacyControllerPairs")]
    fn list_legacy_controller_pairs(&self, limit: u32, at: Option<BlockHash>) -> RpcResult<Vec<u8>>;

    #[method(name = "dbc3_getLegacyControllerCount")]
    fn get_legacy_controller_count(&self, at: Option<BlockHash>) -> RpcResult<u64>;

    #[method(name = "dbc3_getNetworkSummary")]
    fn get_network_summary(&self, at: Option<BlockHash>) -> RpcResult<String>;

    #[method(name = "dbc3_getActiveNodeCount")]
    fn get_active_node_count(&self, at: Option<BlockHash>) -> RpcResult<u64>;

    #[method(name = "dbc3_getTotalNetworkStake")]
    fn get_total_network_stake(&self, at: Option<BlockHash>) -> RpcResult<String>;

    #[method(name = "dbc3_getRegisteredNodes")]
    fn get_registered_nodes(&self, at: Option<BlockHash>) -> RpcResult<Vec<AccountId>>;
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

fn map_err(e: impl std::fmt::Debug) -> jsonrpsee::types::ErrorObjectOwned {
    ErrorObject::owned(
        ErrorCode::InternalError.code(),
        format!("{e:?}"),
        None::<()>,
    )
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

    fn list_task_definitions(&self, at: Option<Block::Hash>) -> RpcResult<Vec<u8>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.list_task_definitions(at_hash).map_err(map_err)
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

    fn get_pool(&self, pool_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_pool(at_hash, pool_id).map_err(map_err)
    }

    fn get_task(&self, task_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_task(at_hash, task_id).map_err(map_err)
    }

    fn get_pool_score(&self, pool_id: u64, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_pool_score(at_hash, pool_id).map_err(map_err)
    }

    fn list_active_tasks(&self, pool_id: u64, at: Option<Block::Hash>) -> RpcResult<Vec<u8>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.list_active_tasks(at_hash, pool_id).map_err(map_err)
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

    fn get_delegated_agent(&self, agent: AccountId, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_delegated_agent(at_hash, agent).map_err(map_err)
    }

    fn get_delegator_state(&self, delegator: AccountId, at: Option<Block::Hash>) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_delegator_state(at_hash, delegator).map_err(map_err)
    }

    fn get_delegated_agent_count(&self, at: Option<Block::Hash>) -> RpcResult<u64> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_delegated_agent_count(at_hash).map_err(map_err)
    }

    fn get_total_delegated_stake(&self, at: Option<Block::Hash>) -> RpcResult<String> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        let stake = api.get_total_delegated_stake(at_hash).map_err(map_err)?;
        Ok(format!("{}", stake))
    }

    fn list_delegated_agents(&self, limit: u32, at: Option<Block::Hash>) -> RpcResult<Vec<u8>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.list_delegated_agents(at_hash, limit).map_err(map_err)
    }

    fn list_agent_delegators(
        &self,
        agent: AccountId,
        limit: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<Vec<u8>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.list_agent_delegators(at_hash, agent, limit).map_err(map_err)
    }

    fn get_legacy_controller(&self, stash: AccountId, at: Option<Block::Hash>) -> RpcResult<Option<AccountId>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_legacy_controller(at_hash, stash).map_err(map_err)
    }

    fn get_legacy_stash(
        &self,
        controller: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<Option<AccountId>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_legacy_stash(at_hash, controller).map_err(map_err)
    }

    fn list_legacy_controller_pairs(
        &self,
        limit: u32,
        at: Option<Block::Hash>,
    ) -> RpcResult<Vec<u8>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.list_legacy_controller_pairs(at_hash, limit).map_err(map_err)
    }

    fn get_legacy_controller_count(&self, at: Option<Block::Hash>) -> RpcResult<u64> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_legacy_controller_count(at_hash).map_err(map_err)
    }

    fn get_network_summary(&self, at: Option<Block::Hash>) -> RpcResult<String> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        let summary = api.get_network_summary(at_hash).map_err(map_err)?;
        // Return hex string of SCALE-encoded (active_pools, total_tasks, pending_attestations, pending_intents, next_intent_id)
        let hex_str: String = summary.iter().map(|b| format!("{:02x}", b)).collect();
        Ok(format!("0x{}", hex_str))
    }

    fn get_active_node_count(&self, at: Option<Block::Hash>) -> RpcResult<u64> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_active_node_count(at_hash).map_err(map_err)
    }

    fn get_total_network_stake(&self, at: Option<Block::Hash>) -> RpcResult<String> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        let stake = api.get_total_network_stake(at_hash).map_err(map_err)?;
        Ok(format!("{}", stake))
    }

    fn get_registered_nodes(&self, at: Option<Block::Hash>) -> RpcResult<Vec<AccountId>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        api.get_registered_nodes(at_hash).map_err(map_err)
    }
}
