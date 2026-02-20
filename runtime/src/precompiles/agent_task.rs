use fp_evm::{
    ExitRevert, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
    PrecompileResult,
};
use sp_core::{Get, U256};
use sp_runtime::RuntimeDebug;
extern crate alloc;
use crate::precompiles::LOG_TARGET;
use alloc::format;
use core::marker::PhantomData;
use frame_support::{ensure, pallet_prelude::Weight, traits::Currency};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::{AddressMapping, GasWeightMapping};

pub struct AgentTask<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    CreateTaskOrder = "createTaskOrder(uint64,address,uint64,uint64)",
    QueryTaskStatus = "queryTaskStatus(uint64)",
    GetModelPrice = "getModelPrice(bytes)",
    RegisterNode = "registerNode(bytes,uint32)",
}

type BalanceOf<T> = <<T as pallet_task_mode::Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

impl<T> Precompile for AgentTask<T>
where
    T: pallet_evm::Config + pallet_task_mode::Config + pallet_agent_attestation::Config,
    BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();

        ensure!(
            input.len() >= 4,
            PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: "invalid input".into(),
            }
        );

        let selector = u32::from_be_bytes(input[..4].try_into().expect("checked. qed!"));
        let selector: Selector = selector.try_into().map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("invalid selector: {:?}", e).into(),
        })?;

        match selector {
            Selector::CreateTaskOrder => Self::create_task_order(handle),
            Selector::QueryTaskStatus => Self::query_task_status(handle),
            Selector::GetModelPrice => Self::get_model_price(handle),
            Selector::RegisterNode => Self::register_node(handle),
        }
    }
}

impl<T> AgentTask<T>
where
    T: pallet_evm::Config + pallet_task_mode::Config + pallet_agent_attestation::Config,
    BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
    /// createTaskOrder(uint64 task_id, address miner, uint64 input_tokens, uint64 output_tokens)
    /// Returns: uint64 order_id
    fn create_task_order(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();
        
        let param = ethabi::decode(
            &[
                ethabi::ParamType::Uint(64),  // task_id
                ethabi::ParamType::Address,   // miner
                ethabi::ParamType::Uint(64),  // input_tokens
                ethabi::ParamType::Uint(64),  // output_tokens
            ],
            &input.get(4..).unwrap_or_default(),
        )
        .map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("decode param failed: {:?}", e).into(),
        })?;

        let task_id_uint = param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode task_id failed".into(),
        })?;
        let task_id: u64 = task_id_uint.as_u64();

        let miner_address =
            param[1].clone().into_address().ok_or_else(|| PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: "decode miner address failed".into(),
            })?;
        let miner_account: T::AccountId = T::AddressMapping::into_account_id(miner_address);

        let input_tokens_uint =
            param[2].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: "decode input_tokens failed".into(),
            })?;
        let input_tokens: u64 = input_tokens_uint.as_u64();

        let output_tokens_uint =
            param[3].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: "decode output_tokens failed".into(),
            })?;
        let output_tokens: u64 = output_tokens_uint.as_u64();

        // Get caller account
        let caller_evm = handle.context().caller;
        let caller_account: T::AccountId = T::AddressMapping::into_account_id(caller_evm);

        log::debug!(
            target: LOG_TARGET,
            "create_task_order: caller: {:?}, task_id: {}, miner: {:?}, input_tokens: {}, output_tokens: {}",
            caller_evm,
            task_id,
            miner_address,
            input_tokens,
            output_tokens
        );

        // Get the next order_id before creating the order
        let order_id = pallet_task_mode::NextOrderId::<T>::get();

        // Call pallet-task-mode to create order
        let origin = frame_system::RawOrigin::Signed(caller_account.clone()).into();
        pallet_task_mode::Pallet::<T>::create_task_order(
            origin,
            task_id,
            miner_account,
            input_tokens,
            output_tokens,
        )
        .map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("create_task_order failed: {:?}", e).into(),
        })?;

        // Record gas cost for storage writes
        let weight = Weight::default()
            .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(2))
            .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(2));

        handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[ethabi::Token::Uint(U256::from(order_id))]),
        })
    }

    /// queryTaskStatus(uint64 order_id)
    /// Returns: (uint8 status, address miner, uint256 cost)
    fn query_task_status(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();

        let param = ethabi::decode(
            &[ethabi::ParamType::Uint(64)], // order_id
            &input.get(4..).unwrap_or_default(),
        )
        .map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("decode param failed: {:?}", e).into(),
        })?;

        let order_id_uint =
            param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: "decode order_id failed".into(),
            })?;
        let order_id: u64 = order_id_uint.as_u64();

        log::debug!(
            target: LOG_TARGET,
            "query_task_status: order_id: {}",
            order_id
        );

        // Query order from storage
        let order =
            pallet_task_mode::TaskOrders::<T>::get(order_id).ok_or_else(|| {
                PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("order {} not found", order_id).into(),
                }
            })?;

        // Convert status to u8
        let status: u8 = match order.status {
            pallet_task_mode::TaskOrderStatus::Pending => 0,
            pallet_task_mode::TaskOrderStatus::InProgress => 1,
            pallet_task_mode::TaskOrderStatus::Completed => 2,
            pallet_task_mode::TaskOrderStatus::Settled => 3,
        };

        // Convert miner AccountId to H160
        // Note: This is a simplified conversion. In production, you may need proper account mapping
        let miner_h160 = sp_core::H160::default(); // Placeholder - implement proper conversion

        // Get total cost
        let cost: U256 = order.total_dbc_charged.into();

        // Record gas cost for storage read
        let weight = Weight::default()
            .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

        handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[
                ethabi::Token::Uint(U256::from(status)),
                ethabi::Token::Address(miner_h160),
                ethabi::Token::Uint(cost),
            ]),
        })
    }

    /// getModelPrice(bytes model_id)
    /// Returns: uint256 price_per_token
    /// Note: This is a simplified implementation that returns input price
    fn get_model_price(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();

        let param = ethabi::decode(
            &[ethabi::ParamType::Bytes], // task_id as bytes (we'll use as u64)
            &input.get(4..).unwrap_or_default(),
        )
        .map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("decode param failed: {:?}", e).into(),
        })?;

        let task_id_bytes = param[0].clone().into_bytes().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode task_id failed".into(),
        })?;

        // Convert bytes to u64 (simplified - take first 8 bytes)
        let task_id: u64 = if task_id_bytes.len() >= 8 {
            u64::from_be_bytes(task_id_bytes[0..8].try_into().unwrap_or([0u8; 8]))
        } else {
            0u64
        };

        log::debug!(
            target: LOG_TARGET,
            "get_model_price: task_id: {}",
            task_id
        );

        // Query task definition from storage
        let task_def = pallet_task_mode::TaskDefinitions::<T>::get(task_id).ok_or_else(|| {
            PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: format!("task definition not found for task_id: {}", task_id).into(),
            }
        })?;

        // Return input price (USD per 1k tokens)
        let price: U256 = task_def.input_price_usd_per_1k.into();

        // Record gas cost for storage read
        let weight = Weight::default()
            .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

        handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[ethabi::Token::Uint(price)]),
        })
    }

    /// registerNode(bytes gpu_uuid, uint32 tflops)
    /// Returns: bool success
    fn register_node(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();

        let param = ethabi::decode(
            &[
                ethabi::ParamType::Bytes,     // gpu_uuid
                ethabi::ParamType::Uint(32),  // tflops
            ],
            &input.get(4..).unwrap_or_default(),
        )
        .map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("decode param failed: {:?}", e).into(),
        })?;

        let gpu_uuid = param[0].clone().into_bytes().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode gpu_uuid failed".into(),
        })?;

        let tflops_uint = param[1].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode tflops failed".into(),
        })?;
        let tflops: u32 = tflops_uint.as_u32();

        // Get caller account
        let caller_evm = handle.context().caller;
        let caller_account: T::AccountId = T::AddressMapping::into_account_id(caller_evm);

        log::debug!(
            target: LOG_TARGET,
            "register_node: caller: {:?}, gpu_uuid: {:?}, tflops: {}",
            caller_evm,
            gpu_uuid,
            tflops
        );

        // Call pallet-agent-attestation to register node
        let origin = frame_system::RawOrigin::Signed(caller_account.clone()).into();
        pallet_agent_attestation::Pallet::<T>::register_node(origin, gpu_uuid, tflops)
            .map_err(|e| PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: format!("register_node failed: {:?}", e).into(),
            })?;

        // Record gas cost for storage writes
        let weight = Weight::default()
            .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1))
            .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));

        handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[ethabi::Token::Bool(true)]),
        })
    }
}
