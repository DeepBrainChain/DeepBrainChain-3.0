use crate::mock::*;
use crate::pallet::{AttestationStatus, Error};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

fn gpu_uuid() -> Vec<u8> {
    b"GPU-12345678-abcd-efgh-ijkl-1234567890ab".to_vec()
}

fn model_id() -> Vec<u8> {
    b"gpt-5.3-codex".to_vec()
}

fn model_id_2() -> Vec<u8> {
    b"deepseek-r1".to_vec()
}

#[test]
fn register_node_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        let node = AgentAttestation::node_of(1).unwrap();
        assert_eq!(node.tflops, 120);
        assert!(node.is_active);
    });
}

#[test]
fn register_node_duplicate_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        assert_noop!(
            AgentAttestation::register_node(RuntimeOrigin::signed(1), gpu_uuid(), 120),
            Error::<Test>::NodeAlreadyRegistered
        );
    });
}

#[test]
fn heartbeat_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        System::set_block_number(102);
        assert_ok!(AgentAttestation::heartbeat(RuntimeOrigin::signed(1)));
        let node = AgentAttestation::node_of(1).unwrap();
        assert_eq!(node.last_heartbeat, 102);
    });
}

#[test]
fn heartbeat_too_early_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        System::set_block_number(51);
        assert_noop!(
            AgentAttestation::heartbeat(RuntimeOrigin::signed(1)),
            Error::<Test>::HeartbeatTooEarly
        );
    });
}

#[test]
fn submit_attestation_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        let result_hash = H256::from_low_u64_be(42);
        assert_ok!(AgentAttestation::submit_attestation(
            RuntimeOrigin::signed(1),
            1,
            result_hash,
            model_id(),
            1000,
            500,
        ));
        let att = AgentAttestation::attestation_of(0).unwrap();
        assert_eq!(att.task_id, 1);
        assert_eq!(att.result_hash, result_hash);
        assert_eq!(att.input_tokens, 1000);
        assert_eq!(att.output_tokens, 500);
        assert!(matches!(att.status, AttestationStatus::Pending));
        assert_eq!(att.challenge_end, 51);
    });
}

#[test]
fn submit_attestation_unregistered_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AgentAttestation::submit_attestation(
                RuntimeOrigin::signed(1),
                1,
                H256::zero(),
                model_id(),
                100,
                50,
            ),
            Error::<Test>::NodeNotRegistered
        );
    });
}

#[test]
fn challenge_attestation_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        assert_ok!(AgentAttestation::submit_attestation(
            RuntimeOrigin::signed(1),
            1,
            H256::from_low_u64_be(42),
            model_id(),
            1000,
            500,
        ));
        System::set_block_number(25);
        assert_ok!(AgentAttestation::challenge_attestation(
            RuntimeOrigin::signed(2),
            0,
        ));
        let att = AgentAttestation::attestation_of(0).unwrap();
        assert_eq!(att.challenger, Some(2));
    });
}

#[test]
fn challenge_after_window_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        assert_ok!(AgentAttestation::submit_attestation(
            RuntimeOrigin::signed(1),
            1,
            H256::from_low_u64_be(42),
            model_id(),
            1000,
            500,
        ));
        System::set_block_number(52);
        assert_noop!(
            AgentAttestation::challenge_attestation(RuntimeOrigin::signed(2), 0),
            Error::<Test>::ChallengeWindowExpired
        );
    });
}

#[test]
fn confirm_attestation_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        assert_ok!(AgentAttestation::submit_attestation(
            RuntimeOrigin::signed(1),
            1,
            H256::from_low_u64_be(42),
            model_id(),
            1000,
            500,
        ));
        let reserved = Balances::reserved_balance(1);
        assert_eq!(reserved, 1_000);
        System::set_block_number(52);
        assert_ok!(AgentAttestation::confirm_attestation(
            RuntimeOrigin::signed(3),
            0,
        ));
        let att = AgentAttestation::attestation_of(0).unwrap();
        assert!(matches!(att.status, AttestationStatus::Confirmed));
        let reserved_after = Balances::reserved_balance(1);
        assert_eq!(reserved_after, 0);
    });
}

#[test]
fn resolve_challenge_slash_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        assert_ok!(AgentAttestation::submit_attestation(
            RuntimeOrigin::signed(1),
            1,
            H256::from_low_u64_be(42),
            model_id(),
            1000,
            500,
        ));
        assert_ok!(AgentAttestation::challenge_attestation(
            RuntimeOrigin::signed(2),
            0,
        ));
        let balance_before = Balances::free_balance(1);
        assert_ok!(AgentAttestation::resolve_challenge(
            RuntimeOrigin::root(),
            0,
            true,
        ));
        let att = AgentAttestation::attestation_of(0).unwrap();
        assert!(matches!(att.status, AttestationStatus::Slashed));
        let balance_after = Balances::free_balance(1);
        // 500 of 1000 deposit slashed, remainder 500 unreserved back
        assert_eq!(balance_after, balance_before + 500);
    });
}

#[test]
fn resolve_challenge_defend_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));
        assert_ok!(AgentAttestation::submit_attestation(
            RuntimeOrigin::signed(1),
            1,
            H256::from_low_u64_be(42),
            model_id(),
            1000,
            500,
        ));
        assert_ok!(AgentAttestation::challenge_attestation(
            RuntimeOrigin::signed(2),
            0,
        ));
        let balance_before = Balances::free_balance(1);
        assert_ok!(AgentAttestation::resolve_challenge(
            RuntimeOrigin::root(),
            0,
            false,
        ));
        let att = AgentAttestation::attestation_of(0).unwrap();
        assert!(matches!(att.status, AttestationStatus::Defended));
        let balance_after = Balances::free_balance(1);
        assert_eq!(balance_after - balance_before, 1_000);
    });
}

#[test]
fn update_capability_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(AgentAttestation::register_node(
            RuntimeOrigin::signed(1),
            gpu_uuid(),
            120,
        ));

        assert_ok!(AgentAttestation::update_capability(
            RuntimeOrigin::signed(1),
            vec![model_id(), model_id_2()],
            8,
            10,
            b"us-west".to_vec(),
        ));

        let cap = AgentAttestation::agent_capability(1).expect("capability should exist");
        assert_eq!(cap.owner, 1);
        assert_eq!(cap.model_ids.len(), 2);
        assert_eq!(cap.max_concurrent, 8);
        assert_eq!(cap.price_per_token, 10);

        let model_bounded: frame_support::BoundedVec<u8, MaxModelIdLen> = model_id().try_into().unwrap();
        let providers = AgentAttestation::get_providers_for_model(&model_bounded);
        assert_eq!(providers, vec![1]);

        assert_ok!(AgentAttestation::update_capability(
            RuntimeOrigin::signed(1),
            vec![model_id_2()],
            4,
            20,
            b"eu".to_vec(),
        ));

        let old_model_bounded: frame_support::BoundedVec<u8, MaxModelIdLen> = model_id().try_into().unwrap();
        let old_providers = AgentAttestation::get_providers_for_model(&old_model_bounded);
        assert!(old_providers.is_empty());
    });
}
