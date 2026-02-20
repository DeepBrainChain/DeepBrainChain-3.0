use crate::{
    mock::{new_test_ext, DBCPriceOCW, RuntimeOrigin, System, TaskMode, Test},
    EraStats, MinerTaskStats, TaskOrderStatus,
};
use frame_support::{assert_noop, assert_ok};

fn create_default_task() {
    assert_ok!(TaskMode::create_task_definition(
        RuntimeOrigin::signed(1),
        b"llama3-70b".to_vec(),
        b"v1".to_vec(),
        2_000_000,
        4_000_000,
        10_000,
        b"ipfs://policy".to_vec(),
    ));
}

fn create_default_order() {
    assert_ok!(TaskMode::create_task_order(
        RuntimeOrigin::signed(1),
        0,
        2,
        1_000,
        500,
    ));
}

#[test]
fn create_task_definition_works() {
    new_test_ext().execute_with(|| {
        create_default_task();

        let task = TaskMode::task_definition_of(0).expect("task exists");
        assert_eq!(task.model_id, b"llama3-70b".to_vec());
        assert_eq!(task.version, b"v1".to_vec());
        assert_eq!(task.admin, 1);
        assert_eq!(task.input_price_usd_per_1k, 2_000_000);
        assert_eq!(task.output_price_usd_per_1k, 4_000_000);
        assert_eq!(task.max_tokens_per_request, 10_000);
        assert!(task.is_active);
        assert_eq!(TaskMode::next_task_id(), 1);
    });
}

#[test]
fn update_task_definition_works() {
    new_test_ext().execute_with(|| {
        create_default_task();

        assert_ok!(TaskMode::update_task_definition(
            RuntimeOrigin::signed(1),
            0,
            Some(3_000_000),
            None,
            Some(20_000),
            Some(false),
        ));

        let task = TaskMode::task_definition_of(0).expect("task exists");
        assert_eq!(task.input_price_usd_per_1k, 3_000_000);
        assert_eq!(task.max_tokens_per_request, 20_000);
        assert!(!task.is_active);
    });
}

#[test]
fn update_task_definition_fails_for_non_admin() {
    new_test_ext().execute_with(|| {
        create_default_task();

        assert_noop!(
            TaskMode::update_task_definition(RuntimeOrigin::signed(2), 0, Some(1), None, None, None),
            crate::Error::<Test>::NotAuthorized
        );
    });
}

#[test]
fn create_task_order_success_reserves_balance_and_enters_in_progress() {
    new_test_ext().execute_with(|| {
        create_default_task();
        create_default_order();

        let order = TaskMode::task_order_of(0).expect("order exists");
        assert_eq!(order.customer, 1);
        assert_eq!(order.miner, 2);
        assert_eq!(order.total_dbc_charged, 40_000_000);
        assert_eq!(order.dbc_burned, 6_000_000);
        assert_eq!(order.miner_payout, 34_000_000);
        assert!(matches!(order.status, TaskOrderStatus::InProgress));
        assert_eq!(<Test as crate::Config>::Currency::reserved_balance(1), 40_000_000);
    });
}

#[test]
fn create_task_order_fails_for_inactive_task() {
    new_test_ext().execute_with(|| {
        create_default_task();
        assert_ok!(TaskMode::update_task_definition(
            RuntimeOrigin::signed(1),
            0,
            None,
            None,
            None,
            Some(false),
        ));

        assert_noop!(
            TaskMode::create_task_order(RuntimeOrigin::signed(1), 0, 2, 100, 100),
            crate::Error::<Test>::TaskDefinitionInactive
        );
    });
}

#[test]
fn token_limit_enforced() {
    new_test_ext().execute_with(|| {
        create_default_task();

        assert_noop!(
            TaskMode::create_task_order(RuntimeOrigin::signed(1), 0, 2, 9_000, 2_000),
            crate::Error::<Test>::TokenCountExceedsLimit
        );
    });
}

#[test]
fn price_oracle_failure_rejected() {
    new_test_ext().execute_with(|| {
        create_default_task();
        DBCPriceOCW::set_price(None);

        assert_noop!(
            TaskMode::create_task_order(RuntimeOrigin::signed(1), 0, 2, 100, 100),
            crate::Error::<Test>::PriceOracleUnavailable
        );

        DBCPriceOCW::set_price(Some(2_000_000));
        DBCPriceOCW::set_multiplier(None);

        assert_noop!(
            TaskMode::create_task_order(RuntimeOrigin::signed(1), 0, 2, 100, 100),
            crate::Error::<Test>::PriceOracleUnavailable
        );
    });
}

#[test]
fn create_task_order_insufficient_balance_rejected() {
    new_test_ext().execute_with(|| {
        create_default_task();
        DBCPriceOCW::set_multiplier(Some(10_000_000_000));

        assert_noop!(
            TaskMode::create_task_order(RuntimeOrigin::signed(1), 0, 2, 100_000, 100_000),
            crate::Error::<Test>::TokenCountExceedsLimit
        );

        assert_ok!(TaskMode::update_task_definition(
            RuntimeOrigin::signed(1),
            0,
            None,
            None,
            Some(500_000),
            None,
        ));

        assert_noop!(
            TaskMode::create_task_order(RuntimeOrigin::signed(1), 0, 2, 100_000, 100_000),
            crate::Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn mark_order_completed_requires_miner_and_correct_status() {
    new_test_ext().execute_with(|| {
        create_default_task();
        create_default_order();

        assert_noop!(
            TaskMode::mark_order_completed(RuntimeOrigin::signed(1), 0, [7u8; 32]),
            crate::Error::<Test>::NotAuthorized
        );

        assert_ok!(TaskMode::mark_order_completed(
            RuntimeOrigin::signed(2),
            0,
            [7u8; 32],
        ));

        assert_noop!(
            TaskMode::mark_order_completed(RuntimeOrigin::signed(2), 0, [8u8; 32]),
            crate::Error::<Test>::InvalidOrderStatus
        );
    });
}

#[test]
fn settle_task_order_works_and_updates_stats() {
    new_test_ext().execute_with(|| {
        create_default_task();
        create_default_order();

        assert_ok!(TaskMode::mark_order_completed(
            RuntimeOrigin::signed(2),
            0,
            [1u8; 32],
        ));

        let treasury_before = <Test as crate::Config>::Currency::free_balance(99);
        let miner_before = <Test as crate::Config>::Currency::free_balance(2);

        assert_ok!(TaskMode::settle_task_order(
            RuntimeOrigin::signed(1),
            0,
            None,
        ));

        let order = TaskMode::task_order_of(0).expect("order exists");
        assert!(matches!(order.status, TaskOrderStatus::Settled));
        assert_eq!(<Test as crate::Config>::Currency::reserved_balance(1), 0);
        assert_eq!(
            <Test as crate::Config>::Currency::free_balance(99),
            treasury_before + 6_000_000
        );
        assert_eq!(
            <Test as crate::Config>::Currency::free_balance(2),
            miner_before + 34_000_000
        );

        let era = EraStats::<Test>::get(0);
        assert_eq!(era.total_charged, 40_000_000);
        assert_eq!(era.total_burned, 6_000_000);
        assert_eq!(era.total_miner_payout, 34_000_000);
        assert_eq!(era.completed_orders, 1);

        let miner_stats = MinerTaskStats::<Test>::get(0, 2);
        assert_eq!(miner_stats, (34_000_000, 1));
    });
}

#[test]
fn settle_rejects_non_completed_order() {
    new_test_ext().execute_with(|| {
        create_default_task();
        create_default_order();

        assert_noop!(
            TaskMode::settle_task_order(RuntimeOrigin::signed(1), 0, None),
            crate::Error::<Test>::InvalidOrderStatus
        );
    });
}

#[test]
fn burn_and_payout_split_is_15_85() {
    new_test_ext().execute_with(|| {
        create_default_task();
        create_default_order();

        let order = TaskMode::task_order_of(0).expect("order exists");
        assert_eq!(order.total_dbc_charged, 40_000_000);
        assert_eq!(order.dbc_burned, 6_000_000);
        assert_eq!(order.miner_payout, 34_000_000);
    });
}

#[test]
fn reward_split_70_30_and_miner_reward_share_works() {
    new_test_ext().execute_with(|| {
        create_default_task();

        assert_ok!(TaskMode::create_task_order(
            RuntimeOrigin::signed(1),
            0,
            2,
            1_000,
            500,
        ));
        assert_ok!(TaskMode::mark_order_completed(
            RuntimeOrigin::signed(2),
            0,
            [2u8; 32],
        ));
        assert_ok!(TaskMode::settle_task_order(
            RuntimeOrigin::signed(1),
            0,
            None,
        ));

        System::set_block_number(2);

        assert_ok!(TaskMode::create_task_order(
            RuntimeOrigin::signed(1),
            0,
            3,
            500,
            500,
        ));
        assert_ok!(TaskMode::mark_order_completed(
            RuntimeOrigin::signed(3),
            1,
            [3u8; 32],
        ));
        assert_ok!(TaskMode::settle_task_order(
            RuntimeOrigin::signed(1),
            1,
            None,
        ));

        let (task_pool, rental_pool) = TaskMode::split_era_rewards(1_000_000).expect("split works");
        assert_eq!(task_pool, 700_000);
        assert_eq!(rental_pool, 300_000);

        let miner2_share = TaskMode::miner_reward_share(0, &2, 1_000_000).expect("share exists");
        let miner3_share = TaskMode::miner_reward_share(0, &3, 1_000_000).expect("share exists");

        // miner2 payout: 34_000_000
        // miner3 payout: 25_500_000
        // total: 59_500_000
        assert_eq!(miner2_share, 400_000);
        assert_eq!(miner3_share, 300_000);
    });
}

#[test]
fn settle_allows_attestation_override() {
    new_test_ext().execute_with(|| {
        create_default_task();
        create_default_order();

        assert_ok!(TaskMode::mark_order_completed(
            RuntimeOrigin::signed(2),
            0,
            [9u8; 32],
        ));
        assert_ok!(TaskMode::settle_task_order(
            RuntimeOrigin::signed(1),
            0,
            Some([4u8; 32]),
        ));

        let order = TaskMode::task_order_of(0).expect("order exists");
        assert_eq!(order.attestation_hash, Some([4u8; 32]));
    });
}
