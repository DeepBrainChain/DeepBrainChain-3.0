#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as TaskMode;
use frame_benchmarking::v1::whitelisted_caller;
use frame_support::traits::Currency;
use frame_system::RawOrigin;

fn setup_task_definition<T: Config>(admin: T::AccountId) -> u64 {
    let task_id = NextTaskId::<T>::get();
    let _ = TaskMode::<T>::create_task_definition(
        RawOrigin::Signed(admin).into(),
        vec![1u8; 8],
        vec![1u8; 4],
        100,
        200,
        10_000,
        vec![2u8; 16],
    );
    task_id
}

frame_benchmarking::v1::benchmarks! {
    create_task_definition {
        let caller: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::set_block_number(1u32.into());
    }: _(RawOrigin::Signed(caller), vec![1u8; 8], vec![1u8; 4], 100u128, 200u128, 10_000u64, vec![2u8; 16])
    verify {
        assert_eq!(NextTaskId::<T>::get(), 1);
    }

    update_task_definition {
        let caller: T::AccountId = whitelisted_caller();
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let task_id = setup_task_definition::<T>(caller.clone());
    }: _(RawOrigin::Signed(caller), task_id, Some(120u128), Some(240u128), Some(20_000u64), Some(false))
    verify {
        let task = TaskDefinitions::<T>::get(task_id).unwrap();
        assert_eq!(task.max_tokens_per_request, 20_000u64);
        assert!(!task.is_active);
    }

    create_task_order {
        let customer: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let task_id = setup_task_definition::<T>(customer.clone());
        let _ = T::Currency::deposit_creating(&customer, 1_000_000_000u128);
    }: _(RawOrigin::Signed(customer), task_id, miner.clone(), 1_000u64, 1_000u64)
    verify {
        assert_eq!(NextOrderId::<T>::get(), 1);
        let order = TaskOrders::<T>::get(0).unwrap();
        assert_eq!(order.miner, miner);
    }

    mark_order_completed {
        let customer: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let task_id = setup_task_definition::<T>(customer.clone());
        let _ = T::Currency::deposit_creating(&customer, 1_000_000_000u128);
        let _ = TaskMode::<T>::create_task_order(RawOrigin::Signed(customer).into(), task_id, miner.clone(), 1_000, 1_000);
    }: _(RawOrigin::Signed(miner), 0u64, [7u8; 32])
    verify {
        let order = TaskOrders::<T>::get(0).unwrap();
        assert!(matches!(order.status, TaskOrderStatus::Completed));
    }

    settle_task_order {
        let customer: T::AccountId = whitelisted_caller();
        let miner: T::AccountId = frame_benchmarking::v1::account("miner", 0, 0);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let task_id = setup_task_definition::<T>(customer.clone());
        let _ = T::Currency::deposit_creating(&customer, 1_000_000_000u128);
        let _ = TaskMode::<T>::create_task_order(RawOrigin::Signed(customer.clone()).into(), task_id, miner.clone(), 1_000, 1_000);
        let _ = TaskMode::<T>::mark_order_completed(RawOrigin::Signed(miner).into(), 0, [7u8; 32]);
    }: _(RawOrigin::Signed(customer), 0u64, Some([8u8; 32]))
    verify {
        let order = TaskOrders::<T>::get(0).unwrap();
        assert!(matches!(order.status, TaskOrderStatus::Settled));
    }
}
