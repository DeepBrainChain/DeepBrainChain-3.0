use fp_evm::{
    ExitRevert, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
    PrecompileResult,
};
use sp_runtime::RuntimeDebug;
extern crate alloc;
use alloc::format;
use core::marker::PhantomData;
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;

pub struct X402SettlementPrecompile<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    QueryPaymentIntent = "queryPaymentIntent(uint64)",
    QuerySettlementReceipt = "querySettlementReceipt(uint64)",
}

impl<T> Precompile for X402SettlementPrecompile<T>
where
    T: pallet_evm::Config + pallet_x402_settlement::Config,
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
            Selector::QueryPaymentIntent => Self::query_payment_intent(handle),
            Selector::QuerySettlementReceipt => Self::query_settlement_receipt(handle),
        }
    }
}

impl<T> X402SettlementPrecompile<T>
where
    T: pallet_evm::Config + pallet_x402_settlement::Config,
{
    fn query_payment_intent(handle: &mut impl PrecompileHandle) -> PrecompileResult {
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
        let intent_id = param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode intent_id failed".into(),
        })?.as_u64();
        let intent = pallet_x402_settlement::Pallet::<T>::get_payment_intent(intent_id);
        let (exists, settled) = match intent {
            Some(p) => {
                let settled = matches!(p.status, pallet_x402_settlement::PaymentIntentStatus::Settled);
                (true, settled)
            },
            None => (false, false),
        };
        let encoded = ethabi::encode(&[
            ethabi::Token::Bool(exists),
            ethabi::Token::Bool(settled),
        ]);
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: encoded,
        })
    }

    fn query_settlement_receipt(handle: &mut impl PrecompileHandle) -> PrecompileResult {
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
        let intent_id = param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: "decode intent_id failed".into(),
        })?.as_u64();
        let receipt = pallet_x402_settlement::Pallet::<T>::get_settlement_receipt(intent_id);
        let exists = receipt.is_some();
        let encoded = ethabi::encode(&[
            ethabi::Token::Bool(exists),
        ]);
        Ok(PrecompileOutput {
            exit_status: ExitSucceed::Returned,
            output: encoded,
        })
    }
}
