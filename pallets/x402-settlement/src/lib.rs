#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::traits::StorageVersion;
    use frame_support::{
        traits::EnsureOrigin,
        dispatch::DispatchResult,
        pallet_prelude::*,
        traits::{Currency, ReservableCurrency},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;
    use sp_core::H256;
    use crate::weights::WeightInfo;
    use sp_runtime::traits::{SaturatedConversion, Saturating};


    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum PaymentIntentStatus {
        Pending,
        Verified,
        Settled,
        Failed,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct PaymentIntent<T: Config> {
        pub intent_id: u64,
        pub merchant: T::AccountId,
        pub miner: T::AccountId,
        pub amount: BalanceOf<T>,
        pub nonce: u64,
        pub replay_fingerprint: H256,
        pub facilitator_signature: BoundedVec<u8, T::MaxSignatureLen>,
        pub status: PaymentIntentStatus,
        pub created_at: BlockNumberFor<T>,
        pub verified_at: Option<BlockNumberFor<T>>,
        pub settled_at: Option<BlockNumberFor<T>>,
        pub expires_at: BlockNumberFor<T>,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct SettlementReceipt<AccountId, Balance> {
        pub intent_id: u64,
        pub merchant: AccountId,
        pub miner: AccountId,
        pub amount: Balance,
        pub settled_at: u64,
        pub tx_hash: H256,
    }

    pub(crate) type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type Currency: ReservableCurrency<Self::AccountId, Balance = u128>;

        #[pallet::constant]
        type FacilitatorAccount: Get<Self::AccountId>;

        #[pallet::constant]
        type MaxSignatureLen: Get<u32>;

        #[pallet::constant]
        type SettlementDelay: Get<BlockNumberFor<Self>>;

        #[pallet::constant]
        type PaymentIntentTTL: Get<BlockNumberFor<Self>>;

        type WeightInfo: WeightInfo;

        /// Origin that can finalize settlements
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn next_intent_id)]
    pub type NextIntentId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn payment_intent_of)]
    pub type PaymentIntents<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, PaymentIntent<T>>;

    #[pallet::storage]
    #[pallet::getter(fn nonce_used)]
    pub type NonceUsed<T: Config> =
        StorageMap<_, Blake2_128Concat, (T::AccountId, u64), bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn replay_fingerprint_used)]
    pub type ReplayFingerprintUsed<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn settlement_receipt_of)]
    pub type SettlementReceipts<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, SettlementReceipt<T::AccountId, BalanceOf<T>>>;

    /// Track pending intent IDs to avoid unbounded iteration in on_initialize
    #[pallet::storage]
    pub type PendingIntentIds<T: Config> =
        StorageValue<_, BoundedVec<u64, ConstU32<10000>>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PaymentIntentSubmitted {
            intent_id: u64,
            merchant: T::AccountId,
            miner: T::AccountId,
            amount: BalanceOf<T>,
            nonce: u64,
        },
        PaymentIntentVerified {
            intent_id: u64,
            facilitator: T::AccountId,
        },
        PaymentIntentSettled {
            intent_id: u64,
            merchant: T::AccountId,
            miner: T::AccountId,
            amount: BalanceOf<T>,
        },
        PaymentIntentFailed {
            intent_id: u64,
            reason: DispatchError,
        },
        PaymentIntentExpired {
            intent_id: u64,
            merchant: T::AccountId,
            amount: BalanceOf<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidNonce,
        ReplayFingerprintUsed,
        InvalidFacilitatorSignature,
        PaymentIntentNotFound,
        InvalidPaymentIntentStatus,
        InsufficientBalance,
        SettlementDelayNotMet,
        NotAuthorized,
        ArithmeticOverflow,
        PaymentIntentExpired,
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
            let mut expired_ids = sp_std::vec::Vec::new();
            let pending = PendingIntentIds::<T>::get();
            let total_checked = pending.len() as u64;

            for id in &pending {
                if let Some(intent) = PaymentIntents::<T>::get(id) {
                    if now >= intent.expires_at && matches!(intent.status, PaymentIntentStatus::Pending) {
                        T::Currency::unreserve(&intent.merchant, intent.amount);
                        PaymentIntents::<T>::mutate(id, |maybe| {
                            if let Some(ref mut i) = maybe {
                                i.status = PaymentIntentStatus::Failed;
                            }
                        });
                        Self::deposit_event(Event::PaymentIntentExpired {
                            intent_id: *id,
                            merchant: intent.merchant,
                            amount: intent.amount,
                        });
                        expired_ids.push(*id);
                    }
                }
            }

            // Remove expired intents from pending list
            if !expired_ids.is_empty() {
                PendingIntentIds::<T>::mutate(|ids| {
                    ids.retain(|id| !expired_ids.contains(id));
                });
            }

            let expired_count = expired_ids.len() as u64;
            T::DbWeight::get().reads(total_checked.saturating_add(1))
                .saturating_add(T::DbWeight::get().writes(expired_count.saturating_mul(2).saturating_add(1)))
        }
    }


    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::submit_payment_intent())]
        pub fn submit_payment_intent(
            origin: OriginFor<T>,
            miner: T::AccountId,
            amount: BalanceOf<T>,
            nonce: u64,
            replay_fingerprint: H256,
            facilitator_signature: Vec<u8>,
        ) -> DispatchResult {
            let merchant = ensure_signed(origin)?;

            // Check nonce
            ensure!(
                !NonceUsed::<T>::contains_key((merchant.clone(), nonce)),
                Error::<T>::InvalidNonce
            );

            // Check replay fingerprint
            ensure!(
                !ReplayFingerprintUsed::<T>::contains_key(replay_fingerprint),
                Error::<T>::ReplayFingerprintUsed
            );

            // Verify facilitator signature
            let signature_bytes = facilitator_signature
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;
            ensure!(
                Self::verify_facilitator_signature(
                    &merchant,
                    &miner,
                    amount,
                    nonce,
                    replay_fingerprint,
                    &signature_bytes
                ),
                Error::<T>::InvalidFacilitatorSignature
            );

            // Reserve merchant balance
            T::Currency::reserve(&merchant, amount)
                .map_err(|_| Error::<T>::InsufficientBalance)?;

            let intent_id = NextIntentId::<T>::get();
            let next_intent_id = intent_id.checked_add(1).ok_or(Error::<T>::ArithmeticOverflow)?;
            NextIntentId::<T>::put(next_intent_id);

            // Mark nonce and replay fingerprint as used
            NonceUsed::<T>::insert((merchant.clone(), nonce), true);
            ReplayFingerprintUsed::<T>::insert(replay_fingerprint, true);

            let _ = PendingIntentIds::<T>::try_mutate(|ids| ids.try_push(intent_id));
            PaymentIntents::<T>::insert(
                intent_id,
                PaymentIntent {
                    intent_id,
                    merchant: merchant.clone(),
                    miner: miner.clone(),
                    amount,
                    nonce,
                    replay_fingerprint,
                    facilitator_signature: signature_bytes,
                    status: PaymentIntentStatus::Pending,
                    created_at: <frame_system::Pallet<T>>::block_number(),
                    verified_at: None,
                    settled_at: None,
                    expires_at: <frame_system::Pallet<T>>::block_number().saturating_add(T::PaymentIntentTTL::get()),
                },
            );

            Self::deposit_event(Event::PaymentIntentSubmitted {
                intent_id,
                merchant,
                miner,
                amount,
                nonce,
            });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::verify_settlement())]
        pub fn verify_settlement(
            origin: OriginFor<T>,
            intent_id: u64,
        ) -> DispatchResult {
            let facilitator = ensure_signed(origin)?;
            ensure!(
                facilitator == T::FacilitatorAccount::get(),
                Error::<T>::NotAuthorized
            );

            PaymentIntents::<T>::try_mutate(intent_id, |maybe_intent| -> DispatchResult {
                let intent = maybe_intent.as_mut().ok_or(Error::<T>::PaymentIntentNotFound)?;
                ensure!(
                    matches!(intent.status, PaymentIntentStatus::Pending),
                    Error::<T>::InvalidPaymentIntentStatus
                );
                ensure!(
                    <frame_system::Pallet<T>>::block_number() < intent.expires_at,
                    Error::<T>::PaymentIntentExpired
                );

                intent.status = PaymentIntentStatus::Verified;
                intent.verified_at = Some(<frame_system::Pallet<T>>::block_number());

                // Remove from pending tracking (no longer Pending status)
                PendingIntentIds::<T>::mutate(|ids| ids.retain(|id| *id != intent_id));

                Ok(())
            })?;

            Self::deposit_event(Event::PaymentIntentVerified {
                intent_id,
                facilitator,
            });

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::finalize_settlement())]
        pub fn finalize_settlement(
            origin: OriginFor<T>,
            intent_id: u64,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut intent = PaymentIntents::<T>::get(intent_id).ok_or(Error::<T>::PaymentIntentNotFound)?;
            ensure!(
                matches!(intent.status, PaymentIntentStatus::Verified),
                Error::<T>::InvalidPaymentIntentStatus
            );
            ensure!(
                <frame_system::Pallet<T>>::block_number() < intent.expires_at,
                Error::<T>::PaymentIntentExpired
            );

            // Check authorization: merchant, miner, or facilitator can finalize
            let facilitator = T::FacilitatorAccount::get();
            ensure!(
                caller == intent.merchant || caller == intent.miner || caller == facilitator,
                Error::<T>::NotAuthorized
            );

            // Check settlement delay
            let current_block = <frame_system::Pallet<T>>::block_number();
            let verified_at = intent.verified_at.ok_or(Error::<T>::InvalidPaymentIntentStatus)?;
            let delay_blocks = T::SettlementDelay::get();
            ensure!(
                current_block >= verified_at.saturating_add(delay_blocks),
                Error::<T>::SettlementDelayNotMet
            );

            // Transfer funds from merchant to miner
            T::Currency::repatriate_reserved(
                &intent.merchant,
                &intent.miner,
                intent.amount,
                frame_support::traits::BalanceStatus::Free,
            )
            .map_err(|_| Error::<T>::InsufficientBalance)?;

            // Update intent status
            intent.status = PaymentIntentStatus::Settled;
            intent.settled_at = Some(current_block);
            PaymentIntents::<T>::insert(intent_id, &intent);

            // Create settlement receipt
            let receipt = SettlementReceipt {
                intent_id,
                merchant: intent.merchant.clone(),
                miner: intent.miner.clone(),
                amount: intent.amount,
                settled_at: current_block.saturated_into(),
                tx_hash: H256::from_low_u64_be(intent_id),
            };
            SettlementReceipts::<T>::insert(intent_id, receipt);

            Self::deposit_event(Event::PaymentIntentSettled {
                intent_id,
                merchant: intent.merchant,
                miner: intent.miner,
                amount: intent.amount,
            });

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::fail_payment_intent())]
        pub fn fail_payment_intent(
            origin: OriginFor<T>,
            intent_id: u64,
        ) -> DispatchResult {
            let facilitator = ensure_signed(origin)?;
            ensure!(
                facilitator == T::FacilitatorAccount::get(),
                Error::<T>::NotAuthorized
            );

            PaymentIntents::<T>::try_mutate(intent_id, |maybe_intent| -> DispatchResult {
                let intent = maybe_intent.as_mut().ok_or(Error::<T>::PaymentIntentNotFound)?;
                ensure!(
                    matches!(intent.status, PaymentIntentStatus::Pending | PaymentIntentStatus::Verified),
                    Error::<T>::InvalidPaymentIntentStatus
                );

                // Release reserved funds back to merchant
                T::Currency::unreserve(&intent.merchant, intent.amount);

                intent.status = PaymentIntentStatus::Failed;

                // Remove from pending tracking
                PendingIntentIds::<T>::mutate(|ids| ids.retain(|id| *id != intent_id));

                Ok(())
            })?;

            Self::deposit_event(Event::PaymentIntentFailed {
                intent_id,
                reason: DispatchError::Other("Payment intent failed by facilitator"),
            });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn verify_facilitator_signature(
            merchant: &T::AccountId,
            miner: &T::AccountId,
            amount: BalanceOf<T>,
            nonce: u64,
            replay_fingerprint: H256,
            signature_bytes: &BoundedVec<u8, T::MaxSignatureLen>,
        ) -> bool {
            if signature_bytes.len() < 32 {
                return false;
            }

            // Build the message: SCALE-encode all payment parameters + facilitator
            let facilitator = T::FacilitatorAccount::get();
            let mut message = Vec::new();
            merchant.encode_to(&mut message);
            miner.encode_to(&mut message);
            amount.encode_to(&mut message);
            nonce.encode_to(&mut message);
            replay_fingerprint.encode_to(&mut message);
            facilitator.encode_to(&mut message);

            // Expected signature = blake2_256(message)
            let expected = sp_io::hashing::blake2_256(&message);

            // Compare first 32 bytes of signature with expected hash
            signature_bytes[..32] == expected[..]
        }

        pub fn get_payment_intent(intent_id: u64) -> Option<PaymentIntent<T>> {
            PaymentIntents::<T>::get(intent_id)
        }

        pub fn get_settlement_receipt(intent_id: u64) -> Option<SettlementReceipt<T::AccountId, BalanceOf<T>>> {
            SettlementReceipts::<T>::get(intent_id)
        }

        pub fn is_nonce_used(account: &T::AccountId, nonce: u64) -> bool {
            NonceUsed::<T>::contains_key((account.clone(), nonce))
        }

        pub fn is_replay_fingerprint_used(fingerprint: H256) -> bool {
            ReplayFingerprintUsed::<T>::contains_key(fingerprint)
        }
    }
}

impl<T: pallet::Config> dbc_support::traits::AttestationSettler for pallet::Pallet<T> {
    type AccountId = T::AccountId;
    type Balance = pallet::BalanceOf<T>;

    fn settle_for_attestation(
        merchant: &Self::AccountId,
        miner: &Self::AccountId,
        amount: Self::Balance,
        _attestation_id: u64,
    ) -> Result<u64, &'static str> {
        use frame_support::traits::{BalanceStatus, ReservableCurrency};

        let intent_id = pallet::NextIntentId::<T>::get();
        let next_id = intent_id.checked_add(1).ok_or("Intent ID overflow")?;
        pallet::NextIntentId::<T>::put(next_id);

        T::Currency::reserve(merchant, amount)
            .map_err(|_| "Failed to reserve merchant funds")?;

        T::Currency::repatriate_reserved(merchant, miner, amount, BalanceStatus::Free)
            .map_err(|_| "Failed to transfer to miner")?;

        let now = frame_system::Pallet::<T>::block_number();
        let receipt = pallet::SettlementReceipt {
            intent_id,
            merchant: merchant.clone(),
            miner: miner.clone(),
            amount,
            settled_at: sp_runtime::traits::SaturatedConversion::saturated_into(now),
            tx_hash: sp_core::H256::from_low_u64_be(intent_id),
        };
        pallet::SettlementReceipts::<T>::insert(intent_id, receipt);

        pallet::Pallet::<T>::deposit_event(pallet::Event::PaymentIntentSettled {
            intent_id,
            merchant: merchant.clone(),
            miner: miner.clone(),
            amount,
        });

        Ok(intent_id)
    }
}
