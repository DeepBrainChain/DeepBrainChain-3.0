use fp_evm::{
    ExitRevert, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
    PrecompileResult,
};
use sp_core::U256;
use sp_runtime::RuntimeDebug;
extern crate alloc;
use alloc::format;
use core::marker::PhantomData;
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;

pub struct ComputePoolPrecompile<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    QueryPool = "queryPool(uint64)",
    QueryTask = "queryTask(uint64)",
}

impl<T> Precompile for ComputePoolPrecompile<T>
where
    T: pallet_evm::Config + pallet_compute_pool_scheduler::Config,
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
            Selector::QueryPool => Self::query_pool(handle),
            Selector::QueryTask => Self::query_task(handle),
        }
    }
}

impl<T> ComputePoolPrecompile<T>
where
    T: pallet_evm::Config + pallet_compute_pool_scheduler::Config,
{
    fn query_pool(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        handle.record_cost(T::GasWeightMapping::weight_to_gas(
            Weight::from_parts(10_000, 0),
        ))?;
        let input = handle.input();
        let param = ethabi::decode(
            &[ethabi::ParamType::Uint(64)],
            &input.get(4..).unwrap_or_default(),
        ).map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("decode failed: {:?}", e).into(),
        })?;
        let pool_id = param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode pool_id failed".into(),
        })?.as_u64();
        let pool = pallet_compute_pool_scheduler::Pools::<T>::get(pool_id);
        let (active, gpu_count, max_tasks) = match pool {
            Some(p) => (true, p.gpu_memory, p.total_tasks),
            None => (false, 0u32, 0u32),
        };
        let encoded = ethabi::encode(&[
            ethabi::Token::Bool(active),
            ethabi::Token::Uint(U256::from(gpu_count)),
            ethabi::Token::Uint(U256::from(max_tasks)),
        ]);
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: encoded,
        })
    }

    fn query_task(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        handle.record_cost(T::GasWeightMapping::weight_to_gas(
            Weight::from_parts(10_000, 0),
        ))?;
        let input = handle.input();
        let param = ethabi::decode(
            &[ethabi::ParamType::Uint(64)],
            &input.get(4..).unwrap_or_default(),
        ).map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("decode failed: {:?}", e).into(),
        })?;
        let task_id = param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode task_id failed".into(),
        })?.as_u64();
        let task = pallet_compute_pool_scheduler::Tasks::<T>::get(task_id);
        let (exists, pool_id, status) = match task {
            Some(t) => {
                let s: u8 = match t.status {
                    pallet_compute_pool_scheduler::TaskStatus::Pending => 0,
                    pallet_compute_pool_scheduler::TaskStatus::Assigned => 1,
                    pallet_compute_pool_scheduler::TaskStatus::Computing => 2,
                    pallet_compute_pool_scheduler::TaskStatus::ProofSubmitted => 3,
                    pallet_compute_pool_scheduler::TaskStatus::Verifying => 4,
                    pallet_compute_pool_scheduler::TaskStatus::Completed => 5,
                    pallet_compute_pool_scheduler::TaskStatus::Failed => 6,
                };
                (true, t.pool_id, s)
            },
            None => (false, 0u64, 255u8),
        };
        let encoded = ethabi::encode(&[
            ethabi::Token::Bool(exists),
            ethabi::Token::Uint(U256::from(pool_id)),
            ethabi::Token::Uint(U256::from(status)),
        ]);
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: encoded,
        })
    }
}
