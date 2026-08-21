use core::marker::PhantomData;
use fp_evm::{ExitRevert, PrecompileFailure};
use pallet_evm::{
    IsPrecompileResult, Precompile, PrecompileHandle, PrecompileResult, PrecompileSet,
};
use scale_info::prelude::format;
use sp_core::H160;

use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
use pallet_evm_precompile_dispatch::Dispatch;
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};

mod bridge;
use bridge::Bridge;
mod dbc_price;
use dbc_price::DBCPrice;

mod dlc_price;
mod machine_info;
use dlc_price::DLCPrice;

use machine_info::MachineInfo;

mod agent_task;
use agent_task::AgentTask;

mod compute_pool;
use compute_pool::ComputePoolPrecompile;

mod attestation;
use attestation::AttestationPrecompile;

mod x402_settlement;
use x402_settlement::X402SettlementPrecompile;

const LOG_TARGET: &str = "evm";

/// Convert sp_core::U256 to ethabi's U256 (different primitive-types versions)
pub(crate) fn to_ethabi_u256(v: sp_core::U256) -> ethabi::ethereum_types::U256 {
    ethabi::ethereum_types::U256::from_big_endian(&v.to_big_endian())
}

/// Convert ethabi H160 to sp_core::H160
pub(crate) fn from_ethabi_h160(v: ethabi::ethereum_types::H160) -> sp_core::H160 {
    sp_core::H160(v.0)
}

/// Convert sp_core::H160 to ethabi's H160
pub(crate) fn to_ethabi_h160(v: sp_core::H160) -> ethabi::ethereum_types::H160 {
    ethabi::ethereum_types::H160(v.0)
}

/// Convert ethabi U256 to sp_core::U256
pub(crate) fn from_ethabi_u256(v: ethabi::ethereum_types::U256) -> sp_core::U256 {
    let mut buf = [0u8; 32];
    v.to_big_endian(&mut buf);
    sp_core::U256::from_big_endian(&buf)
}

pub struct DBCPrecompiles<T>(PhantomData<T>);

impl<T> DBCPrecompiles<T>
where
    T: pallet_evm::Config,
{
    pub fn new() -> Self {
        Self(Default::default())
    }
    pub fn used_addresses() -> [H160; 16] {
        [
            hash(1),
            hash(2),
            hash(3),
            hash(4),
            hash(5),
            hash(1024),
            hash(1025),
            hash(1026),
            hash(2048),
            hash(2049),
            hash(2050),
            hash(2051),
            hash(2096), // AgentTask precompile (0x0830)
            hash(2098), // ComputePool precompile
            hash(2099), // Attestation precompile
            hash(2100), // X402Settlement precompile
        ]
    }
}
impl<T> PrecompileSet for DBCPrecompiles<T>
where
    T: pallet_evm::Config + eth_precompile_whitelist::Config,
    Dispatch<T>: Precompile,
    Bridge<T>: Precompile,
    DBCPrice<T>: Precompile,
    MachineInfo<T>: Precompile,
    DLCPrice<T>: Precompile,
    AgentTask<T>: Precompile,
    ComputePoolPrecompile<T>: Precompile,
    AttestationPrecompile<T>: Precompile,
    X402SettlementPrecompile<T>: Precompile,
{
    fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
        let address = handle.code_address();
        let context = handle.context();
        log::debug!(target: LOG_TARGET, "PrecompileSet execute address: {:?}, context: {:?}", address, handle.context());

        if let IsPrecompileResult::Answer { is_precompile: true, extra_cost: _ } =
            self.is_precompile(address, handle.remaining_gas())
        {
            if address > hash(9) && context.address != address {
                return Some(Err(PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: "cannot be called with DELEGATECALL or CALLCODE".into(),
                }))
            }

            // check if the context.caller in the precompile whitelist
            let precompile_whitelist =
                eth_precompile_whitelist::PrecompileWhitelist::<T>::get(address);

            match address {
                a if a == hash(2048) => {
                    if !precompile_whitelist.contains(&context.caller) {
                        log::debug!(target: LOG_TARGET, "caller {:?} not in the {:?} whitelist", context.caller, address);

                        return Some(Err(PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!("caller {:?} not in the whitelist", context.caller)
                                .into(),
                        }))
                    }
                },
                _ => {},
            }
        }

        match address {
            // Ethereum precompiles :
            a if a == hash(1) => Some(ECRecover::execute(handle)),
            a if a == hash(2) => Some(Sha256::execute(handle)),
            a if a == hash(3) => Some(Ripemd160::execute(handle)),
            a if a == hash(4) => Some(Identity::execute(handle)),
            a if a == hash(5) => Some(Modexp::execute(handle)),
            a if a == hash(6) => Some(Bn128Add::execute(handle)),
            a if a == hash(7) => Some(Bn128Mul::execute(handle)),
            a if a == hash(8) => Some(Bn128Pairing::execute(handle)),
            a if a == hash(9) => Some(Blake2F::execute(handle)),
            // Non-Frontier specific nor Ethereum precompiles :
            a if a == hash(1024) => Some(Sha3FIPS256::<T, ()>::execute(handle)),
            a if a == hash(1025) => Some(Dispatch::<T>::execute(handle)),
            a if a == hash(1026) => Some(ECRecoverPublicKey::execute(handle)),

            // DBC specific precompiles
            a if a == hash(2048) => Some(Bridge::<T>::execute(handle)),
            a if a == hash(2049) => Some(DBCPrice::<T>::execute(handle)),
            a if a == hash(2050) => Some(DLCPrice::<T>::execute(handle)),
            a if a == hash(2051) => Some(MachineInfo::<T>::execute(handle)),
            a if a == hash(2096) => Some(AgentTask::<T>::execute(handle)),
            a if a == hash(2098) => Some(ComputePoolPrecompile::<T>::execute(handle)),
            a if a == hash(2099) => Some(AttestationPrecompile::<T>::execute(handle)),
            a if a == hash(2100) => Some(X402SettlementPrecompile::<T>::execute(handle)),

            _ => None,
        }
    }

    fn is_precompile(&self, address: H160, _gas: u64) -> IsPrecompileResult {
        IsPrecompileResult::Answer {
            is_precompile: Self::used_addresses().contains(&address),
            extra_cost: 0,
        }
    }
}

fn hash(a: u64) -> H160 {
    H160::from_low_u64_be(a)
}
