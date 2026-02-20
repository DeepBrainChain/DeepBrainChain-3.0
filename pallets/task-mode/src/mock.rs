use crate as pallet_task_mode;
use dbc_support::traits::DbcPrice;
use frame_support::{
    parameter_types,
    traits::{ConstU16, ConstU32},
};
use sp_core::H256;
use sp_runtime::{
    generic::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Percent,
};
use std::cell::RefCell;

pub type AccountId = u64;
pub type BlockNumber = u64;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system,
        Balances: pallet_balances,
        TaskMode: pallet_task_mode,
    }
);

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const ExistentialDeposit: u128 = 1;
    pub const MaxLocks: u32 = 50;
    pub const BurnPercentage: Percent = Percent::from_percent(15);
    pub const MinerPayoutPercentage: Percent = Percent::from_percent(85);
    pub const TaskModeRewardPercentage: Percent = Percent::from_percent(70);
    pub const EraDuration: BlockNumber = 100;
    pub const TreasuryAccount: AccountId = 99;
    pub const MaxModelIdLen: u32 = 256;
    pub const MaxPolicyCidLen: u32 = 1024;
}

thread_local! {
    static MOCK_DBC_PRICE: RefCell<Option<u128>> = RefCell::new(Some(2_000_000));
    static MOCK_DBC_MULTIPLIER: RefCell<Option<u128>> = RefCell::new(Some(10));
}

pub struct DBCPriceOCW;

impl DBCPriceOCW {
    pub fn set_price(price: Option<u128>) {
        MOCK_DBC_PRICE.with(|v| *v.borrow_mut() = price);
    }

    pub fn set_multiplier(multiplier: Option<u128>) {
        MOCK_DBC_MULTIPLIER.with(|v| *v.borrow_mut() = multiplier);
    }
}

impl DbcPrice for DBCPriceOCW {
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

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
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
    type AccountData = pallet_balances::AccountData<u128>;
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
    type Balance = u128;
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

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type DbcPriceProvider = DBCPriceOCW;
    type TreasuryAccount = TreasuryAccount;
    type BurnPercentage = BurnPercentage;
    type MinerPayoutPercentage = MinerPayoutPercentage;
    type TaskModeRewardPercentage = TaskModeRewardPercentage;
    type EraDuration = EraDuration;
    type MaxModelIdLen = MaxModelIdLen;
    type MaxPolicyCidLen = MaxPolicyCidLen;
    type WeightInfo = ();
    type ComputeScheduler = MockComputeScheduler;
}

// Mock implementation for TaskComputeScheduler
pub struct MockComputeScheduler;

impl dbc_support::traits::TaskComputeScheduler for MockComputeScheduler {
    type AccountId = u64;
    type Balance = u128;

    fn schedule_compute(
        _user: &Self::AccountId,
        _model_id: &[u8],
        _dimensions: (u32, u32, u32),
    ) -> Result<(u64, Self::AccountId, Self::Balance), &'static str> {
        // Return mock values for testing
        Ok((1, 1, 1000))
    }

    fn is_task_completed(_scheduler_task_id: u64) -> bool {
        true
    }
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .expect("frame system storage builds");

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 1_000_000_000_000), (2, 1_000_000_000_000), (3, 1_000_000_000_000), (99, 1_000_000_000_000)],
    }
    .assimilate_storage(&mut t)
    .expect("balances storage assimilates");

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        DBCPriceOCW::set_price(Some(2_000_000));
        DBCPriceOCW::set_multiplier(Some(10));
    });
    ext
}
