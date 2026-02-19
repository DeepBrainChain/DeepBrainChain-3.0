# Pallet Task Mode - Design Document

**Version**: 1.0  
**Date**: 2026-02-17  
**Author**: 文博 (Wenbo), DBC 3.0 Engineering Director

---

## 1. Overview

`pallet-task-mode` implements the core economic model for DBC 3.0's **Task Mode** mining:

- **Token-based billing**: Charge by input/output tokens, convert to DBC via on-chain oracle
- **Revenue split**: 15% burn / 85% miner
- **Reward allocation**: 70% Task Mode / 30% Long-term Rental per era

---

## 2. Business Rules

### 2.1 Mining Modes
- **Task Mode**: Miners run designated LLM inference workloads
- **Long-term Rental Mode**: Traditional GPU rental (existing `rent-machine` logic)

### 2.2 Billing Formula
```
usd_value = (input_tokens * input_price_per_1k / 1000) + (output_tokens * output_price_per_1k / 1000)
dbc_due = DbcPrice::get_dbc_amount_by_value(usd_value)
burn_amount = dbc_due * 15%
miner_revenue = dbc_due * 85%
```

### 2.3 Reward Distribution
Per era:
```
task_reward_pool = total_era_rewards * 70%
rental_reward_pool = total_era_rewards * 30%
```

---

## 3. Data Structures

### 3.1 TaskDefinition
```rust
pub struct TaskDefinition<AccountId, Balance> {
    /// Model family (e.g., "llama3-70b", "gpt4-turbo")
    pub model_id: Vec<u8>,
    /// Model version
    pub version: Vec<u8>,
    /// Creator/admin account
    pub admin: AccountId,
    /// Input token price (USD per 1000 tokens, scaled by 1e6)
    pub input_price_usd_per_1k: Balance,
    /// Output token price (USD per 1000 tokens, scaled by 1e6)
    pub output_price_usd_per_1k: Balance,
    /// Maximum tokens per request
    pub max_tokens_per_request: u64,
    /// Task policy/SLA metadata (IPFS CID or on-chain reference)
    pub policy_cid: Vec<u8>,
    /// Is this task definition active?
    pub is_active: bool,
}
```

### 3.2 TaskOrder
```rust
pub struct TaskOrder<AccountId, BlockNumber, Balance> {
    /// Unique order ID
    pub order_id: u64,
    /// Task definition reference
    pub task_id: u64,
    /// Customer account
    pub customer: AccountId,
    /// Assigned miner account
    pub miner: AccountId,
    /// Input tokens consumed
    pub input_tokens: u64,
    /// Output tokens generated
    pub output_tokens: u64,
    /// DBC/USD exchange rate snapshot (scaled by 1e6)
    pub dbc_price_snapshot: Balance,
    /// Total DBC charged
    pub total_dbc_charged: Balance,
    /// DBC burned (15%)
    pub dbc_burned: Balance,
    /// DBC paid to miner (85%)
    pub miner_payout: Balance,
    /// Block number when order was created
    pub created_at: BlockNumber,
    /// Settlement status
    pub status: TaskOrderStatus,
    /// Result attestation hash (optional, for verification)
    pub attestation_hash: Option<[u8; 32]>,
}
```

### 3.3 TaskOrderStatus
```rust
pub enum TaskOrderStatus {
    /// Order created, awaiting execution
    Pending,
    /// Execution in progress
    InProgress,
    /// Completed successfully
    Completed,
    /// Failed or disputed
    Failed,
    /// Settlement finalized
    Settled,
}
```

### 3.4 EraTaskStats
```rust
pub struct EraTaskStats<Balance> {
    /// Total DBC charged in this era
    pub total_charged: Balance,
    /// Total DBC burned in this era
    pub total_burned: Balance,
    /// Total DBC paid to miners in this era
    pub total_miner_payout: Balance,
    /// Number of completed orders
    pub completed_orders: u64,
}
```

---

## 4. Storage Items

```rust
/// Task definitions registry
/// TaskDefinitions: map TaskId => TaskDefinition
pub type TaskDefinitions<T> = StorageMap<_, Blake2_128Concat, u64, TaskDefinition<T::AccountId, BalanceOf<T>>>;

/// Task orders ledger
/// TaskOrders: map OrderId => TaskOrder
pub type TaskOrders<T> = StorageMap<_, Blake2_128Concat, u64, TaskOrder<T::AccountId, BlockNumberFor<T>, BalanceOf<T>>>;

/// Next task ID counter
pub type NextTaskId<T> = StorageValue<_, u64, ValueQuery>;

/// Next order ID counter
pub type NextOrderId<T> = StorageValue<_, u64, ValueQuery>;

/// Era statistics
/// EraStats: map EraIndex => EraTaskStats
pub type EraStats<T> = StorageMap<_, Twox64Concat, u32, EraTaskStats<BalanceOf<T>>, ValueQuery>;

/// Miner task statistics (for reward calculation)
/// MinerTaskStats: double_map EraIndex, AccountId => (total_payout, order_count)
pub type MinerTaskStats<T> = StorageDoubleMap<
    _,
    Twox64Concat, u32,
    Blake2_128Concat, T::AccountId,
    (BalanceOf<T>, u64),
    ValueQuery,
>;
```

---

## 5. Extrinsics

### 5.1 Admin Operations

#### `create_task_definition`
```rust
#[pallet::call_index(0)]
#[pallet::weight(Weight::from_parts(10_000, 0))]
pub fn create_task_definition(
    origin: OriginFor<T>,
    model_id: Vec<u8>,
    version: Vec<u8>,
    input_price_usd_per_1k: BalanceOf<T>,
    output_price_usd_per_1k: BalanceOf<T>,
    max_tokens_per_request: u64,
    policy_cid: Vec<u8>,
) -> DispatchResult
```

#### `update_task_definition`
```rust
#[pallet::call_index(1)]
#[pallet::weight(Weight::from_parts(10_000, 0))]
pub fn update_task_definition(
    origin: OriginFor<T>,
    task_id: u64,
    input_price_usd_per_1k: Option<BalanceOf<T>>,
    output_price_usd_per_1k: Option<BalanceOf<T>>,
    max_tokens_per_request: Option<u64>,
    is_active: Option<bool>,
) -> DispatchResult
```

### 5.2 Order Lifecycle

#### `create_task_order`
```rust
#[pallet::call_index(2)]
#[pallet::weight(Weight::from_parts(50_000, 0))]
pub fn create_task_order(
    origin: OriginFor<T>,
    task_id: u64,
    miner: T::AccountId,
    input_tokens: u64,
    output_tokens: u64,
) -> DispatchResult
```
**Logic**:
1. Validate task definition exists and is active
2. Fetch current DBC price from `pallet-dbc-price-ocw`
3. Calculate USD value and convert to DBC
4. Calculate burn (15%) and miner payout (85%)
5. Reserve DBC from customer account
6. Create order with `Pending` status
7. Emit `TaskOrderCreated` event

#### `settle_task_order`
```rust
#[pallet::call_index(3)]
#[pallet::weight(Weight::from_parts(50_000, 0))]
pub fn settle_task_order(
    origin: OriginFor<T>,
    order_id: u64,
    attestation_hash: Option<[u8; 32]>,
) -> DispatchResult
```
**Logic**:
1. Verify order exists and is in `Completed` status
2. Transfer burn amount to treasury (or direct burn)
3. Transfer miner payout to miner account
4. Update order status to `Settled`
5. Update era statistics
6. Update miner task statistics for reward calculation
7. Emit `TaskOrderSettled` event

#### `mark_order_completed`
```rust
#[pallet::call_index(4)]
#[pallet::weight(Weight::from_parts(20_000, 0))]
pub fn mark_order_completed(
    origin: OriginFor<T>,
    order_id: u64,
    attestation_hash: [u8; 32],
) -> DispatchResult
```
**Logic**:
- Miner or authorized oracle marks order as `Completed`
- Stores attestation hash for verification
- Transitions order from `InProgress` -> `Completed`

---

## 6. Events

```rust
#[pallet::event]
#[pallet::generate_deposit(pub(super) fn deposit_event)]
pub enum Event<T: Config> {
    /// New task definition created [task_id, admin]
    TaskDefinitionCreated { task_id: u64, admin: T::AccountId },
    
    /// Task definition updated [task_id]
    TaskDefinitionUpdated { task_id: u64 },
    
    /// New task order created [order_id, customer, miner, total_dbc]
    TaskOrderCreated {
        order_id: u64,
        customer: T::AccountId,
        miner: T::AccountId,
        total_dbc: BalanceOf<T>,
    },
    
    /// Task order completed [order_id, attestation_hash]
    TaskOrderCompleted {
        order_id: u64,
        attestation_hash: [u8; 32],
    },
    
    /// Task order settled [order_id, burned, miner_payout]
    TaskOrderSettled {
        order_id: u64,
        burned: BalanceOf<T>,
        miner_payout: BalanceOf<T>,
    },
}
```

---

## 7. Errors

```rust
#[pallet::error]
pub enum Error<T> {
    /// Task definition not found
    TaskDefinitionNotFound,
    
    /// Task definition is not active
    TaskDefinitionInactive,
    
    /// Task order not found
    TaskOrderNotFound,
    
    /// Invalid order status for this operation
    InvalidOrderStatus,
    
    /// Token count exceeds maximum allowed
    TokenCountExceedsLimit,
    
    /// Insufficient balance to create order
    InsufficientBalance,
    
    /// DBC price oracle unavailable
    PriceOracleUnavailable,
    
    /// Arithmetic overflow in calculation
    ArithmeticOverflow,
    
    /// Not authorized to perform this action
    NotAuthorized,
}
```

---

## 8. Integration Points

### 8.1 DBC Price Oracle Integration
```rust
// Fetch current DBC price from pallet-dbc-price-ocw
let dbc_price = T::DbcPriceProvider::get_dbc_price()
    .ok_or(Error::<T>::PriceOracleUnavailable)?;

// Convert USD value to DBC amount
let dbc_amount = T::DbcPriceProvider::get_dbc_amount_by_value(usd_value)?;
```

### 8.2 Treasury/Burn Integration
```rust
// Transfer burn amount to treasury
T::Currency::transfer(
    &customer,
    &T::TreasuryAccount::get(),
    burn_amount,
    ExistenceRequirement::KeepAlive,
)?;

// OR direct burn (if supported)
// T::Currency::slash(&customer, burn_amount);
```

### 8.3 Reward Distribution Hook
```rust
impl<T: Config> Pallet<T> {
    /// Called by staking pallet at era end to calculate task mode rewards
    pub fn distribute_era_rewards(era_index: u32, task_reward_pool: BalanceOf<T>) {
        // Fetch era stats
        let era_stats = EraStats::<T>::get(era_index);
        
        // Distribute rewards proportionally to miners based on MinerTaskStats
        // (Implementation in reward distribution module)
    }
}
```

---

## 9. Configuration Trait

```rust
#[pallet::config]
pub trait Config: frame_system::Config {
    /// Event type
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    
    /// Currency type for DBC token
    type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;
    
    /// DBC price oracle provider
    type DbcPriceProvider: DbcPriceInterface<BalanceOf<Self>>;
    
    /// Treasury account for burn destination
    #[pallet::constant]
    type TreasuryAccount: Get<Self::AccountId>;
    
    /// Burn percentage (default: 15%)
    #[pallet::constant]
    type BurnPercentage: Get<Percent>;
    
    /// Miner payout percentage (default: 85%)
    #[pallet::constant]
    type MinerPayoutPercentage: Get<Percent>;
    
    /// Task mode reward percentage per era (default: 70%)
    #[pallet::constant]
    type TaskModeRewardPercentage: Get<Percent>;
    
    /// Maximum model ID length
    #[pallet::constant]
    type MaxModelIdLen: Get<u32>;
    
    /// Maximum policy CID length
    #[pallet::constant]
    type MaxPolicyCidLen: Get<u32>;
}
```

---

## 10. Testing Requirements

### Unit Tests
1. `test_create_task_definition` - Verify task creation
2. `test_create_task_order_success` - Happy path order creation
3. `test_create_task_order_insufficient_balance` - Fail on low balance
4. `test_settle_task_order` - Verify settlement logic
5. `test_burn_and_payout_calculation` - Validate 15/85 split
6. `test_era_statistics_update` - Verify era stats accumulation
7. `test_inactive_task_rejection` - Reject orders for inactive tasks
8. `test_token_limit_enforcement` - Reject oversized requests
9. `test_price_oracle_failure` - Handle oracle unavailability
10. `test_reward_distribution` - Verify 70/30 era reward split

### Integration Tests
1. End-to-end order lifecycle (create -> complete -> settle)
2. Multi-miner reward distribution across an era
3. Price oracle integration with `pallet-dbc-price-ocw`
4. Treasury burn transfer validation

---

## 11. Security Considerations

### 11.1 Oracle Manipulation Risk
**Mitigation**: 
- Use median of multiple price sources (future enhancement)
- Implement staleness checks
- Add governance kill-switch for suspicious price movements

### 11.2 Attestation Fraud
**Mitigation**:
- Require cryptographic attestation hashes
- Implement challenge window (future phase)
- Support slashing for fraudulent execution

### 11.3 Arithmetic Safety
**Mitigation**:
- Use checked arithmetic for all balance calculations
- Validate inputs against overflow conditions
- Return errors instead of panicking

### 11.4 Reentrancy Protection
**Mitigation**:
- Update state before external calls (checks-effects-interactions pattern)
- Use transactional storage for multi-step operations

---

## 12. Future Enhancements

1. **Challenge/Dispute System**: Allow customers to dispute task results within a time window
2. **Multi-Source Oracle**: Aggregate prices from multiple sources with median/outlier filtering
3. **Dynamic Pricing**: Adjust token prices based on network load and demand
4. **Batched Settlement**: Settle multiple orders in a single transaction for efficiency
5. **SLA Enforcement**: Automatic penalties for missed latency/quality targets
6. **Cross-Chain Integration**: Support settlement in bridged stablecoins (USDT/USDC)

---

## 13. References

- [DBC 3.0 Strategy Analysis](./DBC_3.0_STRATEGY_ANALYSIS.md) - Section 5
- [pallet-rent-machine](../pallets/rent-machine/src/lib.rs) - Existing billing patterns
- [pallet-dbc-price-ocw](../pallets/dbc-price-ocw/src/lib.rs) - Price oracle interface
- [pallet-online-profile](../pallets/online-profile/src/lib.rs) - Miner profile management

---

**End of Design Document**
