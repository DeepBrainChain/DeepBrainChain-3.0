use crate::{
    mock::*,
    pallet::{Error, PoolStatus, TaskDimensions, TaskPriority, TaskStatus, ComplaintStatus},
};
use frame_support::{assert_noop, assert_ok, traits::Hooks, BoundedVec};

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
            None,
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
                None,
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
                None,
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
            None,
        ));

        // Pool owner submits proof (no verification_result — just proof hash)
        assert_ok!(ComputePoolScheduler::submit_proof(
            RuntimeOrigin::signed(1),
            0,  // task_id
            [42u8; 32],  // proof_hash
        ));

        let task = ComputePoolScheduler::tasks(0).unwrap();
        // Proof submitted but NOT yet verified
        assert_eq!(task.status, TaskStatus::ProofSubmitted);
        assert_eq!(task.proof_hash, Some([42u8; 32]));
        assert_eq!(task.verification_result, None);
    });
}

#[test]
fn verify_proof_works() {
    new_test_ext().execute_with(|| {
        // Account 1 = pool owner, Account 2 = task user, Account 3 = independent verifier
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));
        assert_ok!(ComputePoolScheduler::submit_task(
            RuntimeOrigin::signed(2), dims(), TaskPriority::Normal, None,
        ));
        assert_ok!(ComputePoolScheduler::submit_proof(
            RuntimeOrigin::signed(1), 0, [42u8; 32],
        ));

        // Independent verifier (account 3) approves the proof
        assert_ok!(ComputePoolScheduler::verify_proof(
            RuntimeOrigin::signed(3), 0, true,
        ));

        let task = ComputePoolScheduler::tasks(0).unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.verification_result, Some(true));
    });
}

#[test]
fn verify_proof_reject_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));
        assert_ok!(ComputePoolScheduler::submit_task(
            RuntimeOrigin::signed(2), dims(), TaskPriority::Normal, None,
        ));
        assert_ok!(ComputePoolScheduler::submit_proof(
            RuntimeOrigin::signed(1), 0, [42u8; 32],
        ));

        // Independent verifier rejects the proof
        assert_ok!(ComputePoolScheduler::verify_proof(
            RuntimeOrigin::signed(3), 0, false,
        ));

        let task = ComputePoolScheduler::tasks(0).unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(task.verification_result, Some(false));
    });
}

#[test]
fn self_verification_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));
        assert_ok!(ComputePoolScheduler::submit_task(
            RuntimeOrigin::signed(2), dims(), TaskPriority::Normal, None,
        ));
        assert_ok!(ComputePoolScheduler::submit_proof(
            RuntimeOrigin::signed(1), 0, [42u8; 32],
        ));

        // Pool owner (account 1) tries to verify their own proof — MUST FAIL
        assert_noop!(
            ComputePoolScheduler::verify_proof(RuntimeOrigin::signed(1), 0, true),
            Error::<Test>::SelfVerificationNotAllowed
        );

        // Task should still be in ProofSubmitted state
        let task = ComputePoolScheduler::tasks(0).unwrap();
        assert_eq!(task.status, TaskStatus::ProofSubmitted);
    });
}

#[test]
fn auto_verify_on_timeout() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));
        assert_ok!(ComputePoolScheduler::submit_task(
            RuntimeOrigin::signed(2), dims(), TaskPriority::Normal, None,
        ));
        assert_ok!(ComputePoolScheduler::submit_proof(
            RuntimeOrigin::signed(1), 0, [42u8; 32],
        ));

        let task = ComputePoolScheduler::tasks(0).unwrap();
        assert_eq!(task.status, TaskStatus::ProofSubmitted);

        // Advance past VerificationTimeout (3 blocks in mock)
        run_to_block(5); // block 1 + 3 + 1 = past timeout

        // Trigger on_initialize which should auto-approve
        ComputePoolScheduler::on_initialize(5);

        let task = ComputePoolScheduler::tasks(0).unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.verification_result, Some(true));
    });
}

fn setup_default_pool() {
    assert_ok!(ComputePoolScheduler::register_pool(
        RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
    ));
}

#[test]
fn staking_works() {
    new_test_ext().execute_with(|| {
        setup_default_pool();
        assert_ok!(ComputePoolScheduler::stake_to_pool(RuntimeOrigin::signed(2), 0, 5_000));
        assert_eq!(ComputePoolScheduler::pool_stakes(0, 2), 5_000);
        assert_eq!(ComputePoolScheduler::total_pool_stake(0), 5_000);
        assert_ok!(ComputePoolScheduler::unstake_from_pool(RuntimeOrigin::signed(2), 0, 3_000));
        assert_eq!(ComputePoolScheduler::pool_stakes(0, 2), 2_000);
        assert_eq!(ComputePoolScheduler::total_pool_stake(0), 2_000);
    });
}


// ============================================================
// Complaint Mechanism Tests
// ============================================================

/// Helper: register a pool, submit+complete a task with verified proof
fn setup_completed_task() -> (u64, u64) {
    // Account 1 = pool owner, Account 2 = task user
    assert_ok!(ComputePoolScheduler::register_pool(
        RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
    ));
    assert_ok!(ComputePoolScheduler::submit_task(
        RuntimeOrigin::signed(2), dims(), TaskPriority::Normal, None,
    ));
    assert_ok!(ComputePoolScheduler::submit_proof(
        RuntimeOrigin::signed(1), 0, [42u8; 32],
    ));
    // Independent verifier approves
    assert_ok!(ComputePoolScheduler::verify_proof(
        RuntimeOrigin::signed(3), 0, true,
    ));
    let task = ComputePoolScheduler::tasks(0).unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.verification_result, Some(true));
    (0, 0) // (pool_id, task_id)
}

#[test]
fn file_complaint_works() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();
        let balance_before = Balances::free_balance(2);

        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2),
            task_id,
            b"Poor quality output".to_vec(),
        ));

        let complaint = ComputePoolScheduler::complaints(0).unwrap();
        assert_eq!(complaint.complainant, 2);
        assert_eq!(complaint.pool_id, 0);
        assert_eq!(complaint.task_id, task_id);
        assert_eq!(complaint.status, ComplaintStatus::Open);
        assert_eq!(complaint.deposit, 100); // ComplaintDeposit

        // Deposit should be reserved
        assert_eq!(Balances::free_balance(2), balance_before - 100);

        // TaskComplaint mapping
        assert_eq!(ComputePoolScheduler::task_complaint(task_id), Some(0));

        // PoolOpenComplaints incremented
        assert_eq!(ComputePoolScheduler::pool_open_complaints(0), 1);
    });
}

#[test]
fn file_complaint_not_task_user_fails() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        // Account 3 is not the task user
        assert_noop!(
            ComputePoolScheduler::file_complaint(
                RuntimeOrigin::signed(3),
                task_id,
                b"Bad result".to_vec(),
            ),
            Error::<Test>::NotTaskUser
        );
    });
}

#[test]
fn file_complaint_duplicate_fails() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2), task_id, b"Bad".to_vec(),
        ));

        assert_noop!(
            ComputePoolScheduler::file_complaint(
                RuntimeOrigin::signed(2), task_id, b"Bad again".to_vec(),
            ),
            Error::<Test>::ComplaintAlreadyFiled
        );
    });
}

#[test]
fn file_complaint_incomplete_task_fails() {
    new_test_ext().execute_with(|| {
        // Register pool and submit task but don't complete it
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));
        assert_ok!(ComputePoolScheduler::submit_task(
            RuntimeOrigin::signed(2), dims(), TaskPriority::Normal, None,
        ));

        // Task is in Computing state, not Completed
        assert_noop!(
            ComputePoolScheduler::file_complaint(
                RuntimeOrigin::signed(2), 0, b"Bad".to_vec(),
            ),
            Error::<Test>::InvalidTaskState
        );
    });
}

#[test]
fn file_complaint_too_many_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(ComputePoolScheduler::register_pool(
            RuntimeOrigin::signed(1), gpu_model(), 24, true, 130, 100,
        ));

        // Max is 10, fill up complaints by directly setting storage
        crate::PoolOpenComplaints::<Test>::insert(0u64, 10u32);

        // Submit and complete a task
        assert_ok!(ComputePoolScheduler::submit_task(
            RuntimeOrigin::signed(2), dims(), TaskPriority::Normal, None,
        ));
        assert_ok!(ComputePoolScheduler::submit_proof(
            RuntimeOrigin::signed(1), 0, [42u8; 32],
        ));
        assert_ok!(ComputePoolScheduler::verify_proof(
            RuntimeOrigin::signed(3), 0, true,
        ));

        assert_noop!(
            ComputePoolScheduler::file_complaint(
                RuntimeOrigin::signed(2), 0, b"Bad".to_vec(),
            ),
            Error::<Test>::TooManyOpenComplaints
        );
    });
}

#[test]
fn resolve_complaint_valid() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2), task_id, b"Bad result".to_vec(),
        ));

        let balance_before = Balances::free_balance(2);

        // Root resolves as valid
        assert_ok!(ComputePoolScheduler::resolve_complaint(
            RuntimeOrigin::root(), 0, true,
        ));

        let complaint = ComputePoolScheduler::complaints(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::ResolvedValid);
        assert!(complaint.resolved_at.is_some());

        // Complainant deposit unreserved
        assert!(Balances::free_balance(2) > balance_before);

        // PendingComplaintSlash should exist
        assert!(ComputePoolScheduler::pending_complaint_slash(0).is_some());

        // Pool reputation decreased by 5
        let pool = ComputePoolScheduler::pools(0).unwrap();
        // Pool reputation: 80 initial + 1 task completion - 5 complaint = 76
        assert_eq!(pool.reputation, 76);

        // Open complaints decremented
        assert_eq!(ComputePoolScheduler::pool_open_complaints(0), 0);
    });
}

#[test]
fn resolve_complaint_invalid() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2), task_id, b"Bad result".to_vec(),
        ));

        let pool_owner_balance_before = Balances::free_balance(1);

        // Root resolves as invalid
        assert_ok!(ComputePoolScheduler::resolve_complaint(
            RuntimeOrigin::root(), 0, false,
        ));

        let complaint = ComputePoolScheduler::complaints(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::ResolvedInvalid);

        // Pool owner got half the complainant deposit (50)
        assert!(Balances::free_balance(1) > pool_owner_balance_before);

        // No pending slash
        assert!(ComputePoolScheduler::pending_complaint_slash(0).is_none());
    });
}

#[test]
fn resolve_complaint_already_resolved_fails() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2), task_id, b"Bad".to_vec(),
        ));
        assert_ok!(ComputePoolScheduler::resolve_complaint(
            RuntimeOrigin::root(), 0, true,
        ));

        // Try resolving again
        assert_noop!(
            ComputePoolScheduler::resolve_complaint(RuntimeOrigin::root(), 0, false),
            Error::<Test>::ComplaintNotOpen
        );
    });
}

#[test]
fn cancel_complaint_works() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        let balance_before = Balances::free_balance(2);
        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2), task_id, b"Bad".to_vec(),
        ));

        // Balance decreased by deposit
        assert_eq!(Balances::free_balance(2), balance_before - 100);

        assert_ok!(ComputePoolScheduler::cancel_complaint(
            RuntimeOrigin::signed(2), 0,
        ));

        let complaint = ComputePoolScheduler::complaints(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::Cancelled);

        // Got 90% back (90 out of 100)
        assert_eq!(Balances::free_balance(2), balance_before - 10);

        // Open complaints decremented
        assert_eq!(ComputePoolScheduler::pool_open_complaints(0), 0);
    });
}

#[test]
fn cancel_complaint_not_owner_fails() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2), task_id, b"Bad".to_vec(),
        ));

        // Account 3 is not the complainant
        assert_noop!(
            ComputePoolScheduler::cancel_complaint(RuntimeOrigin::signed(3), 0),
            Error::<Test>::NotComplainant
        );
    });
}

#[test]
fn appeal_complaint_works() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2), task_id, b"Bad result".to_vec(),
        ));

        // Resolve as valid (creates PendingComplaintSlash)
        assert_ok!(ComputePoolScheduler::resolve_complaint(
            RuntimeOrigin::root(), 0, true,
        ));
        assert!(ComputePoolScheduler::pending_complaint_slash(0).is_some());

        // Pool owner appeals within grace period
        assert_ok!(ComputePoolScheduler::appeal_complaint(
            RuntimeOrigin::signed(1), 0,
        ));

        let complaint = ComputePoolScheduler::complaints(0).unwrap();
        assert_eq!(complaint.status, ComplaintStatus::Appealed);

        // PendingComplaintSlash removed
        assert!(ComputePoolScheduler::pending_complaint_slash(0).is_none());
    });
}

#[test]
fn execute_pending_slash_on_initialize() {
    new_test_ext().execute_with(|| {
        let (_pool_id, task_id) = setup_completed_task();

        assert_ok!(ComputePoolScheduler::file_complaint(
            RuntimeOrigin::signed(2), task_id, b"Bad result".to_vec(),
        ));

        // Resolve as valid
        assert_ok!(ComputePoolScheduler::resolve_complaint(
            RuntimeOrigin::root(), 0, true,
        ));

        // PendingComplaintSlash exists
        assert!(ComputePoolScheduler::pending_complaint_slash(0).is_some());

        let pool_before = ComputePoolScheduler::pools(0).unwrap();
        let deposit_before = pool_before.deposit_held;

        // Advance past SlashGracePeriod (5 blocks in mock)
        run_to_block(10);
        ComputePoolScheduler::on_initialize(10);

        // Slash should have been executed
        assert!(ComputePoolScheduler::pending_complaint_slash(0).is_none());

        // Pool deposit_held should have decreased
        let pool_after = ComputePoolScheduler::pools(0).unwrap();
        assert!(pool_after.deposit_held < deposit_before);
    });
}
