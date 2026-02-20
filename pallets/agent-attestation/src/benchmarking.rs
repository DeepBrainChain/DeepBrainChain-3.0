#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v1::{account, benchmarks, whitelisted_caller};
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use sp_core::H256;

benchmarks! {
    register_node {
        let caller: T::AccountId = whitelisted_caller();
        let gpu_uuid = b"GPU-1234".to_vec();
        let tflops = 100u32;
    }: _(RawOrigin::Signed(caller), gpu_uuid, tflops)

    heartbeat {
        let caller: T::AccountId = whitelisted_caller();
        Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            b"GPU-1234".to_vec(),
            100u32,
        )?;

        let now = frame_system::Pallet::<T>::block_number();
        frame_system::Pallet::<T>::set_block_number(now + T::HeartbeatInterval::get());
    }: _(RawOrigin::Signed(caller))

    submit_attestation {
        let caller: T::AccountId = whitelisted_caller();
        Pallet::<T>::register_node(
            RawOrigin::Signed(caller.clone()).into(),
            b"GPU-1234".to_vec(),
            100u32,
        )?;

        let amount = T::AttestationDeposit::get().saturating_mul(10u32.into());
        T::Currency::make_free_balance_be(&caller, amount);

        let task_id = 1u64;
        let result_hash = H256::repeat_byte(1);
        let model_id = b"llama-70b".to_vec();
        let input_tokens = 1000u64;
        let output_tokens = 500u64;
    }: _(RawOrigin::Signed(caller), task_id, result_hash, model_id, input_tokens, output_tokens)

    challenge_attestation {
        let attester: T::AccountId = whitelisted_caller();
        let challenger: T::AccountId = account("challenger", 0, 0);

        Pallet::<T>::register_node(
            RawOrigin::Signed(attester.clone()).into(),
            b"GPU-1234".to_vec(),
            100u32,
        )?;

        let amount = T::AttestationDeposit::get().saturating_mul(10u32.into());
        T::Currency::make_free_balance_be(&attester, amount);

        Pallet::<T>::submit_attestation(
            RawOrigin::Signed(attester).into(),
            1,
            H256::repeat_byte(1),
            b"llama-70b".to_vec(),
            1000,
            500,
        )?;
    }: _(RawOrigin::Signed(challenger), 0u64)

    confirm_attestation {
        let attester: T::AccountId = whitelisted_caller();

        Pallet::<T>::register_node(
            RawOrigin::Signed(attester.clone()).into(),
            b"GPU-1234".to_vec(),
            100u32,
        )?;

        let amount = T::AttestationDeposit::get().saturating_mul(10u32.into());
        T::Currency::make_free_balance_be(&attester, amount);

        Pallet::<T>::submit_attestation(
            RawOrigin::Signed(attester.clone()).into(),
            1,
            H256::repeat_byte(1),
            b"llama-70b".to_vec(),
            1000,
            500,
        )?;

        let now = frame_system::Pallet::<T>::block_number();
        frame_system::Pallet::<T>::set_block_number(now + T::ChallengeWindow::get() + 1u32.into());
    }: _(RawOrigin::Signed(attester), 0u64)

    resolve_challenge {
        let attester: T::AccountId = whitelisted_caller();
        let challenger: T::AccountId = account("challenger", 0, 0);

        Pallet::<T>::register_node(
            RawOrigin::Signed(attester.clone()).into(),
            b"GPU-1234".to_vec(),
            100u32,
        )?;

        let amount = T::AttestationDeposit::get().saturating_mul(10u32.into());
        T::Currency::make_free_balance_be(&attester, amount);

        Pallet::<T>::submit_attestation(
            RawOrigin::Signed(attester).into(),
            1,
            H256::repeat_byte(1),
            b"llama-70b".to_vec(),
            1000,
            500,
        )?;

        Pallet::<T>::challenge_attestation(
            RawOrigin::Signed(challenger).into(),
            0,
        )?;
    }: _(RawOrigin::Root, 0u64, true)
}
