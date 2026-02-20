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

pub struct AttestationPrecompile<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    QueryNode = "queryNode(address)",
    Heartbeat = "heartbeat()",
}

impl<T> Precompile for AttestationPrecompile<T>
where
    T: pallet_evm::Config + pallet_agent_attestation::Config,
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
            Selector::QueryNode => Self::query_node(handle),
            Selector::Heartbeat => Self::do_heartbeat(handle),
        }
    }
}

impl<T> AttestationPrecompile<T>
where
    T: pallet_evm::Config + pallet_agent_attestation::Config,
{
    fn query_node(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        handle.record_cost(T::GasWeightMapping::weight_to_gas(
            Weight::from_parts(10_000, 0),
        ))?;
        let input = handle.input();
        let param = ethabi::decode(
            &[ethabi::ParamType::Address],
            &input.get(4..).unwrap_or_default(),
        ).map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("decode failed: {:?}", e).into(),
        })?;
        let addr = param[0].clone().into_address().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode address failed".into(),
        })?;
        let account: T::AccountId = T::AddressMapping::into_account_id(addr);
        let node = pallet_agent_attestation::Nodes::<T>::get(&account);
        let (registered, reputation) = match node {
            Some(n) => (true, n.tflops),
            None => (false, 0u32),
        };
        let encoded = ethabi::encode(&[
            ethabi::Token::Bool(registered),
            ethabi::Token::Uint(U256::from(reputation)),
        ]);
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: encoded,
        })
    }

    fn do_heartbeat(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        handle.record_cost(T::GasWeightMapping::weight_to_gas(
            Weight::from_parts(50_000, 0),
        ))?;
        let from = T::AddressMapping::into_account_id(handle.context().caller);
        let origin = frame_system::RawOrigin::Signed(from);
        pallet_agent_attestation::Pallet::<T>::heartbeat(
            origin.into(),
        ).map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("heartbeat failed: {:?}", e).into(),
        })?;
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: ethabi::encode(&[ethabi::Token::Bool(true)]),
        })
    }
}
