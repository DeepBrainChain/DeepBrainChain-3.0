use crate as pallet_zk_compute;
use crate::VerifyZkProof;
use frame_support::{
	construct_runtime,
	parameter_types,
	traits::{ConstU16, ConstU32},
	PalletId,
};
use frame_system as system;
use sp_core::H256;
use sp_runtime::traits::IdentityLookup;

pub type AccountId = u64;
pub type Balance = u64;
pub type BlockNumber = u64;

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances,
		ZkCompute: pallet_zk_compute,
	}
);

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const ExistentialDeposit: u64 = 1;
	pub const MaxLocks: u32 = 50;
	pub const ZkPalletId: PalletId = PalletId(*b"zkc/mpal");
	pub const BaseReward: Balance = 100;
	pub const SubmissionDeposit: Balance = 10;
	pub const MaxProofSize: u32 = 1024;
	pub const MaxVerificationKeySize: u32 = 2048;
	pub const MaxPublicInputsSize: u32 = 512;
	pub const MaxPendingTasks: u32 = 32;
	pub const MaxVerifiedTasks: u32 = 64;
	pub const MaxPendingPerMiner: u32 = 2;
	pub const VerificationTimeout: BlockNumber = 5;
	pub const InitialMinerScore: u32 = 50;
	pub const MinMinerScoreToSubmit: u32 = 10;
	pub const MaxMinerScore: u32 = 100;
	pub const ScoreOnSuccess: u32 = 10;
	pub const ScorePenaltyOnFailure: u32 = 20;
}

impl system::Config for Test {	type BaseCallFilter = frame_support::traits::Everything;	type BlockWeights = ();	type BlockLength = ();	type DbWeight = ();	type RuntimeOrigin = RuntimeOrigin;	type RuntimeCall = RuntimeCall;	type Index = u64;	type BlockNumber = BlockNumber;	type Hash = H256;	type Hashing = sp_runtime::traits::BlakeTwo256;	type AccountId = AccountId;	type Lookup = IdentityLookup<Self::AccountId>;	type Header = sp_runtime::generic::Header<BlockNumber, sp_runtime::traits::BlakeTwo256>;	type RuntimeEvent = RuntimeEvent;	type BlockHashCount = BlockHashCount;	type Version = ();	type PalletInfo = PalletInfo;	type AccountData = pallet_balances::AccountData<Balance>;	type OnNewAccount = ();	type OnKilledAccount = ();	type SystemWeightInfo = ();	type SS58Prefix = ConstU16<42>;	type OnSetCode = ();	type MaxConsumers = ConstU32<16>;}

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

pub struct MockZkVerifier;

impl VerifyZkProof for MockZkVerifier {
	fn verify(proof: &[u8], dimensions: (u32, u32, u32)) -> bool {
		proof.first().copied() == Some(1)
			&& dimensions.0 > 0
			&& dimensions.1 > 0
			&& dimensions.2 > 0
	}
}

impl pallet_zk_compute::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type TaskId = u64;
	type ZkVerifier = MockZkVerifier;
	type MaxProofSize = MaxProofSize;
	type MaxVerificationKeySize = MaxVerificationKeySize;
	type MaxPublicInputsSize = MaxPublicInputsSize;
	type MaxPendingTasks = MaxPendingTasks;
	type MaxVerifiedTasks = MaxVerifiedTasks;
	type MaxPendingPerMiner = MaxPendingPerMiner;
	type BaseReward = BaseReward;
	type SubmissionDeposit = SubmissionDeposit;
	type VerificationTimeout = VerificationTimeout;
	type InitialMinerScore = InitialMinerScore;
	type MinMinerScoreToSubmit = MinMinerScoreToSubmit;
	type MaxMinerScore = MaxMinerScore;
	type ScoreOnSuccess = ScoreOnSuccess;
	type ScorePenaltyOnFailure = ScorePenaltyOnFailure;
	type PalletId = ZkPalletId;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	use sp_runtime::traits::AccountIdConversion;

	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.expect("mock storage should build");

	let pallet_account = ZkPalletId::get().into_account_truncating();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(1, 1_000_000),
			(2, 1_000_000),
			(3, 1_000_000),
			(pallet_account, 1_000_000),
		],
	}
	.assimilate_storage(&mut storage)
	.expect("balances storage assimilates");

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
