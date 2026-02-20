#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as X402Settlement;
use frame_benchmarking::v1::whitelisted_caller;
use frame_support::traits::Get;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use sp_core::H256;
use frame_system::pallet_prelude::BlockNumberFor;

fn setup_intent<T: Config>(merchant: T::AccountId, miner: T::AccountId) -> u64 {
    let _ = T::Currency::deposit_creating(&merchant, 1_000_000_000u128);
    let intent_id = NextIntentId::<T>::get();
    let _ = X402Settlement::<T>::submit_payment_intent(
        RawOrigin::Signed(merchant).into(),
        miner,
        1_000u128,
        1,
        H256::from_low_u64_be(1),
        vec![1u8; 8],
    );
    intent_id
}

frame_benchmarking::v1::benchmarks! {
    submit_payment_intent {
        let merchant: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let _ = T::Currency::deposit_creating(&merchant, 1_000_000_000u128);
    }: _(RawOrigin::Signed(merchant), miner, 1_000u128, 1u64, H256::from_low_u64_be(1), vec![1u8; 8])
    verify {
        assert_eq!(NextIntentId::<T>::get(), 1);
    }

    verify_settlement {
        let merchant: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let intent_id = setup_intent::<T>(merchant, miner);
        let facilitator = T::FacilitatorAccount::get();
    }: _(RawOrigin::Signed(facilitator), intent_id)
    verify {
        let intent = PaymentIntents::<T>::get(intent_id).unwrap();
        assert!(matches!(intent.status, PaymentIntentStatus::Verified));
    }

    finalize_settlement {
        let merchant: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let intent_id = setup_intent::<T>(merchant.clone(), miner);
        let facilitator = T::FacilitatorAccount::get();
        let _ = X402Settlement::<T>::verify_settlement(RawOrigin::Signed(facilitator.clone()).into(), intent_id);
        let settle_block: BlockNumberFor<T> = 1u32.into();
        let settle_block = settle_block + T::SettlementDelay::get();
        frame_system::Pallet::<T>::set_block_number(settle_block);
    }: _(RawOrigin::Signed(merchant), intent_id)
    verify {
        let intent = PaymentIntents::<T>::get(intent_id).unwrap();
        assert!(matches!(intent.status, PaymentIntentStatus::Settled));
    }

    fail_payment_intent {
        let merchant: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let intent_id = setup_intent::<T>(merchant, miner);
        let facilitator = T::FacilitatorAccount::get();
    }: _(RawOrigin::Signed(facilitator), intent_id)
    verify {
        let intent = PaymentIntents::<T>::get(intent_id).unwrap();
        assert!(matches!(intent.status, PaymentIntentStatus::Failed));
    }
}
