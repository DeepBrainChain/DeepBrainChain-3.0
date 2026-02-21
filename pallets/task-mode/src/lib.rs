#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::traits::StorageVersion;
    use dbc_support::traits::DbcPrice;
    use frame_support::{
        dispatch::DispatchResult,
        pallet_prelude::*,
        traits::{Currency, ReservableCurrency},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;
    use sp_runtime::{traits::{CheckedAdd, SaturatedConversion}, Percent};

    use crate::weights::WeightInfo;

    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum TaskOrderStatus {
        Pending,
        InProgress,
        Completed,
        Settled,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct TaskDefinition<T: Config> {
        pub model_id: BoundedVec<u8, T::MaxModelIdLen>,
        pub version: BoundedVec<u8, T::MaxModelIdLen>,
        pub admin: T::AccountId,
        pub input_price_usd_per_1k: BalanceOf<T>,
        pub output_price_usd_per_1k: BalanceOf<T>,
        pub max_tokens_per_request: u64,
        pub policy_cid: BoundedVec<u8, T::MaxPolicyCidLen>,
        pub is_active: bool,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct TaskOrder<AccountId, BlockNumber, Balance> {
        pub order_id: u64,
        pub task_id: u64,
        pub customer: AccountId,
        pub miner: AccountId,
        pub input_tokens: u64,
        pub output_tokens: u64,
        pub dbc_price_snapshot: Balance,
        pub total_dbc_charged: Balance,
        pub dbc_burned: Balance,
        pub miner_payout: Balance,
        pub created_at: BlockNumber,
        pub status: TaskOrderStatus,
        pub attestation_hash: Option<[u8; 32]>,
    }

    #[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct EraTaskStats<Balance> {
        pub total_charged: Balance,
        pub total_burned: Balance,
        pub total_miner_payout: Balance,
        pub completed_orders: u64,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type Currency: ReservableCurrency<Self::AccountId, Balance = u128>;

        type DbcPriceProvider: DbcPrice<Balance = BalanceOf<Self>>;

        #[pallet::constant]
        type TreasuryAccount: Get<Self::AccountId>;

        #[pallet::constant]
        type BurnPercentage: Get<Percent>;

        #[pallet::constant]
        type MinerPayoutPercentage: Get<Percent>;

        #[pallet::constant]
        type TaskModeRewardPercentage: Get<Percent>;

        #[pallet::constant]
        type EraDuration: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type MaxModelIdLen: Get<u32>;

        #[pallet::constant]
        type MaxPolicyCidLen: Get<u32>;

        /// Maximum blocks an order can stay in InProgress before it can be cancelled
        #[pallet::constant]
        type OrderTimeout: Get<BlockNumberFor<Self>>;

        type WeightInfo: WeightInfo;

        /// Compute scheduler for task execution
        type ComputeScheduler: dbc_support::traits::TaskComputeScheduler<
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>
        >;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn next_task_id)]
    pub type NextTaskId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_order_id)]
    pub type NextOrderId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn task_definition_of)]
    pub type TaskDefinitions<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, TaskDefinition<T>>;

    #[pallet::storage]
    #[pallet::getter(fn task_order_of)]
    pub type TaskOrders<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, TaskOrder<T::AccountId, BlockNumberFor<T>, BalanceOf<T>>>;

    #[pallet::storage]
    #[pallet::getter(fn era_stats_of)]
    pub type EraStats<T: Config> =
        StorageMap<_, Twox64Concat, u32, EraTaskStats<BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn miner_task_stats_of)]
    pub type MinerTaskStats<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        u32,
        Blake2_128Concat,
        T::AccountId,
        (BalanceOf<T>, u64),
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        TaskDefinitionCreated { task_id: u64, admin: T::AccountId },
        TaskDefinitionUpdated { task_id: u64 },
        TaskOrderCreated {
            order_id: u64,
            customer: T::AccountId,
            miner: T::AccountId,
            total_dbc: BalanceOf<T>,
        },
        TaskOrderCompleted {
            order_id: u64,
            attestation_hash: [u8; 32],
        },
        TaskOrderSettled {
            order_id: u64,
            burned: BalanceOf<T>,
            miner_payout: BalanceOf<T>,
        },
        TaskOrderExpired {
            order_id: u64,
            customer: T::AccountId,
            refunded: BalanceOf<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        TaskDefinitionNotFound,
        TaskDefinitionInactive,
        TaskOrderNotFound,
        InvalidOrderStatus,
        TokenCountExceedsLimit,
        InsufficientBalance,
        PriceOracleUnavailable,
        ArithmeticOverflow,
        NotAuthorized,
        OrderNotExpired,
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

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_task_definition())]
        pub fn create_task_definition(
            origin: OriginFor<T>,
            model_id: Vec<u8>,
            version: Vec<u8>,
            input_price_usd_per_1k: BalanceOf<T>,
            output_price_usd_per_1k: BalanceOf<T>,
            max_tokens_per_request: u64,
            policy_cid: Vec<u8>,
        ) -> DispatchResult {
            let admin = ensure_signed(origin)?;
            ensure!(
                model_id.len() <= T::MaxModelIdLen::get() as usize,
                Error::<T>::ArithmeticOverflow
            );
            ensure!(
                version.len() <= T::MaxModelIdLen::get() as usize,
                Error::<T>::ArithmeticOverflow
            );
            ensure!(
                policy_cid.len() <= T::MaxPolicyCidLen::get() as usize,
                Error::<T>::ArithmeticOverflow
            );

            let task_id = NextTaskId::<T>::get();
            let next_task_id = task_id.checked_add(1).ok_or(Error::<T>::ArithmeticOverflow)?;
            NextTaskId::<T>::put(next_task_id);

            TaskDefinitions::<T>::insert(
                task_id,
                TaskDefinition {
                    model_id: model_id.try_into().map_err(|_| Error::<T>::ArithmeticOverflow)?,
                    version: version.try_into().map_err(|_| Error::<T>::ArithmeticOverflow)?,
                    admin: admin.clone(),
                    input_price_usd_per_1k,
                    output_price_usd_per_1k,
                    max_tokens_per_request,
                    policy_cid: policy_cid.try_into().map_err(|_| Error::<T>::ArithmeticOverflow)?,
                    is_active: true,
                },
            );

            Self::deposit_event(Event::TaskDefinitionCreated { task_id, admin });
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_task_definition())]
        pub fn update_task_definition(
            origin: OriginFor<T>,
            task_id: u64,
            input_price_usd_per_1k: Option<BalanceOf<T>>,
            output_price_usd_per_1k: Option<BalanceOf<T>>,
            max_tokens_per_request: Option<u64>,
            is_active: Option<bool>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            TaskDefinitions::<T>::try_mutate(task_id, |maybe_def| -> DispatchResult {
                let def = maybe_def.as_mut().ok_or(Error::<T>::TaskDefinitionNotFound)?;
                ensure!(def.admin == who, Error::<T>::NotAuthorized);

                if let Some(v) = input_price_usd_per_1k {
                    def.input_price_usd_per_1k = v;
                }
                if let Some(v) = output_price_usd_per_1k {
                    def.output_price_usd_per_1k = v;
                }
                if let Some(v) = max_tokens_per_request {
                    def.max_tokens_per_request = v;
                }
                if let Some(v) = is_active {
                    def.is_active = v;
                }

                Ok(())
            })?;

            Self::deposit_event(Event::TaskDefinitionUpdated { task_id });
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::create_task_order())]
        pub fn create_task_order(
            origin: OriginFor<T>,
            task_id: u64,
            miner: T::AccountId,
            input_tokens: u64,
            output_tokens: u64,
        ) -> DispatchResult {
            let customer = ensure_signed(origin)?;
            let task = TaskDefinitions::<T>::get(task_id).ok_or(Error::<T>::TaskDefinitionNotFound)?;
            ensure!(task.is_active, Error::<T>::TaskDefinitionInactive);

            let total_tokens = input_tokens
                .checked_add(output_tokens)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            ensure!(
                total_tokens <= task.max_tokens_per_request,
                Error::<T>::TokenCountExceedsLimit
            );

            let dbc_price_snapshot =
                T::DbcPriceProvider::get_dbc_price().ok_or(Error::<T>::PriceOracleUnavailable)?;

            let usd_value = Self::calculate_order_usd_value(
                input_tokens,
                output_tokens,
                task.input_price_usd_per_1k,
                task.output_price_usd_per_1k,
            )?;

            let total_dbc_charged = T::DbcPriceProvider::get_dbc_amount_by_value(usd_value)
                .ok_or(Error::<T>::PriceOracleUnavailable)?;

            let (dbc_burned, miner_payout) = Self::calculate_revenue_split(total_dbc_charged)?;

            T::Currency::reserve(&customer, total_dbc_charged)
                .map_err(|_| Error::<T>::InsufficientBalance)?;

            let order_id = NextOrderId::<T>::get();
            let next_order_id = order_id.checked_add(1).ok_or(Error::<T>::ArithmeticOverflow)?;
            NextOrderId::<T>::put(next_order_id);

            TaskOrders::<T>::insert(
                order_id,
                TaskOrder {
                    order_id,
                    task_id,
                    customer: customer.clone(),
                    miner: miner.clone(),
                    input_tokens,
                    output_tokens,
                    dbc_price_snapshot,
                    total_dbc_charged,
                    dbc_burned,
                    miner_payout,
                    created_at: <frame_system::Pallet<T>>::block_number(),
                    status: TaskOrderStatus::Pending,
                    attestation_hash: None,
                },
            );

            // Assigned miner starts execution immediately after order creation.
            TaskOrders::<T>::mutate(order_id, |order| {
                if let Some(order) = order {
                    order.status = TaskOrderStatus::InProgress;
                }
            });

            Self::deposit_event(Event::TaskOrderCreated {
                order_id,
                customer,
                miner,
                total_dbc: total_dbc_charged,
            });
            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::mark_order_completed())]
        pub fn mark_order_completed(
            origin: OriginFor<T>,
            order_id: u64,
            attestation_hash: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            TaskOrders::<T>::try_mutate(order_id, |maybe_order| -> DispatchResult {
                let order = maybe_order.as_mut().ok_or(Error::<T>::TaskOrderNotFound)?;
                ensure!(who == order.miner, Error::<T>::NotAuthorized);

                ensure!(
                    matches!(order.status, TaskOrderStatus::InProgress),
                    Error::<T>::InvalidOrderStatus
                );
                order.status = TaskOrderStatus::Completed;
                order.attestation_hash = Some(attestation_hash);
                Ok(())
            })?;

            Self::deposit_event(Event::TaskOrderCompleted {
                order_id,
                attestation_hash,
            });
            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::settle_task_order())]
        pub fn settle_task_order(
            origin: OriginFor<T>,
            order_id: u64,
            attestation_hash: Option<[u8; 32]>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let mut order = TaskOrders::<T>::get(order_id).ok_or(Error::<T>::TaskOrderNotFound)?;
            ensure!(
                matches!(order.status, TaskOrderStatus::Completed),
                Error::<T>::InvalidOrderStatus
            );

            let task = TaskDefinitions::<T>::get(order.task_id).ok_or(Error::<T>::TaskDefinitionNotFound)?;
            ensure!(
                caller == order.customer || caller == order.miner || caller == task.admin,
                Error::<T>::NotAuthorized
            );

            if let Some(hash) = attestation_hash {
                order.attestation_hash = Some(hash);
            }

            T::Currency::repatriate_reserved(
                &order.customer,
                &T::TreasuryAccount::get(),
                order.dbc_burned,
                frame_support::traits::BalanceStatus::Free,
            )
            .map_err(|_| Error::<T>::InsufficientBalance)?;

            T::Currency::repatriate_reserved(
                &order.customer,
                &order.miner,
                order.miner_payout,
                frame_support::traits::BalanceStatus::Free,
            )
            .map_err(|_| Error::<T>::InsufficientBalance)?;

            order.status = TaskOrderStatus::Settled;
            TaskOrders::<T>::insert(order_id, &order);

            let era = Self::block_to_era(order.created_at);
            EraStats::<T>::mutate(era, |stats| {
                stats.total_charged = stats.total_charged.saturating_add(order.total_dbc_charged);
                stats.total_burned = stats.total_burned.saturating_add(order.dbc_burned);
                stats.total_miner_payout = stats.total_miner_payout.saturating_add(order.miner_payout);
                stats.completed_orders = stats.completed_orders.saturating_add(1);
            });

            MinerTaskStats::<T>::mutate(era, &order.miner, |(total_payout, count)| {
                *total_payout = total_payout.saturating_add(order.miner_payout);
                *count = count.saturating_add(1);
            });

            Self::deposit_event(Event::TaskOrderSettled {
                order_id,
                burned: order.dbc_burned,
                miner_payout: order.miner_payout,
            });

            Ok(())
        }

        /// Cancel an expired order and refund the customer's reserved funds.
        /// Anyone can call this for orders that have exceeded the OrderTimeout.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::settle_task_order())]
        pub fn cancel_expired_order(
            origin: OriginFor<T>,
            order_id: u64,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let mut order = TaskOrders::<T>::get(order_id)
                .ok_or(Error::<T>::TaskOrderNotFound)?;
            ensure!(
                matches!(order.status, TaskOrderStatus::Pending | TaskOrderStatus::InProgress),
                Error::<T>::InvalidOrderStatus
            );

            let now = <frame_system::Pallet<T>>::block_number();
            let deadline = order.created_at
                .checked_add(&T::OrderTimeout::get())
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            ensure!(now > deadline, Error::<T>::OrderNotExpired);

            // Unreserve all funds back to customer
            T::Currency::unreserve(&order.customer, order.total_dbc_charged);

            let customer = order.customer.clone();
            let refunded = order.total_dbc_charged;
            order.status = TaskOrderStatus::Settled;
            TaskOrders::<T>::insert(order_id, &order);

            Self::deposit_event(Event::TaskOrderExpired {
                order_id,
                customer,
                refunded,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn calculate_order_usd_value(
            input_tokens: u64,
            output_tokens: u64,
            input_price_usd_per_1k: BalanceOf<T>,
            output_price_usd_per_1k: BalanceOf<T>,
        ) -> Result<u64, Error<T>> {
            let input_part = (input_tokens as u128)
                .checked_mul(input_price_usd_per_1k)
                .ok_or(Error::<T>::ArithmeticOverflow)?
                .checked_div(1000)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            let output_part = (output_tokens as u128)
                .checked_mul(output_price_usd_per_1k)
                .ok_or(Error::<T>::ArithmeticOverflow)?
                .checked_div(1000)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            let total = input_part
                .checked_add(output_part)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            u64::try_from(total).map_err(|_| Error::<T>::ArithmeticOverflow)
        }

        fn calculate_revenue_split(total: BalanceOf<T>) -> Result<(BalanceOf<T>, BalanceOf<T>), Error<T>> {
            let burned = T::BurnPercentage::get() * total;
            let miner_percent_cut = T::MinerPayoutPercentage::get() * total;

            let charged_check = burned
                .checked_add(miner_percent_cut)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            ensure!(charged_check <= total, Error::<T>::ArithmeticOverflow);

            let miner_payout = total
                .checked_sub(burned)
                .ok_or(Error::<T>::ArithmeticOverflow)?;

            Ok((burned, miner_payout))
        }

        pub fn block_to_era(block: BlockNumberFor<T>) -> u32 {
            let now: u128 = block.saturated_into::<u128>();
            let era_duration: u128 = T::EraDuration::get().saturated_into::<u128>();
            if era_duration == 0 {
                return 0
            }
            let era = now / era_duration;
            era.min(u32::MAX as u128) as u32
        }

        pub fn split_era_rewards(total_era_rewards: BalanceOf<T>) -> Result<(BalanceOf<T>, BalanceOf<T>), Error<T>> {
            let task_reward_pool = T::TaskModeRewardPercentage::get() * total_era_rewards;
            let rental_reward_pool = total_era_rewards
                .checked_sub(task_reward_pool)
                .ok_or(Error::<T>::ArithmeticOverflow)?;
            Ok((task_reward_pool, rental_reward_pool))
        }

        pub fn miner_reward_share(era_index: u32, miner: &T::AccountId, total_era_rewards: BalanceOf<T>) -> Option<BalanceOf<T>> {
            let (task_pool, _) = Self::split_era_rewards(total_era_rewards).ok()?;
            let era_stats = EraStats::<T>::get(era_index);
            if era_stats.total_miner_payout == 0 {
                return None
            }
            let (miner_total_payout, _) = MinerTaskStats::<T>::get(era_index, miner);
            if miner_total_payout == 0 {
                return None
            }

            let reward = task_pool
                .checked_mul(miner_total_payout)?
                .checked_div(era_stats.total_miner_payout)?;
            Some(reward)
        }
    }
}

// ============================================================
// Cross-Pallet Integration: TaskBillingProvider Implementation
// ============================================================

use frame_support::traits::Currency;
use dbc_support::traits::DbcPrice;

// Re-export BalanceOf for use in trait implementations
type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

impl<T: Config> dbc_support::traits::TaskBillingProvider for Pallet<T> {
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;

    fn calculate_billing(
        _model_id: &[u8],
        input_tokens: u64,
        output_tokens: u64,
    ) -> Option<Self::Balance> {
        use sp_runtime::traits::Zero;

        // Get DBC price from oracle
        let dbc_price = T::DbcPriceProvider::get_dbc_price()?;
        if dbc_price.is_zero() {
            return None;
        }

        // Simple pricing model: 1 DBC per 1000 tokens (input + output)
        // In a real implementation, this would use the TaskDefinition pricing
        let total_tokens = input_tokens.checked_add(output_tokens)?;
        let tokens_in_thousands = total_tokens.checked_div(1000)?;
        
        // Calculate DBC amount based on price
        let dbc_amount = (tokens_in_thousands as u128).checked_mul(dbc_price)?;
        
        Some(dbc_amount)
    }

    fn get_revenue_split(total: Self::Balance) -> (Self::Balance, Self::Balance) {
        use sp_runtime::Percent;
        
        // 15% burn, 85% miner
        let burn_percent = Percent::from_percent(15);
        let miner_percent = Percent::from_percent(85);
        
        let burn_amount = burn_percent * total;
        let miner_amount = miner_percent * total;
        
        (burn_amount, miner_amount)
    }
}
