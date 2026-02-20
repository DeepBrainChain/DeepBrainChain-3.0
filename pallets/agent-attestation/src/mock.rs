use crate as pallet_agent_attestation;
use frame_support::{
    parameter_types,
    traits::{ConstU16, ConstU32},
};
use sp_core::H256;
use sp_runtime::{
    generic::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

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
        AgentAttestation: pallet_agent_attestation,
    }
);

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const ExistentialDeposit: u128 = 1;
    pub const MaxLocks: u32 = 50;
    pub const AttestationDeposit: u128 = 1_000;
    pub const ChallengeWindow: BlockNumber = 50;
    pub const SlashPercent: u32 = 50;
    pub const HeartbeatInterval: BlockNumber = 100;
    pub const MaxModelIdLen: u32 = 256;
    pub const MaxGpuUuidLen: u32 = 128;
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

// Mock implementation for AttestationSettler
pub struct MockAttestationSettler;

impl dbc_support::traits::AttestationSettler for MockAttestationSettler {
    type AccountId = u64;
    type Balance = u128;

    fn settle_for_attestation(
        _merchant: &Self::AccountId,
        _miner: &Self::AccountId,
        _amount: Self::Balance,
        _attestation_id: u64,
    ) -> Result<u64, &'static str> {
        Ok(0)
    }
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type AttestationDeposit = AttestationDeposit;
    type ChallengeWindow = ChallengeWindow;
    type SlashPercent = SlashPercent;
    type HeartbeatInterval = HeartbeatInterval;
    type MaxModelIdLen = MaxModelIdLen;
    type MaxGpuUuidLen = MaxGpuUuidLen;
    type WeightInfo = ();
    type OnAttestationConfirmed = MockAttestationSettler;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .expect("frame system storage builds");

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 1_000_000_000_000),
            (2, 1_000_000_000_000),
            (3, 1_000_000_000_000),
            (4, 1_000_000_000_000),
        ],
    }
    .assimilate_storage(&mut t)
    .expect("balances storage assimilates");

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}
