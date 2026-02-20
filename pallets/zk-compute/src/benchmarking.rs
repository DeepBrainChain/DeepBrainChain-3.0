#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as ZkCompute;
use frame_benchmarking::v1::whitelisted_caller;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use sp_runtime::traits::One;

fn setup_pending_task<T: Config>(miner: T::AccountId) -> T::TaskId {
    let _ = T::Currency::deposit_creating(&miner, T::SubmissionDeposit::get() + T::BaseReward::get() + T::SubmissionDeposit::get());
    let task_id = NextTaskId::<T>::get();
    let _ = ZkCompute::<T>::submit_proof(
        RawOrigin::Signed(miner).into(),
        vec![1u8; 8],
        (8, 8, 8),
        120,
        1,
    );
    task_id
}

frame_benchmarking::v1::benchmarks! {
    submit_proof {
        let miner: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let _ = T::Currency::deposit_creating(&miner, T::SubmissionDeposit::get() + T::BaseReward::get());
    }: _(RawOrigin::Signed(miner), vec![1u8; 8], (8u32, 8u32, 8u32), 120u32, 42u64)
    verify {
        assert_eq!(NextTaskId::<T>::get(), One::one());
    }

    verify_task {
        let miner: T::AccountId = whitelisted_caller();
        let verifier: T::AccountId = frame_benchmarking::v1::account("verifier", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let task_id = setup_pending_task::<T>(miner);
    }: _(RawOrigin::Signed(verifier), task_id)
    verify {
        let task = Tasks::<T>::get(task_id).unwrap();
        assert!(!matches!(task.status, ZkVerificationStatus::Pending));
    }

    claim_reward {
        let miner: T::AccountId = whitelisted_caller();
        let verifier: T::AccountId = frame_benchmarking::v1::account("verifier", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let task_id = setup_pending_task::<T>(miner.clone());
        let _ = T::Currency::deposit_creating(&ZkCompute::<T>::account_id(), T::BaseReward::get() + T::SubmissionDeposit::get());
        let _ = ZkCompute::<T>::verify_task(RawOrigin::Signed(verifier).into(), task_id);
    }: _(RawOrigin::Signed(miner), task_id)
    verify {
        let task = Tasks::<T>::get(task_id).unwrap();
        assert!(task.reward_claimed);
    }
}
