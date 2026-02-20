#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::pallet_prelude::*;
use sp_runtime::traits::AtLeast32BitUnsigned;
use sp_std::vec::Vec;

pub trait VerifyZkProof {
	fn verify(proof: &[u8], dimensions: (u32, u32, u32)) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::traits::StorageVersion;
	use super::*;
	use frame_system::offchain::SubmitTransaction;
	use sp_runtime::transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
	};
	use crate::weights::WeightInfo;
	use frame_support::{
		dispatch::DispatchResult,
		traits::{
			BalanceStatus, Currency, ExistenceRequirement::AllowDeath, ReservableCurrency,
		},
		transactional, PalletId,
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{
		AccountIdConversion, CheckedAdd, One, SaturatedConversion, Saturating, Zero,
	};

	pub type TaskIdOf<T> = <T as Config>::TaskId;
	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
	pub type BoundedProofOf<T> = BoundedVec<u8, <T as Config>::MaxProofSize>;
	pub type ZkTaskOf<T> = ZkTask<
		TaskIdOf<T>,
		<T as frame_system::Config>::AccountId,
		<T as frame_system::Config>::BlockNumber,
		BalanceOf<T>,
		BoundedProofOf<T>,
	>;

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum ZkVerificationStatus {
		Pending,
		Verified,
		Failed,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct ZkTask<TaskId, AccountId, BlockNumber, Balance, Proof> {
		pub task_id: TaskId,
		pub miner: AccountId,
		pub proof: Proof,
		pub dimensions: (u32, u32, u32),
		pub status: ZkVerificationStatus,
		pub base_reward: Balance,
		pub multiplier_q100: u32,
		pub submitted_at: BlockNumber,
		pub submission_deposit: Balance,
		pub reward_claimed: bool,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + frame_system::offchain::SendTransactionTypes<Call<Self>> {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: ReservableCurrency<Self::AccountId>;

		type TaskId: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ TypeInfo;

		type ZkVerifier: VerifyZkProof;

		#[pallet::constant]
		type MaxProofSize: Get<u32>;
		#[pallet::constant]
		type MaxVerificationKeySize: Get<u32>;
		#[pallet::constant]
		type MaxPublicInputsSize: Get<u32>;
		#[pallet::constant]
		type MaxPendingTasks: Get<u32>;
		#[pallet::constant]
		type MaxVerifiedTasks: Get<u32>;
		#[pallet::constant]
		type MaxPendingPerMiner: Get<u32>;
		#[pallet::constant]
		type BaseReward: Get<BalanceOf<Self>>;
		#[pallet::constant]
		type SubmissionDeposit: Get<BalanceOf<Self>>;
		#[pallet::constant]
		type VerificationTimeout: Get<Self::BlockNumber>;
		#[pallet::constant]
		type InitialMinerScore: Get<u32>;
		#[pallet::constant]
		type MinMinerScoreToSubmit: Get<u32>;
		#[pallet::constant]
		type MaxMinerScore: Get<u32>;
		#[pallet::constant]
		type ScoreOnSuccess: Get<u32>;
		#[pallet::constant]
		type ScorePenaltyOnFailure: Get<u32>;
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type WeightInfo: WeightInfo;
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
	const UNSIGNED_TXS_PRIORITY: u64 = 100;

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn next_task_id)]
	pub type NextTaskId<T: Config> = StorageValue<_, T::TaskId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn tasks)]
	pub type Tasks<T: Config> = StorageMap<_, Blake2_128Concat, T::TaskId, ZkTaskOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn pending_tasks)]
	pub type PendingTasks<T: Config> =
		StorageValue<_, BoundedVec<T::TaskId, T::MaxPendingTasks>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn verified_tasks)]
	pub type VerifiedTasks<T: Config> =
		StorageValue<_, BoundedVec<T::TaskId, T::MaxVerifiedTasks>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn miner_score)]
	pub type MinerScores<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32>;

	#[pallet::storage]
	#[pallet::getter(fn used_nonce)]
	pub type UsedNonces<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, u64, bool, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn miner_pending_count)]
	pub type MinerPendingCount<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ProofSubmitted {
			task_id: T::TaskId,
			miner: T::AccountId,
			nonce: u64,
			dimensions: (u32, u32, u32),
		},
		ProofVerifiedByOcw { task_id: T::TaskId, verified: bool },
		ProofVerified {
			task_id: T::TaskId,
			verifier: T::AccountId,
			status: ZkVerificationStatus,
		},
		RewardClaimed {
			task_id: T::TaskId,
			miner: T::AccountId,
			reward: BalanceOf<T>,
		},
		MinerScoreUpdated {
			miner: T::AccountId,
			score: u32,
		},
		DepositSlashed {
			task_id: T::TaskId,
			miner: T::AccountId,
			amount: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		TaskNotFound,
		TaskIdOverflow,
		ProofTooLarge,
		InvalidDimensions,
		InvalidMultiplier,
		InvalidTaskStatus,
		RewardAlreadyClaimed,
		TaskNotVerified,
		NotTaskMiner,
		ArithmeticOverflow,
		NonceAlreadyUsed,
		TooManyPendingTasks,
		TooManyVerifiedTasks,
		TooManyPendingTasksForMiner,
		InsufficientMinerScore,
		BalanceTransferFailed,
	}


    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub _phantom: sp_std::marker::PhantomData<T>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { _phantom: Default::default() }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {}
    }

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(_block_number: BlockNumberFor<T>) {
			let _ = Self::ocw_verify_pending_tasks();
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::submit_proof())]
		#[transactional]
		pub fn submit_proof(
			origin: OriginFor<T>,
			proof: Vec<u8>,
			dimensions: (u32, u32, u32),
			multiplier_q100: u32,
			nonce: u64,
		) -> DispatchResult {
			let miner = ensure_signed(origin)?;

			ensure!(Self::valid_dimensions(dimensions), Error::<T>::InvalidDimensions);
			ensure!((120..=150).contains(&multiplier_q100), Error::<T>::InvalidMultiplier);
			ensure!(!UsedNonces::<T>::get(&miner, nonce), Error::<T>::NonceAlreadyUsed);
			ensure!(
				Self::current_miner_score(&miner) >= T::MinMinerScoreToSubmit::get(),
				Error::<T>::InsufficientMinerScore
			);

			let pending_count = MinerPendingCount::<T>::get(&miner);
			ensure!(
				pending_count < T::MaxPendingPerMiner::get(),
				Error::<T>::TooManyPendingTasksForMiner
			);

			let bounded_proof =
				BoundedProofOf::<T>::try_from(proof).map_err(|_| Error::<T>::ProofTooLarge)?;

			T::Currency::reserve(&miner, T::SubmissionDeposit::get())?;

			let task_id = NextTaskId::<T>::get();
			let next_task_id = task_id.checked_add(&One::one()).ok_or(Error::<T>::TaskIdOverflow)?;
			let now = <frame_system::Pallet<T>>::block_number();

			let task = ZkTask {
				task_id,
				miner: miner.clone(),
				proof: bounded_proof,
				dimensions,
				status: ZkVerificationStatus::Pending,
				base_reward: T::BaseReward::get(),
				multiplier_q100,
				submitted_at: now,
				submission_deposit: T::SubmissionDeposit::get(),
				reward_claimed: false,
			};

			PendingTasks::<T>::try_mutate(|queue| {
				queue.try_push(task_id).map_err(|_| Error::<T>::TooManyPendingTasks)
			})?;

			Tasks::<T>::insert(task_id, task);
			NextTaskId::<T>::put(next_task_id);
			UsedNonces::<T>::insert(&miner, nonce, true);
			MinerPendingCount::<T>::insert(&miner, pending_count.saturating_add(1));

			Self::deposit_event(Event::ProofSubmitted { task_id, miner, nonce, dimensions });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::verify_task())]
		#[transactional]
		pub fn verify_task(origin: OriginFor<T>, task_id: T::TaskId) -> DispatchResult {
			let verifier = ensure_signed(origin)?;
			let mut task = Tasks::<T>::get(task_id).ok_or(Error::<T>::TaskNotFound)?;
			ensure!(task.status == ZkVerificationStatus::Pending, Error::<T>::InvalidTaskStatus);

			let now = <frame_system::Pallet<T>>::block_number();
			let deadline = task
				.submitted_at
				.checked_add(&T::VerificationTimeout::get())
				.ok_or(Error::<T>::ArithmeticOverflow)?;

			let timed_out = now > deadline;
			let verified = if timed_out {
				false
			} else {
				T::ZkVerifier::verify(task.proof.as_ref(), task.dimensions)
			};

			task.status = if verified {
				ZkVerificationStatus::Verified
			} else {
				ZkVerificationStatus::Failed
			};
			Tasks::<T>::insert(task_id, &task);

			Self::remove_from_pending(task_id);
			let miner_pending = MinerPendingCount::<T>::get(&task.miner);
			MinerPendingCount::<T>::insert(&task.miner, miner_pending.saturating_sub(1));

			if verified {
				VerifiedTasks::<T>::try_mutate(|history| {
					history.try_push(task_id).map_err(|_| Error::<T>::TooManyVerifiedTasks)
				})?;
				Self::increase_score(&task.miner);
			} else {
				Self::decrease_score(&task.miner);
				let moved = T::Currency::repatriate_reserved(
					&task.miner,
					&Self::account_id(),
					task.submission_deposit,
					BalanceStatus::Free,
				)?;
				let slashed = task.submission_deposit.saturating_sub(moved);
				if !slashed.is_zero() {
					Self::deposit_event(Event::DepositSlashed {
						task_id,
						miner: task.miner.clone(),
						amount: slashed,
					});
				}
			}

			Self::deposit_event(Event::ProofVerified {
				task_id,
				verifier,
				status: task.status,
			});

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::claim_reward())]
		#[transactional]
		pub fn claim_reward(origin: OriginFor<T>, task_id: T::TaskId) -> DispatchResult {
			let miner = ensure_signed(origin)?;
			let mut task = Tasks::<T>::get(task_id).ok_or(Error::<T>::TaskNotFound)?;
			ensure!(task.miner == miner, Error::<T>::NotTaskMiner);
			ensure!(task.status == ZkVerificationStatus::Verified, Error::<T>::TaskNotVerified);
			ensure!(!task.reward_claimed, Error::<T>::RewardAlreadyClaimed);

			let reward = Self::calculate_reward(&task)?;
			T::Currency::transfer(&Self::account_id(), &task.miner, reward, AllowDeath)
				.map_err(|_| Error::<T>::BalanceTransferFailed)?;
			let _ = T::Currency::unreserve(&task.miner, task.submission_deposit);

			task.reward_claimed = true;
			Tasks::<T>::insert(task_id, task.clone());

			Self::deposit_event(Event::RewardClaimed {
				task_id,
				miner,
				reward,
			});
			Ok(())
		}
		/// Submit ZK verification result from off-chain worker (unsigned transaction)
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::verify_task())]
		#[transactional]
		pub fn submit_verification_unsigned(
			origin: OriginFor<T>,
			task_id: T::TaskId,
			verified: bool,
		) -> DispatchResult {
			ensure_none(origin)?;
			let mut task = Tasks::<T>::get(task_id).ok_or(Error::<T>::TaskNotFound)?;
			ensure!(task.status == ZkVerificationStatus::Pending, Error::<T>::InvalidTaskStatus);

			task.status = if verified {
				ZkVerificationStatus::Verified
			} else {
				ZkVerificationStatus::Failed
			};
			Tasks::<T>::insert(task_id, &task);

			Self::remove_from_pending(task_id);
			let miner_pending = MinerPendingCount::<T>::get(&task.miner);
			MinerPendingCount::<T>::insert(&task.miner, miner_pending.saturating_sub(1));

			if verified {
				let _ = VerifiedTasks::<T>::try_mutate(|history| {
					history.try_push(task_id)
				});
				Self::increase_score(&task.miner);
			} else {
				Self::decrease_score(&task.miner);
				let _ = T::Currency::repatriate_reserved(
					&task.miner,
					&Self::account_id(),
					task.submission_deposit,
					BalanceStatus::Free,
				);
			}

			Self::deposit_event(Event::ProofVerifiedByOcw {
				task_id,
				verified,
			});

			Ok(())
		}
	}


	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match call {
				Call::submit_verification_unsigned { .. } => {
					ValidTransaction::with_tag_prefix("zk-verify")
						.priority(UNSIGNED_TXS_PRIORITY)
						.longevity(5)
						.propagate(true)
						.build()
				},
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	impl<T: Config> Pallet<T> {
		fn ocw_verify_pending_tasks() -> Result<(), Error<T>> {
			let pending = PendingTasks::<T>::get();
			for task_id in pending.iter() {
				if let Some(task) = Tasks::<T>::get(task_id) {
					if task.status != ZkVerificationStatus::Pending {
						continue;
					}
					let verified = T::ZkVerifier::verify(task.proof.as_ref(), task.dimensions);
					let call = Call::submit_verification_unsigned {
						task_id: *task_id,
						verified,
					};
					let _ = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into());
				}
			}
			Ok(())
		}

		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		fn valid_dimensions((m, n, k): (u32, u32, u32)) -> bool {
			if m == 0 || n == 0 || k == 0 {
				return false;
			}
			let volume = (m as u128)
				.checked_mul(n as u128)
				.and_then(|v| v.checked_mul(k as u128));
			volume.is_some()
		}

		fn calculate_reward(task: &ZkTaskOf<T>) -> Result<BalanceOf<T>, Error<T>> {
			let (m, n, k) = task.dimensions;
			let volume = (m as u128)
				.checked_mul(n as u128)
				.and_then(|v| v.checked_mul(k as u128))
				.ok_or(Error::<T>::ArithmeticOverflow)?;

			let base_reward_u128: u128 = task.base_reward.saturated_into::<u128>();
			let numerator = base_reward_u128
				.checked_mul(volume)
				.and_then(|v| v.checked_mul(task.multiplier_q100 as u128))
				.ok_or(Error::<T>::ArithmeticOverflow)?;
			let reward_u128 = numerator
				.checked_div(100_000_000u128)
				.ok_or(Error::<T>::ArithmeticOverflow)?;

			Ok(reward_u128.saturated_into::<BalanceOf<T>>())
		}

		fn remove_from_pending(task_id: T::TaskId) {
			PendingTasks::<T>::mutate(|queue| {
				if let Some(index) = queue.iter().position(|id| *id == task_id) {
					queue.swap_remove(index);
				}
			});
		}

		fn current_miner_score(miner: &T::AccountId) -> u32 {
			MinerScores::<T>::get(miner).unwrap_or_else(T::InitialMinerScore::get)
		}

		fn increase_score(miner: &T::AccountId) {
			let score = Self::current_miner_score(miner)
				.saturating_add(T::ScoreOnSuccess::get())
				.min(T::MaxMinerScore::get());
			MinerScores::<T>::insert(miner, score);
			Self::deposit_event(Event::MinerScoreUpdated { miner: miner.clone(), score });
		}

		fn decrease_score(miner: &T::AccountId) {
			let score = Self::current_miner_score(miner).saturating_sub(T::ScorePenaltyOnFailure::get());
			MinerScores::<T>::insert(miner, score);
			Self::deposit_event(Event::MinerScoreUpdated { miner: miner.clone(), score });
		}
	}
}


