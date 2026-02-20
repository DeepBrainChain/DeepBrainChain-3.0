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
use pallet_evm::{AddressMapping, GasWeightMapping};

pub struct ZkComputePrecompile<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    QueryTask = "queryTask(uint64)",
    ClaimReward = "claimReward(uint64)",
}

impl<T> Precompile for ZkComputePrecompile<T>
where
    T: pallet_evm::Config + pallet_zk_compute::Config,
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
            Selector::QueryTask => Self::query_task(handle),
            Selector::ClaimReward => Self::claim_reward(handle),
        }
    }
}

impl<T> ZkComputePrecompile<T>
where
    T: pallet_evm::Config + pallet_zk_compute::Config,
{
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
        let task_id_u256 = param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode task_id failed".into(),
        })?;
        let task_id = task_id_u256.as_u64();
        let task_id_typed: T::TaskId = task_id.try_into().map_err(|_| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "invalid task_id".into(),
        })?;
        let task = pallet_zk_compute::Tasks::<T>::get(task_id_typed);
        let (status, m, n, k) = match task {
            Some(t) => {
                let s: u8 = match t.status {
                    pallet_zk_compute::ZkVerificationStatus::Pending => 0,
                    pallet_zk_compute::ZkVerificationStatus::Verified => 1,
                    pallet_zk_compute::ZkVerificationStatus::Failed => 2,
                };
                (s, t.dimensions.0, t.dimensions.1, t.dimensions.2)
            },
            None => (255u8, 0u32, 0u32, 0u32),
        };
        let encoded = ethabi::encode(&[
            ethabi::Token::Uint(U256::from(status)),
            ethabi::Token::Uint(U256::from(m)),
            ethabi::Token::Uint(U256::from(n)),
            ethabi::Token::Uint(U256::from(k)),
        ]);
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: encoded,
        })
    }

    fn claim_reward(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        handle.record_cost(T::GasWeightMapping::weight_to_gas(
            Weight::from_parts(50_000, 0),
        ))?;
        let input = handle.input();
        let param = ethabi::decode(
            &[ethabi::ParamType::Uint(64)],
            &input.get(4..).unwrap_or_default(),
        ).map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("decode failed: {:?}", e).into(),
        })?;
        let task_id_u256 = param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode task_id failed".into(),
        })?;
        let task_id = task_id_u256.as_u64();
        let task_id_typed: T::TaskId = task_id.try_into().map_err(|_| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "invalid task_id".into(),
        })?;
        let from = T::AddressMapping::into_account_id(handle.context().caller);
        let origin = frame_system::RawOrigin::Signed(from);
        pallet_zk_compute::Pallet::<T>::claim_reward(
            origin.into(),
            task_id_typed,
        ).map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("claim_reward failed: {:?}", e).into(),
        })?;
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[ethabi::Token::Bool(true)]),
        })
    }
}
