use crate::mock::{
	new_test_ext, Balances, RuntimeOrigin, SubmissionDeposit, System, Test, VerificationTimeout,
	ZkCompute,
};
use crate::{Error, Event, MinerScores, ZkVerificationStatus};
use frame_support::{assert_noop, assert_ok, traits::ReservableCurrency};

#[test]
fn submit_proof_should_store_task_and_reserve_deposit() {
	new_test_ext().execute_with(|| {
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![1, 2, 3],
			(100, 100, 100),
			120,
			1,
		));

		let task = ZkCompute::tasks(0u64).expect("task exists");
		assert_eq!(task.miner, 1);
		assert_eq!(task.status, ZkVerificationStatus::Pending);
		assert_eq!(task.base_reward, 100);
		assert_eq!(task.multiplier_q100, 120);
		assert_eq!(Balances::reserved_balance(1), SubmissionDeposit::get());
		assert_eq!(ZkCompute::pending_tasks().len(), 1);

		System::assert_last_event(
			Event::<Test>::ProofSubmitted {
				task_id: 0u64,
				miner: 1,
				nonce: 1,
				dimensions: (100, 100, 100),
			}
			.into(),
		);
	});
}

#[test]
fn submit_proof_should_reject_replay_nonce() {
	new_test_ext().execute_with(|| {
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![1],
			(100, 100, 100),
			120,
			9,
		));

		assert_noop!(
			ZkCompute::submit_proof(RuntimeOrigin::signed(1), vec![1], (100, 100, 100), 120, 9,),
			Error::<Test>::NonceAlreadyUsed
		);
	});
}

#[test]
fn submit_proof_should_limit_pending_per_miner() {
	new_test_ext().execute_with(|| {
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![1],
			(100, 100, 100),
			120,
			1,
		));
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![1],
			(100, 100, 100),
			120,
			2,
		));

		assert_noop!(
			ZkCompute::submit_proof(RuntimeOrigin::signed(1), vec![1], (100, 100, 100), 120, 3,),
			Error::<Test>::TooManyPendingTasksForMiner
		);
	});
}

#[test]
fn submit_proof_should_fail_if_score_too_low() {
	new_test_ext().execute_with(|| {
		MinerScores::<Test>::insert(1, 5);
		assert_noop!(
			ZkCompute::submit_proof(RuntimeOrigin::signed(1), vec![1], (100, 100, 100), 120, 1,),
			Error::<Test>::InsufficientMinerScore
		);
	});
}

#[test]
fn verify_task_should_mark_verified_and_increase_score() {
	new_test_ext().execute_with(|| {
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![1, 0, 0],
			(100, 100, 100),
			120,
			1,
		));
		assert_ok!(ZkCompute::verify_task(RuntimeOrigin::signed(2), 0u64));

		let task = ZkCompute::tasks(0u64).expect("task exists");
		assert_eq!(task.status, ZkVerificationStatus::Verified);
		assert_eq!(ZkCompute::pending_tasks().len(), 0);
		assert_eq!(ZkCompute::verified_tasks().len(), 1);
		assert_eq!(ZkCompute::miner_score(1), Some(60));
	});
}

#[test]
fn verify_task_should_mark_failed_and_slash_deposit() {
	new_test_ext().execute_with(|| {
		let pallet_before = Balances::free_balance(ZkCompute::account_id());
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![0, 9, 9],
			(100, 100, 100),
			120,
			1,
		));
		assert_ok!(ZkCompute::verify_task(RuntimeOrigin::signed(2), 0u64));

		let task = ZkCompute::tasks(0u64).expect("task exists");
		assert_eq!(task.status, ZkVerificationStatus::Failed);
		assert_eq!(Balances::reserved_balance(1), 0);
		assert_eq!(Balances::free_balance(ZkCompute::account_id()), pallet_before + SubmissionDeposit::get());
		assert_eq!(ZkCompute::miner_score(1), Some(30));
	});
}

#[test]
fn verify_task_should_fail_after_timeout() {
	new_test_ext().execute_with(|| {
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![1, 0, 0],
			(100, 100, 100),
			120,
			1,
		));
		System::set_block_number(VerificationTimeout::get() + 10);
		assert_ok!(ZkCompute::verify_task(RuntimeOrigin::signed(2), 0u64));

		let task = ZkCompute::tasks(0u64).expect("task exists");
		assert_eq!(task.status, ZkVerificationStatus::Failed);
	});
}

#[test]
fn claim_reward_should_pay_and_unreserve_deposit() {
	new_test_ext().execute_with(|| {
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![1, 7, 7],
			(100, 100, 100),
			120,
			1,
		));
		assert_ok!(ZkCompute::verify_task(RuntimeOrigin::signed(2), 0u64));

		let free_before = Balances::free_balance(1);
		assert_ok!(ZkCompute::claim_reward(RuntimeOrigin::signed(1), 0u64));

		let task = ZkCompute::tasks(0u64).expect("task exists");
		assert!(task.reward_claimed);
		assert_eq!(Balances::reserved_balance(1), 0);
		assert_eq!(Balances::free_balance(1), free_before + 120 + SubmissionDeposit::get());

		assert_noop!(
			ZkCompute::claim_reward(RuntimeOrigin::signed(1), 0u64),
			Error::<Test>::RewardAlreadyClaimed
		);
	});
}

#[test]
fn claim_reward_should_fail_for_unverified_task() {
	new_test_ext().execute_with(|| {
		assert_ok!(ZkCompute::submit_proof(
			RuntimeOrigin::signed(1),
			vec![1, 2, 3],
			(100, 100, 100),
			120,
			1,
		));
		assert_noop!(
			ZkCompute::claim_reward(RuntimeOrigin::signed(1), 0u64),
			Error::<Test>::TaskNotVerified
		);
	});
}
