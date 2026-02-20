#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v1::{account, benchmarks, whitelisted_caller};
use frame_support::traits::{Currency, Get, ReservableCurrency};
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use sp_runtime::traits::Saturating;

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
    let caller: T::AccountId = account(name, index, 0);
    let amount = T::PoolDeposit::get().saturating_mul(1_000_000u32.into());
    T::Currency::make_free_balance_be(&caller, amount);
    caller
}

fn create_pool<T: Config>(owner: &T::AccountId) -> PoolId {
    let gpu_model: BoundedVec<u8, T::MaxGpuModelLen> = b"RTX4090".to_vec().try_into().unwrap();
    Pallet::<T>::register_pool(
        RawOrigin::Signed(owner.clone()).into(),
        gpu_model,
        16_384u32,
        true,
        80u32,
        1000u32.into(),
    ).expect("register_pool failed");
    NextPoolId::<T>::get().saturating_sub(1)
}

/// Create a task and force it into Computing status with escrow set up.
fn create_computing_task<T: Config>(owner: &T::AccountId, user: &T::AccountId, pool_id: PoolId) -> TaskId {
    let task_id = NextTaskId::<T>::get();
    let now = frame_system::Pallet::<T>::block_number();
    let reward: BalanceOf<T> = 500u32.into();
    let task_deposit = T::TaskDeposit::get();

    // Reserve from user
    let total_reserved = reward.saturating_add(task_deposit);
    T::Currency::reserve(user, total_reserved).expect("reserve failed");

    Tasks::<T>::insert(task_id, ComputeTask {
        task_id,
        user: user.clone(),
        pool_id,
        dimensions: TaskDimensions { m: 128, n: 128, k: 128 },
        priority: TaskPriority::Normal,
        status: TaskStatus::Computing,
        submitted_at: now,
        proof_hash: None,
        verification_result: None,
        reward_amount: Some(reward),
    });

    TaskEscrowStore::<T>::insert(task_id, TaskEscrow {
        user: user.clone(),
        pool_owner: owner.clone(),
        reward_amount: reward,
        task_deposit,
        claimed: false,
    });

    ActiveTaskCount::<T>::mutate(pool_id, |v| *v = v.saturating_add(1));
    NextTaskId::<T>::put(task_id.saturating_add(1));

    task_id
}

benchmarks! {
    register_pool {
        let caller: T::AccountId = funded_account::<T>("caller", 0);
        let gpu_model: BoundedVec<u8, T::MaxGpuModelLen> = b"RTX4090".to_vec().try_into().unwrap();
        let gpu_memory = 16_384u32;
        let has_nvlink = true;
        let nvlink_efficiency = 80u32;
        let price: BalanceOf<T> = 1000u32.into();
    }: _(RawOrigin::Signed(caller), gpu_model, gpu_memory, has_nvlink, nvlink_efficiency, price)

    update_pool_config {
        let caller: T::AccountId = funded_account::<T>("caller", 0);
        let pool_id = create_pool::<T>(&caller);

        let gpu_model: BoundedVec<u8, T::MaxGpuModelLen> = b"A100".to_vec().try_into().unwrap();
        let gpu_memory = 32_768u32;
        let has_nvlink = true;
        let nvlink_efficiency = 90u32;
        let price: BalanceOf<T> = 1200u32.into();
    }: _(RawOrigin::Signed(caller), pool_id, gpu_model, gpu_memory, has_nvlink, nvlink_efficiency, price)

    deregister_pool {
        let caller: T::AccountId = funded_account::<T>("caller", 0);
        let pool_id = create_pool::<T>(&caller);
    }: _(RawOrigin::Signed(caller), pool_id)

    submit_task {
        let _owner: T::AccountId = funded_account::<T>("owner", 0);
        let _pool_id = create_pool::<T>(&_owner);
        let user: T::AccountId = funded_account::<T>("user", 1);
        let dimensions = TaskDimensions { m: 128, n: 128, k: 128 };
        let priority = TaskPriority::Normal;
    }: _(RawOrigin::Signed(user), dimensions, priority)

    submit_proof {
        let owner: T::AccountId = funded_account::<T>("owner", 0);
        let pool_id = create_pool::<T>(&owner);
        let user: T::AccountId = funded_account::<T>("user", 1);
        let task_id = create_computing_task::<T>(&owner, &user, pool_id);

        let proof_hash = [1u8; 32];
        let verification_result = true;
    }: _(RawOrigin::Signed(owner), task_id, proof_hash, verification_result)

    claim_reward {
        let owner: T::AccountId = funded_account::<T>("owner", 0);
        let pool_id = create_pool::<T>(&owner);
        let user: T::AccountId = funded_account::<T>("user", 1);
        let task_id = create_computing_task::<T>(&owner, &user, pool_id);

        // Advance task to Completed with verified result
        Tasks::<T>::mutate(task_id, |maybe_task| {
            if let Some(task) = maybe_task.as_mut() {
                task.status = TaskStatus::Completed;
                task.verification_result = Some(true);
                task.proof_hash = Some([1u8; 32]);
            }
        });
        Rewards::<T>::insert(task_id, BalanceOf::<T>::from(500u32));
    }: _(RawOrigin::Signed(owner), task_id)

    dispute_verification {
        let owner: T::AccountId = funded_account::<T>("owner", 0);
        let pool_id = create_pool::<T>(&owner);
        let user: T::AccountId = funded_account::<T>("user", 1);
        let task_id = create_computing_task::<T>(&owner, &user, pool_id);

        // Advance task to Completed
        Tasks::<T>::mutate(task_id, |maybe_task| {
            if let Some(task) = maybe_task.as_mut() {
                task.status = TaskStatus::Completed;
                task.verification_result = Some(true);
                task.proof_hash = Some([1u8; 32]);
            }
        });
    }: _(RawOrigin::Signed(user), task_id)
}
