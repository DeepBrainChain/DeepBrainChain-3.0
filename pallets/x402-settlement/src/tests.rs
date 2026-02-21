use crate::pallet::PaymentIntentStatus;
use crate::mock::{new_test_ext, RuntimeOrigin, System, Test, X402Settlement, Balances};
use frame_support::{assert_noop, assert_ok, traits::{Currency, Hooks}};
use sp_core::H256;
use codec::Encode;
/// Generate a valid facilitator sr25519 signature for testing.
fn make_facilitator_sig(merchant: u64, miner: u64, amount: u128, nonce: u64, fingerprint: H256) -> Vec<u8> {
    use sp_core::Pair;
    let pair = sp_core::sr25519::Pair::from_seed(&[1u8; 32]); // Must match FacilitatorPublicKeyValue in mock
    let mut message = Vec::new();
    merchant.encode_to(&mut message);
    miner.encode_to(&mut message);
    amount.encode_to(&mut message);
    nonce.encode_to(&mut message);
    fingerprint.encode_to(&mut message);
    pair.sign(&message).0.to_vec()
}

fn create_default_payment_intent() -> u64 {
    let sig = make_facilitator_sig(1, 3, 1_000_000, 1, H256::from_low_u64_be(12345));
    assert_ok!(X402Settlement::submit_payment_intent(
        RuntimeOrigin::signed(1),
        3,
        1_000_000,
        1,
        H256::from_low_u64_be(12345),
        sig,
    ));
    0 // First intent ID
}

#[test]
fn submit_payment_intent_works() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        let intent = X402Settlement::payment_intent_of(intent_id).expect("intent exists");
        assert_eq!(intent.merchant, 1);
        assert_eq!(intent.miner, 3);
        assert_eq!(intent.amount, 1_000_000);
        assert_eq!(intent.nonce, 1);
        assert_eq!(intent.replay_fingerprint, H256::from_low_u64_be(12345));
        assert!(matches!(intent.status, PaymentIntentStatus::Pending));
        assert_eq!(X402Settlement::next_intent_id(), 1);

        // Check that balance is reserved
        assert_eq!(Balances::reserved_balance(1), 1_000_000);
        assert_eq!(Balances::free_balance(1), 999_999_000_000);

        // Check nonce and replay fingerprint are marked as used
        assert!(X402Settlement::is_nonce_used(&1, 1));
        assert!(X402Settlement::is_replay_fingerprint_used(H256::from_low_u64_be(12345)));
    });
}

#[test]
fn submit_payment_intent_fails_with_duplicate_nonce() {
    new_test_ext().execute_with(|| {
        create_default_payment_intent();

        let sig2 = make_facilitator_sig(1, 3, 500_000, 1, H256::from_low_u64_be(67890));
        assert_noop!(
            X402Settlement::submit_payment_intent(
                RuntimeOrigin::signed(1),
                3,
                500_000,
                1, // Same nonce
                H256::from_low_u64_be(67890),
                sig2,
            ),
            crate::pallet::Error::<Test>::InvalidNonce
        );
    });
}

#[test]
fn submit_payment_intent_fails_with_duplicate_replay_fingerprint() {
    new_test_ext().execute_with(|| {
        create_default_payment_intent();

        let sig2 = make_facilitator_sig(2, 4, 500_000, 1, H256::from_low_u64_be(12345));
        assert_noop!(
            X402Settlement::submit_payment_intent(
                RuntimeOrigin::signed(2),
                4,
                500_000,
                1,
                H256::from_low_u64_be(12345), // Same fingerprint
                sig2,
            ),
            crate::pallet::Error::<Test>::ReplayFingerprintUsed
        );
    });
}

#[test]
fn submit_payment_intent_fails_with_insufficient_balance() {
    new_test_ext().execute_with(|| {
        // Get current balance
        let current_balance = Balances::free_balance(&1);
        
        // Try to submit payment intent with amount greater than balance
        let sig = make_facilitator_sig(1, 3, current_balance + 1, 1, H256::from_low_u64_be(12345));
        assert_noop!(
            X402Settlement::submit_payment_intent(
                RuntimeOrigin::signed(1),
                3,
                current_balance + 1, // More than available balance
                1,
                H256::from_low_u64_be(12345),
                sig,
            ),
            crate::pallet::Error::<Test>::InsufficientBalance
        );
    });
}



#[test]
fn submit_payment_intent_fails_with_invalid_signature() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            X402Settlement::submit_payment_intent(
                RuntimeOrigin::signed(1),
                3,
                1_000_000,
                1,
                H256::from_low_u64_be(12345),
                b"invalid_short".to_vec(), // Too short (< 32 bytes after try_into)
            ),
            crate::pallet::Error::<Test>::InvalidFacilitatorSignature
        );
    });
}
#[test]
fn verify_settlement_works() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        // Only facilitator can verify
        assert_noop!(
            X402Settlement::verify_settlement(RuntimeOrigin::signed(1), intent_id),
            crate::pallet::Error::<Test>::NotAuthorized
        );

        assert_ok!(X402Settlement::verify_settlement(
            RuntimeOrigin::signed(100), // Facilitator
            intent_id
        ));

        let intent = X402Settlement::payment_intent_of(intent_id).expect("intent exists");
        assert!(matches!(intent.status, PaymentIntentStatus::Verified));
        assert_eq!(intent.verified_at, Some(1));
    });
}

#[test]
fn verify_settlement_fails_for_wrong_status() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        // Verify once
        assert_ok!(X402Settlement::verify_settlement(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        // Try to verify again - should fail
        assert_noop!(
            X402Settlement::verify_settlement(RuntimeOrigin::signed(100), intent_id),
            crate::pallet::Error::<Test>::InvalidPaymentIntentStatus
        );
    });
}

#[test]
fn finalize_settlement_works() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        // First verify the settlement
        assert_ok!(X402Settlement::verify_settlement(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        // Advance blocks to meet settlement delay
        System::set_block_number(15); // Settlement delay is 10 blocks

        // Note: merchant balance is already reduced by 1_000_000 (reserved)
        // After finalization, reserved becomes 0 and miner gets the funds
        let miner_balance_before = Balances::free_balance(3);

        // Merchant can finalize
        assert_ok!(X402Settlement::finalize_settlement(
            RuntimeOrigin::signed(1),
            intent_id
        ));

        let intent = X402Settlement::payment_intent_of(intent_id).expect("intent exists");
        assert!(matches!(intent.status, PaymentIntentStatus::Settled));
        assert_eq!(intent.settled_at, Some(15));

        // Check funds transferred
        assert_eq!(Balances::reserved_balance(1), 0);
        // Merchant's free balance should remain at 999_999_000_000 (already reduced when reserved)
        assert_eq!(Balances::free_balance(1), 999_999_000_000);
        assert_eq!(
            Balances::free_balance(3),
            miner_balance_before + 1_000_000
        );

        // Check receipt created
        let receipt = X402Settlement::settlement_receipt_of(intent_id).expect("receipt exists");
        assert_eq!(receipt.merchant, 1);
        assert_eq!(receipt.miner, 3);
        assert_eq!(receipt.amount, 1_000_000);
        assert_eq!(receipt.settled_at, 15);
    });
}

#[test]
fn finalize_settlement_fails_before_delay() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        assert_ok!(X402Settlement::verify_settlement(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        // Try to finalize immediately - should fail
        assert_noop!(
            X402Settlement::finalize_settlement(RuntimeOrigin::signed(1), intent_id),
            crate::pallet::Error::<Test>::SettlementDelayNotMet
        );
    });
}

#[test]
fn finalize_settlement_by_miner_works() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        assert_ok!(X402Settlement::verify_settlement(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        System::set_block_number(15);

        // Miner can also finalize
        assert_ok!(X402Settlement::finalize_settlement(
            RuntimeOrigin::signed(3),
            intent_id
        ));

        let intent = X402Settlement::payment_intent_of(intent_id).expect("intent exists");
        assert!(matches!(intent.status, PaymentIntentStatus::Settled));
    });
}

#[test]
fn finalize_settlement_by_facilitator_works() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        assert_ok!(X402Settlement::verify_settlement(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        System::set_block_number(15);

        // Facilitator can also finalize
        assert_ok!(X402Settlement::finalize_settlement(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        let intent = X402Settlement::payment_intent_of(intent_id).expect("intent exists");
        assert!(matches!(intent.status, PaymentIntentStatus::Settled));
    });
}

#[test]
fn fail_payment_intent_works() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        // Only facilitator can fail
        assert_noop!(
            X402Settlement::fail_payment_intent(RuntimeOrigin::signed(1), intent_id),
            crate::pallet::Error::<Test>::NotAuthorized
        );

        assert_ok!(X402Settlement::fail_payment_intent(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        let intent = X402Settlement::payment_intent_of(intent_id).expect("intent exists");
        assert!(matches!(intent.status, PaymentIntentStatus::Failed));

        // Check funds released - merchant should have full balance back
        assert_eq!(Balances::reserved_balance(1), 0);
        assert_eq!(Balances::free_balance(1), 1_000_000_000_000);
    });
}

#[test]
fn fail_verified_payment_intent_works() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        assert_ok!(X402Settlement::verify_settlement(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        // Can also fail verified intents
        assert_ok!(X402Settlement::fail_payment_intent(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        let intent = X402Settlement::payment_intent_of(intent_id).expect("intent exists");
        assert!(matches!(intent.status, PaymentIntentStatus::Failed));
    });
}

#[test]
fn fail_settled_payment_intent_fails() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();

        assert_ok!(X402Settlement::verify_settlement(
            RuntimeOrigin::signed(100),
            intent_id
        ));

        System::set_block_number(15);
        assert_ok!(X402Settlement::finalize_settlement(
            RuntimeOrigin::signed(1),
            intent_id
        ));

        // Cannot fail settled intents
        assert_noop!(
            X402Settlement::fail_payment_intent(RuntimeOrigin::signed(100), intent_id),
            crate::pallet::Error::<Test>::InvalidPaymentIntentStatus
        );
    });
}

#[test]
fn payment_intent_expires_on_initialize() {
    new_test_ext().execute_with(|| {
        let intent_id = create_default_payment_intent();
        System::set_block_number(102);
        X402Settlement::on_initialize(102);
        let intent = X402Settlement::payment_intent_of(intent_id).expect("intent exists");
        assert!(matches!(intent.status, PaymentIntentStatus::Failed));
        assert_eq!(Balances::reserved_balance(1), 0);
        assert_eq!(Balances::free_balance(1), 1_000_000_000_000);
    });
}
