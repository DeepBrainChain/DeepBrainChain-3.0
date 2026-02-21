#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::vec;
use crate::Pallet as X402Settlement;
use frame_benchmarking::v1::whitelisted_caller;
use frame_support::traits::Get;
use frame_support::traits::{Currency, ReservableCurrency};
use frame_system::RawOrigin;
use sp_core::H256;
use frame_system::pallet_prelude::BlockNumberFor;
use frame_support::BoundedVec;

/// Set up a payment intent in Pending status with proper fund reservation.
/// Directly inserts storage to avoid calling submit_payment_intent.
fn setup_pending_intent<T: Config>(
    merchant: T::AccountId,
    miner: T::AccountId,
    amount: BalanceOf<T>,
) -> u64 {
    let _ = T::Currency::deposit_creating(&merchant, 10_000_000_000_000_000_000u128);
    T::Currency::reserve(&merchant, amount).expect("reserve failed");

    let intent_id = NextIntentId::<T>::get();
    let block: BlockNumberFor<T> = 1u32.into();
    let sig: BoundedVec<u8, T::MaxSignatureLen> = vec![0u8; 64].try_into().unwrap();

    PaymentIntents::<T>::insert(intent_id, PaymentIntent::<T> {
        intent_id,
        merchant: merchant.clone(),
        miner: miner.clone(),
        amount,
        nonce: 1,
        replay_fingerprint: H256::from_low_u64_be(1),
        facilitator_signature: sig,
        status: PaymentIntentStatus::Pending,
        created_at: block,
        verified_at: None,
        settled_at: None,
        expires_at: block + T::PaymentIntentTTL::get(),
    });

    NextIntentId::<T>::put(intent_id + 1);
    intent_id
}

/// Set up a payment intent in Verified status with proper fund reservation.
fn setup_verified_intent<T: Config>(
    merchant: T::AccountId,
    miner: T::AccountId,
    amount: BalanceOf<T>,
) -> u64 {
    let _ = T::Currency::deposit_creating(&merchant, 10_000_000_000_000_000_000u128);
    T::Currency::reserve(&merchant, amount).expect("reserve failed");

    let intent_id = NextIntentId::<T>::get();
    let block: BlockNumberFor<T> = 1u32.into();
    let sig: BoundedVec<u8, T::MaxSignatureLen> = vec![0u8; 64].try_into().unwrap();

    PaymentIntents::<T>::insert(intent_id, PaymentIntent::<T> {
        intent_id,
        merchant: merchant.clone(),
        miner: miner.clone(),
        amount,
        nonce: 1,
        replay_fingerprint: H256::from_low_u64_be(1),
        facilitator_signature: sig,
        status: PaymentIntentStatus::Verified,
        created_at: block,
        verified_at: Some(block),
        settled_at: None,
        expires_at: block + T::PaymentIntentTTL::get(),
    });

    NextIntentId::<T>::put(intent_id + 1);
    intent_id
}

frame_benchmarking::v1::benchmarks! {
    submit_payment_intent {
        let merchant: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let _ = T::Currency::deposit_creating(&merchant, 10_000_000_000_000_000_000u128);

        let amount: BalanceOf<T> = 1_000u128;
        let nonce: u64 = 1;
        let replay_fingerprint = H256::from_low_u64_be(1);
        // Dummy signature — signature verification is bypassed in runtime-benchmarks
        let sig: sp_std::vec::Vec<u8> = vec![0u8; 64];
    }: _(RawOrigin::Signed(merchant), miner, amount, nonce, replay_fingerprint, sig)
    verify {
        assert_eq!(NextIntentId::<T>::get(), 1);
    }

    verify_settlement {
        let merchant: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let intent_id = setup_pending_intent::<T>(merchant, miner, 1_000u128);
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
        let _ = T::Currency::deposit_creating(&miner, 10_000_000_000_000_000_000u128);
        let intent_id = setup_verified_intent::<T>(merchant.clone(), miner.clone(), 1_000u128);
        // Advance past settlement delay
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
        let intent_id = setup_pending_intent::<T>(merchant, miner, 1_000u128);
        let facilitator = T::FacilitatorAccount::get();
    }: _(RawOrigin::Signed(facilitator), intent_id)
    verify {
        let intent = PaymentIntents::<T>::get(intent_id).unwrap();
        assert!(matches!(intent.status, PaymentIntentStatus::Failed));
    }
}
