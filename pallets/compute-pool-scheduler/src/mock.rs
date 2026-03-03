use crate as pallet_compute_pool_scheduler;
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU16, ConstU32, Everything},
};
use sp_core::H256;
use sp_runtime::{
    generic::Header,
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

pub type AccountId = u64;
pub type Balance = u128;
pub type BlockNumber = u64;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        ComputePoolScheduler: pallet_compute_pool_scheduler,
    }
);

type Block = frame_system::mocking::MockBlock<Test>;

parameter_types! {
    pub const BlockHashCount: BlockNumber = 250;
    pub const ExistentialDeposit: Balance = 1;
    pub const MaxLocks: u32 = 32;
    pub const PoolDeposit: Balance = 1_000;
    pub const TaskDeposit: Balance = 100;
    pub const FailureSlash: Balance = 50;
    pub const TaskTimeout: BlockNumber = 5;
    pub const MaxGpuModelLen: u32 = 64;
    pub const MinPoolStake: u128 = 1_000;
    pub const StakeSlashPercent: u32 = 10;
    pub const MaxTasksPerPool: u32 = 16;
    pub const InitialReputation: u32 = 80;
    pub const VerificationTimeout: BlockNumber = 3;
    pub const ComplaintDeposit: Balance = 100;
    pub const ComplaintSlashPercent: u32 = 20;
    pub const MaxOpenComplaints: u32 = 10;
    pub const SlashGracePeriod: BlockNumber = 5;
}

impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
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
    type RuntimeTask = RuntimeTask;
    type ExtensionsWeightInfo = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
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
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type DoneSlashHandler = ();
}

// Mock implementation for TaskCompletionHandler
pub struct MockTaskCompletionHandler;

impl dbc_support::traits::TaskCompletionHandler for MockTaskCompletionHandler {
    type AccountId = u64;

    fn on_task_completed(
        _attester: &Self::AccountId,
        _task_id: u64,
        _result_hash: sp_core::H256,
        _model_id: &[u8],
        _input_tokens: u64,
        _output_tokens: u64,
    ) -> Result<u64, &'static str> {
        Ok(0)
    }
}

impl crate::Config for Test {
    type Currency = Balances;
    type PoolDeposit = PoolDeposit;
    type TaskDeposit = TaskDeposit;
    type FailureSlash = FailureSlash;
    type TaskTimeout = TaskTimeout;
    type MaxGpuModelLen = MaxGpuModelLen;
    type MinPoolStake = MinPoolStake;
    type StakeSlashPercent = StakeSlashPercent;
    type MaxTasksPerPool = MaxTasksPerPool;
    type InitialReputation = InitialReputation;
    type VerificationTimeout = VerificationTimeout;
    type WeightInfo = ();
    type OnTaskCompleted = MockTaskCompletionHandler;
    type ComplaintDeposit = ComplaintDeposit;
    type ComplaintSlashPercent = ComplaintSlashPercent;
    type MaxOpenComplaints = MaxOpenComplaints;
    type SlashGracePeriod = SlashGracePeriod;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage =
        frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 1_000_000),
            (2, 1_000_000),
            (3, 1_000_000),
            (4, 1_000_000),
            (5, 1_000_000),
            (99, 1_000_000),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext = sp_io::TestExternalities::from(storage);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn run_to_block(n: BlockNumber) {
    while System::block_number() < n {
        let next = System::block_number() + 1;
        System::set_block_number(next);
    }
}
