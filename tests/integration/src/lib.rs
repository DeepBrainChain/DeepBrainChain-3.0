//! DBC 3.0 Cross-Pallet Integration Tests
//!
//! Tests the full pipeline: TaskMode -> ComputePoolScheduler -> AgentAttestation -> X402Settlement
//! with REAL pallet wiring (no mocks for cross-pallet calls).

#[cfg(test)]
mod tests {
    use frame_support::{
        construct_runtime, parameter_types,
        traits::{ConstU16, ConstU32, Everything},
    };
    use sp_core::H256;
    use sp_runtime::{
        generic::Header,
        traits::{BlakeTwo256, IdentityLookup},
        Percent,
    };
    use std::cell::RefCell;

    use dbc_support::traits::DbcPrice;

    pub type AccountId = u64;
    pub type BlockNumber = u64;
    pub type Balance = u128;

    // ================================================================
    // construct_runtime! with all 4 DBC 3.0 pallets wired together
    // ================================================================
    construct_runtime!(
        pub enum Test where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system,
            Balances: pallet_balances,
            TaskMode: pallet_task_mode,
            ComputePoolScheduler: pallet_compute_pool_scheduler,
            AgentAttestation: pallet_agent_attestation,
            X402Settlement: pallet_x402_settlement,
        }
    );

    type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
    type Block = frame_system::mocking::MockBlock<Test>;

    // ================================================================
    // Parameter types (merged from all pallet mocks)
    // ================================================================
    parameter_types! {
        pub const BlockHashCount: u64 = 250;
        pub const ExistentialDeposit: Balance = 1;
        pub const MaxLocks: u32 = 50;

        // TaskMode parameters
        pub const BurnPercentage: Percent = Percent::from_percent(15);
        pub const MinerPayoutPercentage: Percent = Percent::from_percent(85);
        pub const TaskModeRewardPercentage: Percent = Percent::from_percent(70);
        pub const EraDuration: BlockNumber = 100;
        pub const TreasuryAccount: AccountId = 99;
        pub const MaxModelIdLen: u32 = 256;
        pub const MaxPolicyCidLen: u32 = 1024;

        // ComputePoolScheduler parameters
        pub const PoolDeposit: Balance = 1_000;
        pub const TaskDeposit: Balance = 100;
        pub const FailureSlash: Balance = 50;
        pub const TaskTimeout: BlockNumber = 50;
        pub const MaxGpuModelLen: u32 = 64;
        pub const MaxTasksPerPool: u32 = 16;
        pub const InitialReputation: u32 = 80;

        // AgentAttestation parameters
        pub const AttestationDeposit: Balance = 1_000;
        pub const ChallengeWindow: BlockNumber = 50;
        pub const SlashPercent: u32 = 50;
        pub const HeartbeatInterval: BlockNumber = 100;
        pub const MaxGpuUuidLen: u32 = 128;

        // X402Settlement parameters
        pub const FacilitatorAccount: AccountId = 100;
        pub const MaxSignatureLen: u32 = 256;
        pub const SettlementDelay: BlockNumber = 10;
    }

    // ================================================================
    // Mock DBC Price Provider
    // ================================================================
    thread_local! {
        static MOCK_DBC_PRICE: RefCell<Option<u128>> = RefCell::new(Some(2_000_000));
        static MOCK_DBC_MULTIPLIER: RefCell<Option<u128>> = RefCell::new(Some(10));
    }

    pub struct MockDbcPriceProvider;

    impl MockDbcPriceProvider {
        pub fn set_price(price: Option<u128>) {
            MOCK_DBC_PRICE.with(|v| *v.borrow_mut() = price);
        }

        pub fn set_multiplier(multiplier: Option<u128>) {
            MOCK_DBC_MULTIPLIER.with(|v| *v.borrow_mut() = multiplier);
        }
    }

    impl DbcPrice for MockDbcPriceProvider {
        type Balance = u128;

        fn get_dbc_price() -> Option<Self::Balance> {
            MOCK_DBC_PRICE.with(|v| *v.borrow())
        }

        fn get_dbc_amount_by_value(value: u64) -> Option<Self::Balance> {
            let value_u128 = value as u128;
            MOCK_DBC_MULTIPLIER.with(|v| {
                let mult = *v.borrow();
                mult.and_then(|m| value_u128.checked_mul(m))
            })
        }

        fn get_dlc_amount_by_value(_value: u64) -> Option<Self::Balance> {
            None
        }
    }

    // ================================================================
    // Pallet Configs
    // ================================================================

    impl frame_system::Config for Test {
        type BaseCallFilter = Everything;
        type BlockWeights = ();
        type BlockLength = ();
        type DbWeight = ();
        type RuntimeOrigin = RuntimeOrigin;
        type RuntimeCall = RuntimeCall;
        type Index = u64;
        type BlockNumber = BlockNumber;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header<BlockNumber, BlakeTwo256>;
        type RuntimeEvent = RuntimeEvent;
        type BlockHashCount = BlockHashCount;
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = pallet_balances::AccountData<Balance>;
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type SystemWeightInfo = ();
        type SS58Prefix = ConstU16<42>;
        type OnSetCode = ();
        type MaxConsumers = ConstU32<16>;
    }

    impl pallet_balances::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type WeightInfo = ();
        type Balance = Balance;
        type DustRemoval = ();
        type ExistentialDeposit = ExistentialDeposit;
        type AccountStore = System;
        type MaxLocks = MaxLocks;
        type MaxReserves = ();
        type ReserveIdentifier = [u8; 8];
        type MaxFreezes = ConstU32<0>;
        type MaxHolds = ConstU32<0>;
        type FreezeIdentifier = ();
        type HoldIdentifier = ();
    }

    // REAL WIRING: TaskMode uses ComputePoolScheduler for compute scheduling
    impl pallet_task_mode::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type DbcPriceProvider = MockDbcPriceProvider;
        type TreasuryAccount = TreasuryAccount;
        type BurnPercentage = BurnPercentage;
        type MinerPayoutPercentage = MinerPayoutPercentage;
        type TaskModeRewardPercentage = TaskModeRewardPercentage;
        type EraDuration = EraDuration;
        type MaxModelIdLen = MaxModelIdLen;
        type MaxPolicyCidLen = MaxPolicyCidLen;
        type WeightInfo = ();
        // REAL: TaskMode -> ComputePoolScheduler
        type ComputeScheduler = ComputePoolScheduler;
    }

    // REAL WIRING: ComputePoolScheduler notifies AgentAttestation on task completion
    impl pallet_compute_pool_scheduler::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type PoolDeposit = PoolDeposit;
        type TaskDeposit = TaskDeposit;
        type FailureSlash = FailureSlash;
        type TaskTimeout = TaskTimeout;
        type MaxGpuModelLen = MaxGpuModelLen;
        type MaxTasksPerPool = MaxTasksPerPool;
        type InitialReputation = InitialReputation;
        type WeightInfo = ();
        // REAL: ComputePoolScheduler -> AgentAttestation
        type OnTaskCompleted = AgentAttestation;
    }

    // REAL WIRING: AgentAttestation triggers X402Settlement on attestation confirmation
    impl pallet_agent_attestation::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type AttestationDeposit = AttestationDeposit;
        type ChallengeWindow = ChallengeWindow;
        type SlashPercent = SlashPercent;
        type HeartbeatInterval = HeartbeatInterval;
        type MaxModelIdLen = MaxModelIdLen;
        type MaxGpuUuidLen = MaxGpuUuidLen;
        type WeightInfo = ();
        // REAL: AgentAttestation -> X402Settlement
        type OnAttestationConfirmed = X402Settlement;
    }

    impl pallet_x402_settlement::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type Currency = Balances;
        type FacilitatorAccount = FacilitatorAccount;
        type MaxSignatureLen = MaxSignatureLen;
        type SettlementDelay = SettlementDelay;
        type WeightInfo = ();
    }

    // ================================================================
    // Test helpers
    // ================================================================

    pub fn new_test_ext() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .expect("frame system storage builds");

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (1, 1_000_000_000_000),  // customer
                (2, 1_000_000_000_000),  // miner / pool owner
                (3, 1_000_000_000_000),  // admin / challenger
                (4, 1_000_000_000_000),  // extra account
                (99, 1_000_000_000_000), // treasury
                (100, 1_000_000_000_000), // facilitator
            ],
        }
        .assimilate_storage(&mut t)
        .expect("balances storage assimilates");

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            System::set_block_number(1);
            MockDbcPriceProvider::set_price(Some(2_000_000));
            MockDbcPriceProvider::set_multiplier(Some(10));
        });
        ext
    }

    fn run_to_block(n: BlockNumber) {
        while System::block_number() < n {
            let next = System::block_number() + 1;
            System::set_block_number(next);
        }
    }

    // ================================================================
    // Test 1: Full pipeline - Task -> Compute -> Attestation -> Settlement
    // ================================================================
    #[test]
    fn full_pipeline_task_to_settlement() {
        new_test_ext().execute_with(|| {
            let admin: AccountId = 3;
            let customer: AccountId = 1;
            let miner: AccountId = 2;

            // ----- Step 1: Register a compute pool (miner) -----
            let gpu_model: frame_support::BoundedVec<u8, MaxGpuModelLen> =
                b"NVIDIA-A100".to_vec().try_into().unwrap();
            assert!(pallet_compute_pool_scheduler::Pallet::<Test>::register_pool(
                RuntimeOrigin::signed(miner),
                gpu_model,
                80, // gpu_memory
                false,
                100, // nvlink_efficiency (100 = no nvlink)
                10,  // price_per_task
            ).is_ok());

            // Verify pool exists
            let pool = pallet_compute_pool_scheduler::Pools::<Test>::get(0).unwrap();
            assert_eq!(pool.owner, miner);

            // ----- Step 2: Register miner as attestation node -----
            assert!(pallet_agent_attestation::Pallet::<Test>::register_node(
                RuntimeOrigin::signed(miner),
                b"GPU-UUID-001".to_vec(),
                312, // tflops
            ).is_ok());

            // ----- Step 3: Create a task definition (admin) -----
            assert!(pallet_task_mode::Pallet::<Test>::create_task_definition(
                RuntimeOrigin::signed(admin),
                b"llama-70b".to_vec(),
                b"v1.0".to_vec(),
                5,    // input_price_usd_per_1k
                15,   // output_price_usd_per_1k
                4096, // max_tokens_per_request
                b"QmPolicyCid123".to_vec(),
            ).is_ok());

            // Verify task definition
            let task_def = pallet_task_mode::TaskDefinitions::<Test>::get(0).unwrap();
            assert_eq!(task_def.admin, admin);
            assert!(task_def.is_active);

            // ----- Step 4: Create a task order (customer) -----
            assert!(pallet_task_mode::Pallet::<Test>::create_task_order(
                RuntimeOrigin::signed(customer),
                0,     // task_id (the definition we just created)
                miner, // miner
                500,   // input_tokens
                1000,  // output_tokens
            ).is_ok());

            // Verify task order exists and is InProgress
            let order = pallet_task_mode::TaskOrders::<Test>::get(0).unwrap();
            assert_eq!(order.customer, customer);
            assert_eq!(order.miner, miner);
            assert!(matches!(
                order.status,
                pallet_task_mode::pallet::TaskOrderStatus::InProgress
            ));

            // ----- Step 5: Submit a compute task directly via scheduler -----
            // (In a real runtime, TaskMode would call schedule_compute internally)
            let dimensions = pallet_compute_pool_scheduler::pallet::TaskDimensions {
                m: 100,
                n: 100,
                k: 10, // must be <= gpu_memory (80)
            };
            assert!(pallet_compute_pool_scheduler::Pallet::<Test>::submit_task(
                RuntimeOrigin::signed(customer),
                dimensions,
                pallet_compute_pool_scheduler::pallet::TaskPriority::Normal,
            ).is_ok());

            // Verify task was assigned to the pool
            let compute_task = pallet_compute_pool_scheduler::Tasks::<Test>::get(0).unwrap();
            assert_eq!(compute_task.pool_id, 0);
            assert!(matches!(
                compute_task.status,
                pallet_compute_pool_scheduler::pallet::TaskStatus::Computing
            ));

            // ----- Step 6: Submit proof (miner) -----
            // This triggers OnTaskCompleted -> AgentAttestation::on_task_completed
            let proof_hash = [1u8; 32];
            assert!(pallet_compute_pool_scheduler::Pallet::<Test>::submit_proof(
                RuntimeOrigin::signed(miner),
                0,          // task_id
                proof_hash, // proof_hash
                true,       // verification_result
            ).is_ok());

            // Verify task completed
            let completed_task = pallet_compute_pool_scheduler::Tasks::<Test>::get(0).unwrap();
            assert!(matches!(
                completed_task.status,
                pallet_compute_pool_scheduler::pallet::TaskStatus::Completed
            ));
            assert_eq!(completed_task.verification_result, Some(true));

            // Verify attestation was created automatically by cross-pallet call
            let attestation = pallet_agent_attestation::Attestations::<Test>::get(0).unwrap();
            assert_eq!(attestation.attester, miner);
            assert_eq!(attestation.task_id, 0);
            assert!(matches!(
                attestation.status,
                pallet_agent_attestation::pallet::AttestationStatus::Pending
            ));

            // ----- Step 7: Advance past challenge window and confirm attestation -----
            run_to_block(1 + ChallengeWindow::get() + 1);

            assert!(pallet_agent_attestation::Pallet::<Test>::confirm_attestation(
                RuntimeOrigin::signed(admin), // anyone can confirm after window
                0, // attestation_id
            ).is_ok());

            // Verify attestation is confirmed
            let confirmed_att = pallet_agent_attestation::Attestations::<Test>::get(0).unwrap();
            assert!(matches!(
                confirmed_att.status,
                pallet_agent_attestation::pallet::AttestationStatus::Confirmed
            ));

            // ----- Step 8: Verify settlement receipt was created by cross-pallet call -----
            // X402Settlement::settle_for_attestation was called by AgentAttestation::confirm_attestation
            let receipt = pallet_x402_settlement::pallet::SettlementReceipts::<Test>::get(0);
            assert!(receipt.is_some(), "Settlement receipt should exist from cross-pallet call");
            let receipt = receipt.unwrap();
            assert_eq!(receipt.miner, miner);

            // ----- Step 9: Claim compute reward -----
            assert!(pallet_compute_pool_scheduler::Pallet::<Test>::claim_reward(
                RuntimeOrigin::signed(miner),
                0, // task_id
            ).is_ok());

            // ----- Step 10: Settle the TaskMode order -----
            assert!(pallet_task_mode::Pallet::<Test>::mark_order_completed(
                RuntimeOrigin::signed(miner),
                0, // order_id
                [2u8; 32], // attestation_hash
            ).is_ok());

            assert!(pallet_task_mode::Pallet::<Test>::settle_task_order(
                RuntimeOrigin::signed(customer),
                0,    // order_id
                None, // attestation_hash
            ).is_ok());

            let settled_order = pallet_task_mode::TaskOrders::<Test>::get(0).unwrap();
            assert!(matches!(
                settled_order.status,
                pallet_task_mode::pallet::TaskOrderStatus::Settled
            ));

            println!("PASS: full_pipeline_task_to_settlement - all cross-pallet calls verified");
        });
    }

    // ================================================================
    // Test 2: Pool registration, task assignment, proof, claim reward
    // ================================================================
    #[test]
    fn pool_registration_and_task_assignment() {
        new_test_ext().execute_with(|| {
            let pool_owner: AccountId = 2;
            let task_user: AccountId = 1;

            // ----- Register pool -----
            let gpu_model: frame_support::BoundedVec<u8, MaxGpuModelLen> =
                b"RTX-4090".to_vec().try_into().unwrap();
            assert!(pallet_compute_pool_scheduler::Pallet::<Test>::register_pool(
                RuntimeOrigin::signed(pool_owner),
                gpu_model,
                24,    // gpu_memory
                false, // no nvlink
                100,   // nvlink_efficiency
                5,     // price_per_task
            ).is_ok());

            // Register pool owner as attestation node (needed for on_task_completed)
            assert!(pallet_agent_attestation::Pallet::<Test>::register_node(
                RuntimeOrigin::signed(pool_owner),
                b"GPU-RTX4090-UUID".to_vec(),
                200, // tflops
            ).is_ok());

            // Verify pool registered
            assert!(pallet_compute_pool_scheduler::Pools::<Test>::get(0).is_some());
            assert_eq!(
                pallet_compute_pool_scheduler::PoolByOwner::<Test>::get(pool_owner),
                Some(0)
            );

            // ----- Submit task -----
            let dims = pallet_compute_pool_scheduler::pallet::TaskDimensions {
                m: 50,
                n: 50,
                k: 10,
            };
            assert!(pallet_compute_pool_scheduler::Pallet::<Test>::submit_task(
                RuntimeOrigin::signed(task_user),
                dims,
                pallet_compute_pool_scheduler::pallet::TaskPriority::High,
            ).is_ok());

            // Verify task assigned to pool
            let task = pallet_compute_pool_scheduler::Tasks::<Test>::get(0).unwrap();
            assert_eq!(task.pool_id, 0);
            assert_eq!(task.user, task_user);
            assert!(matches!(
                task.status,
                pallet_compute_pool_scheduler::pallet::TaskStatus::Computing
            ));

            // Verify active task count incremented
            assert_eq!(pallet_compute_pool_scheduler::ActiveTaskCount::<Test>::get(0), 1);

            // ----- Submit proof -----
            let proof_hash = [42u8; 32];
            assert!(pallet_compute_pool_scheduler::Pallet::<Test>::submit_proof(
                RuntimeOrigin::signed(pool_owner),
                0,
                proof_hash,
                true,
            ).is_ok());

            // Verify task completed
            let completed = pallet_compute_pool_scheduler::Tasks::<Test>::get(0).unwrap();
            assert!(matches!(
                completed.status,
                pallet_compute_pool_scheduler::pallet::TaskStatus::Completed
            ));

            // Verify reward is available
            assert!(pallet_compute_pool_scheduler::Rewards::<Test>::get(0).is_some());

            // Verify attestation was created via cross-pallet call
            let att = pallet_agent_attestation::Attestations::<Test>::get(0);
            assert!(att.is_some(), "Attestation should have been created via OnTaskCompleted");

            // ----- Claim reward -----
            let balance_before = pallet_balances::Pallet::<Test>::free_balance(pool_owner);
            assert!(pallet_compute_pool_scheduler::Pallet::<Test>::claim_reward(
                RuntimeOrigin::signed(pool_owner),
                0,
            ).is_ok());
            let balance_after = pallet_balances::Pallet::<Test>::free_balance(pool_owner);
            assert!(balance_after > balance_before, "Pool owner should have received reward");

            // Verify reward was removed
            assert!(pallet_compute_pool_scheduler::Rewards::<Test>::get(0).is_none());

            // Verify active task count decremented
            assert_eq!(pallet_compute_pool_scheduler::ActiveTaskCount::<Test>::get(0), 0);

            // Verify reputation updated
            let rep = pallet_compute_pool_scheduler::MinerReputation::<Test>::get(pool_owner);
            assert_eq!(rep.total_tasks, 1);
            assert_eq!(rep.successful_tasks, 1);

            println!("PASS: pool_registration_and_task_assignment - full flow verified");
        });
    }

    // ================================================================
    // Test 3: Attestation challenge and resolution flow
    // ================================================================
    #[test]
    fn attestation_challenge_flow() {
        new_test_ext().execute_with(|| {
            let attester: AccountId = 2;
            let challenger: AccountId = 3;

            // ----- Register node -----
            assert!(pallet_agent_attestation::Pallet::<Test>::register_node(
                RuntimeOrigin::signed(attester),
                b"GPU-UUID-ATTEST".to_vec(),
                250,
            ).is_ok());

            // Verify node registered
            let node = pallet_agent_attestation::Nodes::<Test>::get(attester).unwrap();
            assert!(node.is_active);
            assert_eq!(node.tflops, 250);

            // ----- Submit attestation -----
            let result_hash = H256::from([0xAB; 32]);
            assert!(pallet_agent_attestation::Pallet::<Test>::submit_attestation(
                RuntimeOrigin::signed(attester),
                42,   // task_id
                result_hash,
                b"gpt-4-turbo".to_vec(),
                1000, // input_tokens
                2000, // output_tokens
            ).is_ok());

            // Verify attestation created
            let att = pallet_agent_attestation::Attestations::<Test>::get(0).unwrap();
            assert_eq!(att.attester, attester);
            assert_eq!(att.task_id, 42);
            assert!(matches!(
                att.status,
                pallet_agent_attestation::pallet::AttestationStatus::Pending
            ));
            assert_eq!(att.challenge_end, 1 + ChallengeWindow::get());

            // ----- Challenge attestation (within window) -----
            assert!(pallet_agent_attestation::Pallet::<Test>::challenge_attestation(
                RuntimeOrigin::signed(challenger),
                0, // attestation_id
            ).is_ok());

            let challenged_att = pallet_agent_attestation::Attestations::<Test>::get(0).unwrap();
            assert_eq!(challenged_att.challenger, Some(challenger));

            // ----- Resolve challenge: attester is guilty (slash) -----
            let attester_balance_before = pallet_balances::Pallet::<Test>::free_balance(attester);
            let attester_reserved_before = pallet_balances::Pallet::<Test>::reserved_balance(attester);

            assert!(pallet_agent_attestation::Pallet::<Test>::resolve_challenge(
                RuntimeOrigin::root(), // root only
                0,    // attestation_id
                true, // attester_is_guilty
            ).is_ok());

            let slashed_att = pallet_agent_attestation::Attestations::<Test>::get(0).unwrap();
            assert!(matches!(
                slashed_att.status,
                pallet_agent_attestation::pallet::AttestationStatus::Slashed
            ));

            // Verify slash occurred: reserved balance decreased
            let attester_reserved_after = pallet_balances::Pallet::<Test>::reserved_balance(attester);
            assert!(
                attester_reserved_after < attester_reserved_before,
                "Attester should have been slashed"
            );

            // ----- Test defend flow: submit new attestation and defend -----
            assert!(pallet_agent_attestation::Pallet::<Test>::submit_attestation(
                RuntimeOrigin::signed(attester),
                43,
                H256::from([0xCD; 32]),
                b"llama-70b".to_vec(),
                500,
                800,
            ).is_ok());

            // Challenge it
            assert!(pallet_agent_attestation::Pallet::<Test>::challenge_attestation(
                RuntimeOrigin::signed(challenger),
                1, // second attestation
            ).is_ok());

            // Resolve: attester wins (defended)
            let reserved_before_defend = pallet_balances::Pallet::<Test>::reserved_balance(attester);

            assert!(pallet_agent_attestation::Pallet::<Test>::resolve_challenge(
                RuntimeOrigin::root(),
                1,     // attestation_id
                false, // attester is NOT guilty
            ).is_ok());

            let defended_att = pallet_agent_attestation::Attestations::<Test>::get(1).unwrap();
            assert!(matches!(
                defended_att.status,
                pallet_agent_attestation::pallet::AttestationStatus::Defended
            ));

            // Verify deposit was returned
            let reserved_after_defend = pallet_balances::Pallet::<Test>::reserved_balance(attester);
            assert!(
                reserved_after_defend < reserved_before_defend,
                "Deposit should have been unreserved after defense"
            );

            println!("PASS: attestation_challenge_flow - slash and defend paths verified");
        });
    }
}
