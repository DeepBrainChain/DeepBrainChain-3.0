use crate::{
    mock::*,
    pallet::{Error, PoolStatus, TaskDimensions, TaskPriority, TaskStatus},
};
use frame_support::{assert_noop, assert_ok, BoundedVec};

fn gpu_model() -> BoundedVec<u8, <Test as crate::Config>::MaxGpuModelLen> {
    b"RTX-4090".to_vec().try_into().unwrap()
}

fn dims() -> TaskDimensions {
    TaskDimensions { m: 8, n: 8, k: 8 }
}

#[test]
fn mock_runtime_should_bootstrap() {
    new_test_ext().execute_with(|| {
        assert_eq!(System::block_number(), 1);
    });
}

#[test]
fn register_pool_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1),
            gpu_model(),
            24,     // 24GB GPU memory
            true,   // has NVLink
            130,    // NVLink efficiency (must be 120-150)
            100,    // price per task
        ));
        let pool = ComputePoolScheduler::pools(0).unwrap();
        assert_eq!(pool.owner, 1);
        assert_eq!(pool.gpu_memory, 24);
        assert!(pool.has_nvlink);
        assert_eq!(pool.reputation, 80); // InitialReputation capped at 100
        assert_eq!(pool.status, PoolStatus::Active);
    });
}

#[test]
fn register_pool_duplicate_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));
        assert_noop!(
            ComputePoolScheduler::register_pool(
                RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
            ),
            Error::<Test>::PoolAlreadyExists
        );
    });
}

#[test]
fn register_pool_zero_memory_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ComputePoolScheduler::register_pool(
                RuntimeOrigin::signed(1), gpu_model(), 0, false, 100, 100,
            ),
            Error::<Test>::InvalidDimensions
        );
    });
}

#[test]
fn register_pool_zero_price_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ComputePoolScheduler::register_pool(
                RuntimeOrigin::signed(1), gpu_model(), 24, false, 100, 0,
            ),
            Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn submit_task_works() {
    new_test_ext().execute_with(|| {
        // Register a pool first
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));

        // Submit a task (from different account)
        assert_ok!(ComputePoolScheduler::submit_task(
            RuntimeOrigin::signed(2),
            dims(),
            TaskPriority::Normal,
        ));

        let task = ComputePoolScheduler::tasks(0).unwrap();
        assert_eq!(task.user, 2);
        assert_eq!(task.pool_id, 0);
        assert_eq!(task.status, TaskStatus::Computing);
    });
}

#[test]
fn submit_task_no_pool_fails() {
    new_test_ext().execute_with(|| {
        // No pools registered
        assert_noop!(
            ComputePoolScheduler::submit_task(
                RuntimeOrigin::signed(2),
                dims(),
                TaskPriority::Normal,
            ),
            Error::<Test>::NoAvailablePool
        );
    });
}

#[test]
fn submit_task_invalid_dimensions_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));
        assert_noop!(
            ComputePoolScheduler::submit_task(
                RuntimeOrigin::signed(2),
                TaskDimensions { m: 0, n: 64, k: 64 },
                TaskPriority::Normal,
            ),
            Error::<Test>::InvalidDimensions
        );
    });
}

#[test]
fn deregister_pool_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, false, 100, 100,
        ));
        assert_ok!(ComputePoolScheduler::deregister_pool(
            RuntimeOrigin::signed(1), 0,
        ));
        // Pool may be removed from storage or marked as deregistered
        match ComputePoolScheduler::pools(0) {
            Some(pool) => assert_eq!(pool.status, PoolStatus::Deregistered),
            None => {} // Pool was removed from storage
        }
    });
}

#[test]
fn deregister_pool_not_owner_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, false, 100, 100,
        ));
        assert_noop!(
            ComputePoolScheduler::deregister_pool(RuntimeOrigin::signed(2), 0),
            Error::<Test>::NotPoolOwner
        );
    });
}

#[test]
fn submit_proof_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));
        assert_ok!(ComputePoolScheduler::submit_task(
            RuntimeOrigin::signed(2), dims(), TaskPriority::Normal,
        ));

        // Pool owner submits proof
        assert_ok!(ComputePoolScheduler::submit_proof(
            RuntimeOrigin::signed(1),
            0,  // task_id
            [42u8; 32],  // proof_hash
            true,  // verification_result
        ));

        let task = ComputePoolScheduler::tasks(0).unwrap();
        // With verification_result=true, task goes to Completed
        assert_eq!(task.status, TaskStatus::Completed);
    });
}
