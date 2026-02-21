#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub mod weights;

use frame_support::{
    traits::Currency,
    weights::Weight,
};
use dbc_support::traits::TaskCompletionHandler;

// Re-export BalanceOf for use in trait implementations
type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub type PoolId = u64;
pub type TaskId = u64;



#[frame_support::pallet]
pub mod pallet {
    use frame_support::traits::StorageVersion;
    use super::*;
    use crate::weights::WeightInfo;
    use codec::{Decode, Encode, MaxEncodedLen};
    use frame_support::{
        dispatch::DispatchResult,
        pallet_prelude::*,
        traits::{tokens::BalanceStatus, Currency, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{Saturating, UniqueSaturatedInto, Zero};
    use sp_runtime::{ArithmeticError, RuntimeDebug};
    use sp_std::vec::Vec;

    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum PoolStatus {
        Active,
        Inactive,
        Deregistered,
    }

    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum TaskPriority {
        Low,
        Normal,
        High,
        Critical,
    }

    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum TaskStatus {
        Pending,
        Assigned,
        Computing,
        ProofSubmitted,
        Verifying,
        Completed,
        Failed,
    }

    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct TaskDimensions {
        pub m: u32,
        pub n: u32,
        pub k: u32,
    }

    #[derive(
        Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default,
    )]
    pub struct PoolScore {
        pub reputation_score: u32,
        pub success_rate_score: u32,
        pub price_score: u32,
        pub nvlink_score: u32,
        pub final_score: u32,
    }

    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(MaxGpuModelLen))]
    pub struct ComputePool<AccountId, Balance, MaxGpuModelLen: Get<u32>> {
        pub pool_id: PoolId,
        pub owner: AccountId,
        pub gpu_model: BoundedVec<u8, MaxGpuModelLen>,
        pub gpu_memory: u32,
        pub has_nvlink: bool,
        pub nvlink_efficiency: u32,
        pub price_per_task: Balance,
        pub reputation: u32,
        pub success_rate: u32,
        pub total_tasks: u32,
        pub completed_tasks: u32,
        pub failed_tasks: u32,
        pub status: PoolStatus,
        pub deposit_held: Balance,
        pub score: PoolScore,
    }

    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct ComputeTask<AccountId, BlockNumber, Balance> {
        pub task_id: TaskId,
        pub user: AccountId,
        pub pool_id: PoolId,
        pub dimensions: TaskDimensions,
        pub priority: TaskPriority,
        pub status: TaskStatus,
        pub submitted_at: BlockNumber,
        pub proof_hash: Option<[u8; 32]>,
        pub verification_result: Option<bool>,
        pub reward_amount: Option<Balance>,
    }

    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct ReputationInfo {
        pub reputation: u32,
        pub total_tasks: u32,
        pub successful_tasks: u32,
        pub failed_tasks: u32,
    }

    impl Default for ReputationInfo {
        fn default() -> Self {
            Self { reputation: 80, total_tasks: 0, successful_tasks: 0, failed_tasks: 0 }
        }
    }

    #[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct TaskEscrow<AccountId, Balance> {
        pub user: AccountId,
        pub pool_owner: AccountId,
        pub reward_amount: Balance,
        pub task_deposit: Balance,
        pub claimed: bool,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        #[pallet::constant]
        type PoolDeposit: Get<BalanceOf<Self>>;
        #[pallet::constant]
        type TaskDeposit: Get<BalanceOf<Self>>;
        #[pallet::constant]
        type FailureSlash: Get<BalanceOf<Self>>;
        #[pallet::constant]
        type TaskTimeout: Get<BlockNumberFor<Self>>;
        #[pallet::constant]
        type MaxGpuModelLen: Get<u32>;
        #[pallet::constant]
        type MinPoolStake: Get<BalanceOf<Self>>;
        #[pallet::constant]
        type StakeSlashPercent: Get<u32>;
        #[pallet::constant]
        type MaxTasksPerPool: Get<u32>;
        #[pallet::constant]
        type InitialReputation: Get<u32>;
        type WeightInfo: WeightInfo;
        /// Handler to notify when a task is completed
        type OnTaskCompleted: dbc_support::traits::TaskCompletionHandler<AccountId = Self::AccountId>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn next_pool_id)]
    pub type NextPoolId<T: Config> = StorageValue<_, PoolId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_task_id)]
    pub type NextTaskId<T: Config> = StorageValue<_, TaskId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pools)]
    pub type Pools<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        PoolId,
        ComputePool<T::AccountId, BalanceOf<T>, T::MaxGpuModelLen>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pool_by_owner)]
    pub type PoolByOwner<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, PoolId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn tasks)]
    pub type Tasks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        TaskId,
        ComputeTask<T::AccountId, BlockNumberFor<T>, BalanceOf<T>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pool_tasks)]
    pub type PoolTasks<T: Config> =
        StorageMap<_, Blake2_128Concat, PoolId, BoundedVec<TaskId, T::MaxTasksPerPool>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn active_task_count)]
    pub type ActiveTaskCount<T: Config> = StorageMap<_, Blake2_128Concat, PoolId, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn miner_reputation)]
    pub type MinerReputation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ReputationInfo, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn rewards)]
    pub type Rewards<T: Config> =
        StorageMap<_, Blake2_128Concat, TaskId, BalanceOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn task_escrow)]
    pub type TaskEscrowStore<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        TaskId,
        TaskEscrow<T::AccountId, BalanceOf<T>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pool_stakes)]
    pub type PoolStakes<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, PoolId, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn total_pool_stake)]
    pub type TotalPoolStake<T: Config> =
        StorageMap<_, Blake2_128Concat, PoolId, BalanceOf<T>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PoolRegistered { pool_id: PoolId, owner: T::AccountId },
        PoolConfigUpdated { pool_id: PoolId },
        PoolDeregistered { pool_id: PoolId, owner: T::AccountId },
        TaskSubmitted { task_id: TaskId, user: T::AccountId },
        TaskAssigned { task_id: TaskId, pool_id: PoolId, final_score: u32 },
        TaskStatusChanged { task_id: TaskId, status: TaskStatus },
        ProofSubmitted { task_id: TaskId, pool_id: PoolId },
        ProofVerified { task_id: TaskId, result: bool },
        RewardAvailable { task_id: TaskId, amount: BalanceOf<T> },
        RewardClaimed { task_id: TaskId, pool_owner: T::AccountId, amount: BalanceOf<T> },
        VerificationDisputed { task_id: TaskId, user: T::AccountId },
        TaskTimedOut { task_id: TaskId },
        PoolSlashed { pool_id: PoolId, amount: BalanceOf<T> },
        Staked { who: T::AccountId, pool_id: PoolId, amount: BalanceOf<T> },
        Unstaked { who: T::AccountId, pool_id: PoolId, amount: BalanceOf<T> },
        StakeSlashed { pool_id: PoolId, amount: BalanceOf<T> },
    }

    #[pallet::error]
    pub enum Error<T> {
        PoolAlreadyExists,
        PoolNotFound,
        NotPoolOwner,
        PoolInactive,
        InvalidNvlinkEfficiency,
        InvalidDimensions,
        TaskNotFound,
        NoAvailablePool,
        InvalidTaskState,
        NotTaskUser,
        NotAssignedPoolOwner,
        InvalidProof,
        RewardNotAvailable,
        RewardAlreadyClaimed,
        TaskExpired,
        DisputeNotAllowed,
        TooManyActiveTasks,
        ActiveTasksExist,
        InsufficientBalance,
        ArithmeticOverflow,
        InsufficientStake,
        StakeNotFound,
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
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut expired: Vec<TaskId> = Vec::new();
            let mut reads: u64 = 0;

            // Only iterate active tasks via PoolTasks (bounded per pool by MaxTasksPerPool)
            // instead of Tasks::iter() which grows unbounded
            for (_pool_id, task_ids) in PoolTasks::<T>::iter() {
                for task_id in task_ids.iter() {
                    reads = reads.saturating_add(1);
                    if let Some(task) = Tasks::<T>::get(task_id) {
                        if !Self::is_terminal(&task.status)
                            && now > task.submitted_at.saturating_add(T::TaskTimeout::get())
                        {
                            expired.push(*task_id);
                        }
                    }
                }
            }

            let expired_count = expired.len() as u64;
            for task_id in expired {
                let _ = Self::mark_task_failed(task_id, true);
            }

            // Return accurate weight based on actual work done
            T::DbWeight::get().reads(reads.saturating_add(1))
                .saturating_add(T::DbWeight::get().reads_writes(
                    expired_count.saturating_mul(5),
                    expired_count.saturating_mul(5),
                ))
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_pool())]
        pub fn register_pool(
            origin: OriginFor<T>,
            gpu_model: BoundedVec<u8, T::MaxGpuModelLen>,
            gpu_memory: u32,
            has_nvlink: bool,
            nvlink_efficiency: u32,
            price_per_task: BalanceOf<T>,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;
            ensure!(!PoolByOwner::<T>::contains_key(&owner), Error::<T>::PoolAlreadyExists);
            ensure!(gpu_memory > 0, Error::<T>::InvalidDimensions);
            ensure!(!price_per_task.is_zero(), Error::<T>::InsufficientBalance);
            Self::ensure_nvlink(has_nvlink, nvlink_efficiency)?;

            T::Currency::reserve(&owner, T::PoolDeposit::get())
                .map_err(|_| Error::<T>::InsufficientBalance)?;
            let pool_id = NextPoolId::<T>::get();
            let pool = ComputePool::<T::AccountId, BalanceOf<T>, T::MaxGpuModelLen> {
                pool_id,
                owner: owner.clone(),
                gpu_model,
                gpu_memory,
                has_nvlink,
                nvlink_efficiency,
                price_per_task,
                reputation: T::InitialReputation::get().min(100),
                success_rate: 100,
                total_tasks: 0,
                completed_tasks: 0,
                failed_tasks: 0,
                status: PoolStatus::Active,
                deposit_held: T::PoolDeposit::get(),
                score: PoolScore::default(),
            };

            Pools::<T>::insert(pool_id, pool);
            PoolByOwner::<T>::insert(&owner, pool_id);
            MinerReputation::<T>::insert(
                &owner,
                ReputationInfo {
                    reputation: T::InitialReputation::get().min(100),
                    total_tasks: 0,
                    successful_tasks: 0,
                    failed_tasks: 0,
                },
            );
            ActiveTaskCount::<T>::insert(pool_id, 0);
            let next_pool_id = pool_id.checked_add(1).ok_or(Error::<T>::ArithmeticOverflow)?;
            NextPoolId::<T>::put(next_pool_id);

            Self::deposit_event(Event::PoolRegistered { pool_id, owner });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_pool_config())]
        pub fn update_pool_config(
            origin: OriginFor<T>,
            pool_id: PoolId,
            gpu_model: BoundedVec<u8, T::MaxGpuModelLen>,
            gpu_memory: u32,
            has_nvlink: bool,
            nvlink_efficiency: u32,
            price_per_task: BalanceOf<T>,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(gpu_memory > 0, Error::<T>::InvalidDimensions);
            ensure!(!price_per_task.is_zero(), Error::<T>::InsufficientBalance);
            Self::ensure_nvlink(has_nvlink, nvlink_efficiency)?;

            Pools::<T>::try_mutate(pool_id, |maybe_pool| -> DispatchResult {
                let pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;
                ensure!(pool.owner == sender, Error::<T>::NotPoolOwner);
                ensure!(
                    matches!(pool.status, PoolStatus::Active | PoolStatus::Inactive),
                    Error::<T>::PoolInactive
                );

                pool.gpu_model = gpu_model;
                pool.gpu_memory = gpu_memory;
                pool.has_nvlink = has_nvlink;
                pool.nvlink_efficiency = nvlink_efficiency;
                pool.price_per_task = price_per_task;

                Ok(())
            })?;

            Self::deposit_event(Event::PoolConfigUpdated { pool_id });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::deregister_pool())]
        pub fn deregister_pool(origin: OriginFor<T>, pool_id: PoolId) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
            ensure!(pool.owner == sender, Error::<T>::NotPoolOwner);
            ensure!(ActiveTaskCount::<T>::get(pool_id) == 0, Error::<T>::ActiveTasksExist);

            let owner = pool.owner.clone();
            let held = pool.deposit_held;
            Pools::<T>::remove(pool_id);
            PoolTasks::<T>::remove(pool_id);
            ActiveTaskCount::<T>::remove(pool_id);
            PoolByOwner::<T>::remove(&owner);
            let _ = T::Currency::unreserve(&owner, held);

            Self::deposit_event(Event::PoolDeregistered { pool_id, owner });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::submit_task())]
        pub fn submit_task(
            origin: OriginFor<T>,
            dimensions: TaskDimensions,
            priority: TaskPriority,
            preferred_pool_id: Option<PoolId>,
        ) -> DispatchResult {
            let user = ensure_signed(origin)?;
            ensure!(
                dimensions.m > 0 && dimensions.n > 0 && dimensions.k > 0,
                Error::<T>::InvalidDimensions
            );

            let task_id = NextTaskId::<T>::get();
            let now = frame_system::Pallet::<T>::block_number();
            let task = ComputeTask::<T::AccountId, BlockNumberFor<T>, BalanceOf<T>> {
                task_id,
                user: user.clone(),
                pool_id: 0,
                dimensions,
                priority,
                status: TaskStatus::Pending,
                submitted_at: now,
                proof_hash: None,
                verification_result: None,
                reward_amount: None,
            };
            Tasks::<T>::insert(task_id, task.clone());
            Self::deposit_event(Event::TaskSubmitted { task_id, user: user.clone() });
            Self::deposit_event(Event::TaskStatusChanged { task_id, status: TaskStatus::Pending });

            let (pool_id, score) = if let Some(pool_id) = preferred_pool_id {
                let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
                ensure!(matches!(pool.status, PoolStatus::Active), Error::<T>::PoolInactive);
                ensure!(
                    ActiveTaskCount::<T>::get(pool_id) < T::MaxTasksPerPool::get(),
                    Error::<T>::TooManyActiveTasks
                );
                ensure!(pool.gpu_memory >= task.dimensions.k, Error::<T>::NoAvailablePool);

                let score = Self::calculate_pool_score(
                    &pool,
                    pool.price_per_task.unique_saturated_into(),
                    pool.price_per_task.unique_saturated_into(),
                );
                Pools::<T>::mutate(pool_id, |maybe_pool| {
                    if let Some(pool) = maybe_pool {
                        pool.score = score.clone();
                    }
                });
                (pool_id, score)
            } else {
                Self::select_best_pool_for_task(&task)?
            };
            let pool = Pools::<T>::get(pool_id).ok_or(Error::<T>::PoolNotFound)?;
            ensure!(
                ActiveTaskCount::<T>::get(pool_id) < T::MaxTasksPerPool::get(),
                Error::<T>::TooManyActiveTasks
            );

            let reward = Self::calculate_reward(
                &task.dimensions,
                &pool.price_per_task,
                pool.has_nvlink,
                pool.nvlink_efficiency,
            )?;
            let task_deposit = T::TaskDeposit::get();
            let total_reserved = reward.saturating_add(task_deposit);
            T::Currency::reserve(&user, total_reserved)
                .map_err(|_| Error::<T>::InsufficientBalance)?;

            Tasks::<T>::try_mutate(task_id, |maybe_task| -> DispatchResult {
                let t = maybe_task.as_mut().ok_or(Error::<T>::TaskNotFound)?;
                t.pool_id = pool_id;
                t.status = TaskStatus::Assigned;
                t.reward_amount = Some(reward);
                Ok(())
            })?;
            Self::deposit_event(Event::TaskAssigned {
                task_id,
                pool_id,
                final_score: score.final_score,
            });
            Self::deposit_event(Event::TaskStatusChanged { task_id, status: TaskStatus::Assigned });

            Tasks::<T>::try_mutate(task_id, |maybe_task| -> DispatchResult {
                let t = maybe_task.as_mut().ok_or(Error::<T>::TaskNotFound)?;
                t.status = TaskStatus::Computing;
                Ok(())
            })?;
            Self::deposit_event(Event::TaskStatusChanged {
                task_id,
                status: TaskStatus::Computing,
            });

            PoolTasks::<T>::try_mutate(pool_id, |task_ids| {
                task_ids.try_push(task_id).map_err(|_| Error::<T>::TooManyActiveTasks)
            })?;
            ActiveTaskCount::<T>::mutate(pool_id, |v| *v = v.saturating_add(1));

            TaskEscrowStore::<T>::insert(
                task_id,
                TaskEscrow {
                    user,
                    pool_owner: pool.owner,
                    reward_amount: reward,
                    task_deposit,
                    claimed: false,
                },
            );
            let next_task_id = task_id.checked_add(1).ok_or(Error::<T>::ArithmeticOverflow)?;
            NextTaskId::<T>::put(next_task_id);
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::submit_proof())]
        pub fn submit_proof(
            origin: OriginFor<T>,
            task_id: TaskId,
            proof_hash: [u8; 32],
            verification_result: bool,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(proof_hash != [0u8; 32], Error::<T>::InvalidProof);

            let task = Tasks::<T>::get(task_id).ok_or(Error::<T>::TaskNotFound)?;
            ensure!(matches!(task.status, TaskStatus::Computing), Error::<T>::InvalidTaskState);
            let pool = Pools::<T>::get(task.pool_id).ok_or(Error::<T>::PoolNotFound)?;
            ensure!(pool.owner == sender, Error::<T>::NotAssignedPoolOwner);

            let now = frame_system::Pallet::<T>::block_number();
            ensure!(
                now <= task.submitted_at.saturating_add(T::TaskTimeout::get()),
                Error::<T>::TaskExpired
            );

            Tasks::<T>::try_mutate(task_id, |maybe_task| -> DispatchResult {
                let t = maybe_task.as_mut().ok_or(Error::<T>::TaskNotFound)?;
                t.proof_hash = Some(proof_hash);
                t.status = TaskStatus::ProofSubmitted;
                Ok(())
            })?;
            Self::deposit_event(Event::ProofSubmitted { task_id, pool_id: task.pool_id });
            Self::deposit_event(Event::TaskStatusChanged {
                task_id,
                status: TaskStatus::ProofSubmitted,
            });

            Tasks::<T>::try_mutate(task_id, |maybe_task| -> DispatchResult {
                let t = maybe_task.as_mut().ok_or(Error::<T>::TaskNotFound)?;
                t.status = TaskStatus::Verifying;
                Ok(())
            })?;
            Self::deposit_event(Event::TaskStatusChanged {
                task_id,
                status: TaskStatus::Verifying,
            });

            Tasks::<T>::try_mutate(task_id, |maybe_task| -> DispatchResult {
                let t = maybe_task.as_mut().ok_or(Error::<T>::TaskNotFound)?;
                t.verification_result = Some(verification_result);
                t.status =
                    if verification_result { TaskStatus::Completed } else { TaskStatus::Failed };
                Ok(())
            })?;

            Self::deposit_event(Event::ProofVerified { task_id, result: verification_result });
            Self::deposit_event(Event::TaskStatusChanged {
                task_id,
                status: if verification_result {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                },
            });

            Self::decrement_pool_activity(task.pool_id, task_id);
            Self::update_reputation(task.pool_id, verification_result);

            if verification_result {
                if let Some(amount) = Tasks::<T>::get(task_id).and_then(|t| t.reward_amount) {
                    Rewards::<T>::insert(task_id, amount);
                    Self::deposit_event(Event::RewardAvailable { task_id, amount });
                }

                // Notify attestation system about task completion
                let final_task = Tasks::<T>::get(task_id).ok_or(Error::<T>::TaskNotFound)?;
                let pool = Pools::<T>::get(final_task.pool_id).ok_or(Error::<T>::PoolNotFound)?;
                let result_hash = sp_core::H256::from(proof_hash);
                
                // Call the completion handler (ignore errors to not block the flow)
                let _ = T::OnTaskCompleted::on_task_completed(
                    &pool.owner,
                    task_id,
                    result_hash,
                    &[], // model_id placeholder
                    0,   // input_tokens placeholder
                    0,   // output_tokens placeholder
                );
            } else {
                Self::release_escrow(task_id)?;
                Self::slash_pool(task.pool_id)?;
            }

            Ok(())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::claim_reward())]
        pub fn claim_reward(origin: OriginFor<T>, task_id: TaskId) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let task = Tasks::<T>::get(task_id).ok_or(Error::<T>::TaskNotFound)?;
            ensure!(matches!(task.status, TaskStatus::Completed), Error::<T>::InvalidTaskState);
            ensure!(task.verification_result == Some(true), Error::<T>::RewardNotAvailable);

            let pool = Pools::<T>::get(task.pool_id).ok_or(Error::<T>::PoolNotFound)?;
            ensure!(pool.owner == sender, Error::<T>::NotAssignedPoolOwner);

            let reward = Rewards::<T>::get(task_id).ok_or(Error::<T>::RewardNotAvailable)?;
            TaskEscrowStore::<T>::try_mutate_exists(task_id, |maybe_escrow| -> DispatchResult {
                let escrow = maybe_escrow.as_mut().ok_or(Error::<T>::RewardNotAvailable)?;
                ensure!(!escrow.claimed, Error::<T>::RewardAlreadyClaimed);

                let remainder = T::Currency::repatriate_reserved(
                    &escrow.user,
                    &sender,
                    reward,
                    BalanceStatus::Free,
                )
                .map_err(|_| Error::<T>::InsufficientBalance)?;
                ensure!(remainder.is_zero(), Error::<T>::InsufficientBalance);
                let _ = T::Currency::unreserve(&escrow.user, escrow.task_deposit);
                escrow.claimed = true;
                Ok(())
            })?;

            Rewards::<T>::remove(task_id);
            Self::deposit_event(Event::RewardClaimed {
                task_id,
                pool_owner: sender,
                amount: reward,
            });
            Ok(())
        }

        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::dispute_verification())]
        pub fn dispute_verification(origin: OriginFor<T>, task_id: TaskId) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            let task = Tasks::<T>::get(task_id).ok_or(Error::<T>::TaskNotFound)?;
            ensure!(task.user == sender, Error::<T>::NotTaskUser);
            ensure!(
                matches!(task.status, TaskStatus::Completed | TaskStatus::Failed),
                Error::<T>::DisputeNotAllowed
            );

            if task.verification_result == Some(true) {
                Tasks::<T>::try_mutate(task_id, |maybe_task| -> DispatchResult {
                    let t = maybe_task.as_mut().ok_or(Error::<T>::TaskNotFound)?;
                    t.status = TaskStatus::Failed;
                    t.verification_result = Some(false);
                    Ok(())
                })?;
                Self::update_reputation(task.pool_id, false);
                Self::release_escrow(task_id)?;
                Self::slash_pool(task.pool_id)?;
                Self::deposit_event(Event::TaskStatusChanged {
                    task_id,
                    status: TaskStatus::Failed,
                });
            } else if task.verification_result == Some(false) {
                Tasks::<T>::try_mutate(task_id, |maybe_task| -> DispatchResult {
                    let t = maybe_task.as_mut().ok_or(Error::<T>::TaskNotFound)?;
                    t.status = TaskStatus::Completed;
                    t.verification_result = Some(true);
                    Ok(())
                })?;
                if let Some(amount) = task.reward_amount {
                    Rewards::<T>::insert(task_id, amount);
                    Self::deposit_event(Event::RewardAvailable { task_id, amount });
                }
                Self::update_reputation(task.pool_id, true);
                Self::deposit_event(Event::TaskStatusChanged {
                    task_id,
                    status: TaskStatus::Completed,
                });
            } else {
                return Err(Error::<T>::DisputeNotAllowed.into());
            }

            Self::deposit_event(Event::VerificationDisputed { task_id, user: sender });
            Ok(())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::stake_to_pool())]
        pub fn stake_to_pool(
            origin: OriginFor<T>,
            pool_id: PoolId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(Pools::<T>::contains_key(pool_id), Error::<T>::PoolNotFound);
            T::Currency::reserve(&who, amount).map_err(|_| Error::<T>::InsufficientBalance)?;
            PoolStakes::<T>::mutate(pool_id, &who, |stake| *stake = stake.saturating_add(amount));
            TotalPoolStake::<T>::mutate(pool_id, |total| *total = total.saturating_add(amount));
            Self::deposit_event(Event::Staked { who, pool_id, amount });
            Ok(())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::unstake_from_pool())]
        pub fn unstake_from_pool(
            origin: OriginFor<T>,
            pool_id: PoolId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let current_stake = PoolStakes::<T>::get(pool_id, &who);
            ensure!(current_stake >= amount, Error::<T>::StakeNotFound);
            T::Currency::unreserve(&who, amount);
            PoolStakes::<T>::mutate(pool_id, &who, |stake| *stake = stake.saturating_sub(amount));
            TotalPoolStake::<T>::mutate(pool_id, |total| *total = total.saturating_sub(amount));
            Self::deposit_event(Event::Unstaked { who, pool_id, amount });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn ensure_nvlink(has_nvlink: bool, nvlink_efficiency: u32) -> DispatchResult {
            if has_nvlink {
                ensure!(
                    (120..=150).contains(&nvlink_efficiency),
                    Error::<T>::InvalidNvlinkEfficiency
                );
            } else {
                ensure!(nvlink_efficiency == 100, Error::<T>::InvalidNvlinkEfficiency);
            }
            Ok(())
        }

        fn is_terminal(status: &TaskStatus) -> bool {
            matches!(status, TaskStatus::Completed | TaskStatus::Failed)
        }

        fn mark_task_failed(task_id: TaskId, timed_out: bool) -> DispatchResult {
            let task = Tasks::<T>::get(task_id).ok_or(Error::<T>::TaskNotFound)?;
            if Self::is_terminal(&task.status) {
                return Ok(());
            }

            Tasks::<T>::try_mutate(task_id, |maybe_task| -> DispatchResult {
                let t = maybe_task.as_mut().ok_or(Error::<T>::TaskNotFound)?;
                t.status = TaskStatus::Failed;
                t.verification_result = Some(false);
                Ok(())
            })?;

            Self::decrement_pool_activity(task.pool_id, task_id);
            Self::update_reputation(task.pool_id, false);
            Self::release_escrow(task_id)?;
            Self::slash_pool(task.pool_id)?;
            if timed_out {
                Self::deposit_event(Event::TaskTimedOut { task_id });
            }
            Self::deposit_event(Event::TaskStatusChanged { task_id, status: TaskStatus::Failed });
            Ok(())
        }

        fn release_escrow(task_id: TaskId) -> DispatchResult {
            Rewards::<T>::remove(task_id);
            if let Some(escrow) = TaskEscrowStore::<T>::take(task_id) {
                let total = escrow.reward_amount.saturating_add(escrow.task_deposit);
                let _ = T::Currency::unreserve(&escrow.user, total);
            }
            Ok(())
        }

        fn slash_pool(pool_id: PoolId) -> DispatchResult {
            Pools::<T>::try_mutate(pool_id, |maybe_pool| -> DispatchResult {
                let pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;
                if pool.deposit_held.is_zero() {
                    return Ok(());
                }
                let slash_amount = if pool.deposit_held > T::FailureSlash::get() {
                    T::FailureSlash::get()
                } else {
                    pool.deposit_held
                };
                let (_imbalance, unslashed) =
                    T::Currency::slash_reserved(&pool.owner, slash_amount);
                let actual_slashed = slash_amount.saturating_sub(unslashed);
                pool.deposit_held = pool.deposit_held.saturating_sub(actual_slashed);
                if !actual_slashed.is_zero() {
                    Self::deposit_event(Event::PoolSlashed { pool_id, amount: actual_slashed });
                }
                Ok(())
            })
        }

        fn calculate_reward(
            dimensions: &TaskDimensions,
            price_per_task: &BalanceOf<T>,
            has_nvlink: bool,
            nvlink_efficiency: u32,
        ) -> Result<BalanceOf<T>, DispatchError> {
            let complexity = (dimensions.m as u128)
                .checked_mul(dimensions.n as u128)
                .and_then(|x| x.checked_mul(dimensions.k as u128))
                .ok_or(ArithmeticError::Overflow)?;
            let complexity_factor = core::cmp::max(1u128, complexity / 1_000_000u128);
            let base_price: u128 = (*price_per_task).unique_saturated_into();
            let mut reward_u128 =
                base_price.checked_mul(complexity_factor).ok_or(ArithmeticError::Overflow)?;
            if has_nvlink {
                reward_u128 = reward_u128
                    .checked_mul(nvlink_efficiency as u128)
                    .ok_or(ArithmeticError::Overflow)?
                    / 100u128;
            }
            Ok(reward_u128.unique_saturated_into())
        }

        fn select_best_pool_for_task(
            task: &ComputeTask<T::AccountId, BlockNumberFor<T>, BalanceOf<T>>,
        ) -> Result<(PoolId, PoolScore), DispatchError> {
            let mut candidates: Vec<(
                PoolId,
                ComputePool<T::AccountId, BalanceOf<T>, T::MaxGpuModelLen>,
            )> = Vec::new();
            for (pool_id, pool) in Pools::<T>::iter().take(50) {
                if !matches!(pool.status, PoolStatus::Active) {
                    continue;
                }
                if ActiveTaskCount::<T>::get(pool_id) >= T::MaxTasksPerPool::get() {
                    continue;
                }
                if pool.gpu_memory < task.dimensions.k {
                    continue;
                }
                candidates.push((pool_id, pool));
            }
            ensure!(!candidates.is_empty(), Error::<T>::NoAvailablePool);

            let mut min_price = u128::MAX;
            let mut max_price = 0u128;
            for (_, pool) in &candidates {
                let p: u128 = pool.price_per_task.unique_saturated_into();
                if p < min_price {
                    min_price = p;
                }
                if p > max_price {
                    max_price = p;
                }
            }

            let mut best: Option<(PoolId, PoolScore)> = None;
            for (pool_id, pool) in &candidates {
                let score = Self::calculate_pool_score(pool, min_price, max_price);
                match best {
                    Some((_, ref current)) if current.final_score >= score.final_score => {},
                    _ => best = Some((*pool_id, score)),
                }
            }

            let (selected_pool, selected_score) = best.ok_or(Error::<T>::NoAvailablePool)?;
            Pools::<T>::mutate(selected_pool, |maybe_pool| {
                if let Some(pool) = maybe_pool {
                    pool.score = selected_score.clone();
                }
            });
            Ok((selected_pool, selected_score))
        }

        fn calculate_pool_score(
            pool: &ComputePool<T::AccountId, BalanceOf<T>, T::MaxGpuModelLen>,
            min_price: u128,
            max_price: u128,
        ) -> PoolScore {
            let reputation_score = pool.reputation.min(100);
            let success_rate_score = pool.success_rate.min(100);
            let price_u128: u128 = pool.price_per_task.unique_saturated_into();

            let normalized_price_score = if max_price == min_price {
                100
            } else {
                let spread = max_price.saturating_sub(min_price);
                let delta = price_u128.saturating_sub(min_price);
                (100u128.saturating_sub(delta.saturating_mul(100) / spread)) as u32
            };

            let nvlink_score = if pool.has_nvlink {
                (pool.nvlink_efficiency.min(150)).saturating_mul(100) / 150
            } else {
                0
            };

            let reputation_component = reputation_score.saturating_mul(40);
            let success_component = success_rate_score.saturating_mul(30);
            let price_component = normalized_price_score.saturating_mul(20);
            let nvlink_component = nvlink_score.saturating_mul(10);
            let final_score = reputation_component
                .saturating_add(success_component)
                .saturating_add(price_component)
                .saturating_add(nvlink_component);

            PoolScore {
                reputation_score: reputation_component,
                success_rate_score: success_component,
                price_score: price_component,
                nvlink_score: nvlink_component,
                final_score,
            }
        }

        fn decrement_pool_activity(pool_id: PoolId, task_id: TaskId) {
            ActiveTaskCount::<T>::mutate(pool_id, |count| *count = count.saturating_sub(1));
            PoolTasks::<T>::mutate(pool_id, |ids| ids.retain(|id| *id != task_id));
        }

        fn update_reputation(pool_id: PoolId, success: bool) {
            Pools::<T>::mutate(pool_id, |maybe_pool| {
                if let Some(pool) = maybe_pool {
                    pool.total_tasks = pool.total_tasks.saturating_add(1);
                    if success {
                        pool.completed_tasks = pool.completed_tasks.saturating_add(1);
                    } else {
                        pool.failed_tasks = pool.failed_tasks.saturating_add(1);
                    }
                    if pool.total_tasks > 0 {
                        pool.success_rate =
                            (pool.completed_tasks.saturating_mul(100) / pool.total_tasks).min(100);
                    }
                    pool.reputation = if success {
                        pool.reputation.saturating_add(1).min(100)
                    } else {
                        pool.reputation.saturating_sub(2)
                    };

                    MinerReputation::<T>::mutate(&pool.owner, |rep| {
                        rep.total_tasks = rep.total_tasks.saturating_add(1);
                        if success {
                            rep.successful_tasks = rep.successful_tasks.saturating_add(1);
                            rep.reputation = rep.reputation.saturating_add(1).min(100);
                        } else {
                            rep.failed_tasks = rep.failed_tasks.saturating_add(1);
                            rep.reputation = rep.reputation.saturating_sub(2);
                        }
                    });
                }
            });
        }
    }
}

// ============================================================
// Cross-Pallet Integration: TaskComputeScheduler Implementation
// ============================================================

impl<T: Config> dbc_support::traits::TaskComputeScheduler for Pallet<T> {
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;

    fn schedule_compute(
        user: &Self::AccountId,
        _model_id: &[u8],
        dimensions: (u32, u32, u32),
    ) -> Result<(u64, Self::AccountId, Self::Balance), &'static str> {

        // Find the best available pool based on reputation and status
        let mut best_pool: Option<(PoolId, ComputePool<T::AccountId, BalanceOf<T>, T::MaxGpuModelLen>)> = None;
        let mut best_score = 0u32;

        for (pool_id, pool) in Pools::<T>::iter().take(50) {
            if pool.status != PoolStatus::Active {
                continue;
            }
            
            // Calculate a simple score based on reputation and success rate
            let score = pool.reputation.saturating_add(pool.success_rate) / 2;
            if score > best_score {
                best_score = score;
                best_pool = Some((pool_id, pool));
            }
        }

        let (pool_id, pool) = best_pool.ok_or("No active pool available")?;

        // Create the task
        let task_id = NextTaskId::<T>::get();
        let next_task_id = task_id.checked_add(1).ok_or("Task ID overflow")?;
        NextTaskId::<T>::put(next_task_id);

        let task_dimensions = TaskDimensions {
            m: dimensions.0,
            n: dimensions.1,
            k: dimensions.2,
        };

        let now = frame_system::Pallet::<T>::block_number();
        let estimated_cost = pool.price_per_task;

        let task = ComputeTask {
            task_id,
            user: user.clone(),
            pool_id,
            dimensions: task_dimensions,
            priority: TaskPriority::Normal,
            status: TaskStatus::Pending,
            submitted_at: now,
            proof_hash: None,
            verification_result: None,
            reward_amount: Some(estimated_cost),
        };

        Tasks::<T>::insert(task_id, task);

        // Update pool activity
        ActiveTaskCount::<T>::mutate(pool_id, |count| *count = count.saturating_add(1));
        PoolTasks::<T>::try_mutate(pool_id, |tasks| {
            tasks.try_push(task_id).map_err(|_| "Pool task limit reached")
        })?;

        // Update task status to Computing
        Tasks::<T>::try_mutate(task_id, |maybe_task| -> Result<(), &'static str> {
            let t = maybe_task.as_mut().ok_or("Task not found")?;
            t.status = TaskStatus::Computing;
            Ok(())
        })?;

        Ok((task_id, pool.owner, estimated_cost))
    }

    fn is_task_completed(scheduler_task_id: u64) -> bool {
        Tasks::<T>::get(scheduler_task_id)
            .map(|task| matches!(task.status, TaskStatus::Completed))
            .unwrap_or(false)
    }
}
