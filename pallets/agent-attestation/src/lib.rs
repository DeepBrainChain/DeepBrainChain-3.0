#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
pub use pallet::*;
pub mod weights;

use sp_runtime::traits::Saturating;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo;
    use dbc_support::traits::AttestationSettler;
    use frame_support::traits::StorageVersion;
    use frame_support::{
        dispatch::DispatchResult,
        pallet_prelude::*,
        traits::EnsureOrigin,
        traits::{Currency, ReservableCurrency},
        BoundedVec,
    };
    use frame_system::pallet_prelude::*;
    use sp_core::H256;
    use sp_runtime::traits::Saturating;
    use alloc::vec::Vec;

    type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Status of an attestation
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum AttestationStatus {
        /// Attestation submitted, within challenge window
        Pending,
        /// Challenge window passed, attestation confirmed
        Confirmed,
        /// Attestation was challenged and found invalid
        Slashed,
        /// Attestation was challenged but defender won
        Defended,
    }

    /// A task result attestation submitted by a miner/agent
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct Attestation<T: Config> {
        pub id: u64,
        pub attester: T::AccountId,
        pub task_id: u64,
        pub result_hash: H256,
        pub model_id: BoundedVec<u8, T::MaxModelIdLen>,
        pub input_tokens: u64,
        pub output_tokens: u64,
        pub deposit: BalanceOf<T>,
        pub status: AttestationStatus,
        pub submitted_at: BlockNumberFor<T>,
        pub challenge_end: BlockNumberFor<T>,
        pub challenger: Option<T::AccountId>,
    }

    /// Hardware info registered by a node
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct NodeRegistration<T: Config> {
        pub owner: T::AccountId,
        pub gpu_uuid: BoundedVec<u8, T::MaxGpuUuidLen>,
        pub tflops: u32,
        pub registered_at: BlockNumberFor<T>,
        pub last_heartbeat: BlockNumberFor<T>,
        pub is_active: bool,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct AgentCapability<T: Config> {
        pub owner: T::AccountId,
        pub model_ids: BoundedVec<BoundedVec<u8, T::MaxModelIdLen>, T::MaxModelsPerAgent>,
        pub max_concurrent: u32,
        pub price_per_token: BalanceOf<T>,
        pub region: BoundedVec<u8, ConstU32<16>>,
        pub updated_at: BlockNumberFor<T>,
    }

    /// Benchmark claim status
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum BenchmarkClaimStatus {
        Active,
        Challenged,
        Slashed,
        Defended,
    }

    /// A benchmark score claim by a miner for a specific model
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct BenchmarkClaim<T: Config> {
        pub claimer: T::AccountId,
        pub model_id: BoundedVec<u8, T::MaxModelIdLen>,
        pub score: u32,
        pub deposit: BalanceOf<T>,
        pub submitted_at: BlockNumberFor<T>,
        pub status: BenchmarkClaimStatus,
        pub challenger: Option<T::AccountId>,
        pub challenge_deposit: Option<BalanceOf<T>>,
        pub challenged_at: Option<BlockNumberFor<T>>,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Currency: ReservableCurrency<Self::AccountId>;

        /// Minimum deposit for submitting an attestation
        #[pallet::constant]
        type AttestationDeposit: Get<BalanceOf<Self>>;

        /// Number of blocks for the challenge window
        #[pallet::constant]
        type ChallengeWindow: Get<BlockNumberFor<Self>>;

        /// Slash percentage (0-100) applied to invalid attestations
        #[pallet::constant]
        type SlashPercent: Get<u32>;

        /// Heartbeat interval in blocks
        #[pallet::constant]
        type HeartbeatInterval: Get<BlockNumberFor<Self>>;

        /// Max length of model ID
        #[pallet::constant]
        type MaxModelIdLen: Get<u32>;

        /// Max length of GPU UUID
        #[pallet::constant]
        type MaxGpuUuidLen: Get<u32>;

        #[pallet::constant]
        type MaxModelsPerAgent: Get<u32>;

        /// Minimum deposit for submitting a benchmark claim
        #[pallet::constant]
        type BenchmarkDeposit: Get<BalanceOf<Self>>;

        /// Minimum deposit for challenging a benchmark claim
        #[pallet::constant]
        type BenchmarkChallengeDeposit: Get<BalanceOf<Self>>;

        /// Maximum benchmark claims per miner
        #[pallet::constant]
        type MaxBenchmarkClaims: Get<u32>;

        type WeightInfo: WeightInfo;

        /// Handler to trigger settlement after attestation is confirmed
        type OnAttestationConfirmed: dbc_support::traits::AttestationSettler<
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
        >;

        /// Origin that can confirm attestations
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    // ---- Storage ----

    #[pallet::storage]
    #[pallet::getter(fn next_attestation_id)]
    pub type NextAttestationId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn attestation_of)]
    pub type Attestations<T: Config> = StorageMap<_, Blake2_128Concat, u64, Attestation<T>>;

    #[pallet::storage]
    #[pallet::getter(fn node_of)]
    pub type Nodes<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, NodeRegistration<T>>;

    #[pallet::storage]
    #[pallet::getter(fn attester_count)]
    pub type AttesterTaskCount<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, u64, u32, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn agent_capability)]
    pub type AgentCapabilities<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, AgentCapability<T>>;

    #[pallet::storage]
    #[pallet::getter(fn model_providers)]
    pub type ModelProviders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, T::MaxModelIdLen>,
        Blake2_128Concat,
        T::AccountId,
        bool,
        ValueQuery,
    >;

    // ---- Benchmark Storage ----

    #[pallet::storage]
    #[pallet::getter(fn next_benchmark_claim_id)]
    pub type NextBenchmarkClaimId<T> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn benchmark_claims)]
    pub type BenchmarkClaims<T: Config> = StorageMap<_, Blake2_128Concat, u64, BenchmarkClaim<T>>;

    /// (AccountId, ModelId) -> BenchmarkClaimId
    #[pallet::storage]
    #[pallet::getter(fn miner_benchmark)]
    pub type MinerBenchmark<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        BoundedVec<u8, T::MaxModelIdLen>,
        u64,
        OptionQuery,
    >;

    /// AccountId -> active benchmark claim count
    #[pallet::storage]
    #[pallet::getter(fn miner_claim_count)]
    pub type MinerClaimCount<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    // ---- Events ----

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        NodeRegistered {
            who: T::AccountId,
            tflops: u32,
        },
        HeartbeatReceived {
            who: T::AccountId,
            block: BlockNumberFor<T>,
        },
        AttestationSubmitted {
            id: u64,
            attester: T::AccountId,
            task_id: u64,
            result_hash: H256,
        },
        AttestationChallenged {
            id: u64,
            challenger: T::AccountId,
        },
        AttestationConfirmed {
            id: u64,
        },
        AgentCapabilityUpdated {
            who: T::AccountId,
            model_count: u32,
        },
        AttestationSlashed {
            id: u64,
            attester: T::AccountId,
            slash_amount: BalanceOf<T>,
        },
        AttestationDefended {
            id: u64,
        },
        BenchmarkClaimSubmitted {
            claim_id: u64,
            claimer: T::AccountId,
            score: u32,
        },
        BenchmarkChallenged {
            claim_id: u64,
            challenger: T::AccountId,
        },
        BenchmarkClaimSlashed {
            claim_id: u64,
            claimer: T::AccountId,
            slash_amount: BalanceOf<T>,
        },
        BenchmarkChallengerSlashed {
            claim_id: u64,
            challenger: T::AccountId,
            slash_amount: BalanceOf<T>,
        },
        BenchmarkClaimUpdated {
            claim_id: u64,
            new_score: u32,
        },
    }

    // ---- Errors ----

    #[pallet::error]
    pub enum Error<T> {
        NodeAlreadyRegistered,
        NodeNotRegistered,
        HeartbeatTooEarly,
        AttestationNotFound,
        AlreadyChallenged,
        ChallengeWindowExpired,
        ChallengeWindowNotExpired,
        NotAttester,
        InvalidStatus,
        InsufficientDeposit,
        ArithmeticOverflow,
        InvalidModelId,
        TooManyModels,
        InvalidRegion,
        DuplicateAttestation,
        BenchmarkClaimNotFound,
        BenchmarkAlreadyChallenged,
        BenchmarkNotChallenged,
        BenchmarkAlreadyResolved,
        NotClaimOwner,
        TooManyBenchmarkClaims,
        CannotChallengeSelf,
    }

    // ---- Hooks ----

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub _phantom: core::marker::PhantomData<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { _phantom: Default::default() }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {}
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // ---- Extrinsics ----

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a node with GPU hardware info
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_node())]
        pub fn register_node(
            origin: OriginFor<T>,
            gpu_uuid: Vec<u8>,
            tflops: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!Nodes::<T>::contains_key(&who), Error::<T>::NodeAlreadyRegistered);

            let gpu_uuid_bounded: BoundedVec<u8, T::MaxGpuUuidLen> =
                gpu_uuid.try_into().map_err(|_| Error::<T>::ArithmeticOverflow)?;

            let now = <frame_system::Pallet<T>>::block_number();

            Nodes::<T>::insert(
                &who,
                NodeRegistration {
                    owner: who.clone(),
                    gpu_uuid: gpu_uuid_bounded,
                    tflops,
                    registered_at: now,
                    last_heartbeat: now,
                    is_active: true,
                },
            );

            Self::deposit_event(Event::NodeRegistered { who, tflops });
            Ok(())
        }

        /// Send heartbeat to prove liveness (every HeartbeatInterval blocks)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::heartbeat())]
        pub fn heartbeat(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Nodes::<T>::try_mutate(&who, |maybe_node| -> DispatchResult {
                let node = maybe_node.as_mut().ok_or(Error::<T>::NodeNotRegistered)?;
                let now = <frame_system::Pallet<T>>::block_number();

                ensure!(
                    now >= node.last_heartbeat.saturating_add(T::HeartbeatInterval::get()),
                    Error::<T>::HeartbeatTooEarly
                );

                node.last_heartbeat = now;
                node.is_active = true;

                Self::deposit_event(Event::HeartbeatReceived { who: who.clone(), block: now });
                Ok(())
            })
        }

        /// Submit attestation for a completed task result
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::submit_attestation())]
        pub fn submit_attestation(
            origin: OriginFor<T>,
            task_id: u64,
            result_hash: H256,
            model_id: Vec<u8>,
            input_tokens: u64,
            output_tokens: u64,
        ) -> DispatchResult {
            let attester = ensure_signed(origin)?;

            // Must be a registered node
            ensure!(Nodes::<T>::contains_key(&attester), Error::<T>::NodeNotRegistered);

            // Prevent duplicate attestations for the same (attester, task_id)
            ensure!(
                AttesterTaskCount::<T>::get(&attester, task_id) == 0,
                Error::<T>::DuplicateAttestation
            );

            let deposit = T::AttestationDeposit::get();
            T::Currency::reserve(&attester, deposit)
                .map_err(|_| Error::<T>::InsufficientDeposit)?;

            let model_id_bounded: BoundedVec<u8, T::MaxModelIdLen> =
                model_id.try_into().map_err(|_| Error::<T>::ArithmeticOverflow)?;

            let id = NextAttestationId::<T>::get();
            let next_id = id.checked_add(1).ok_or(Error::<T>::ArithmeticOverflow)?;
            NextAttestationId::<T>::put(next_id);

            let now = <frame_system::Pallet<T>>::block_number();
            let challenge_end = now.saturating_add(T::ChallengeWindow::get());

            Attestations::<T>::insert(
                id,
                Attestation {
                    id,
                    attester: attester.clone(),
                    task_id,
                    result_hash,
                    model_id: model_id_bounded,
                    input_tokens,
                    output_tokens,
                    deposit,
                    status: AttestationStatus::Pending,
                    submitted_at: now,
                    challenge_end,
                    challenger: None,
                },
            );

            // Track per-attester task count
            AttesterTaskCount::<T>::mutate(&attester, task_id, |count| {
                *count = count.saturating_add(1);
            });

            Self::deposit_event(Event::AttestationSubmitted { id, attester, task_id, result_hash });
            Ok(())
        }

        /// Challenge an attestation within the challenge window
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::challenge_attestation())]
        pub fn challenge_attestation(origin: OriginFor<T>, attestation_id: u64) -> DispatchResult {
            let challenger = ensure_signed(origin)?;

            Attestations::<T>::try_mutate(attestation_id, |maybe_att| -> DispatchResult {
                let att = maybe_att.as_mut().ok_or(Error::<T>::AttestationNotFound)?;

                ensure!(
                    matches!(att.status, AttestationStatus::Pending),
                    Error::<T>::InvalidStatus
                );
                ensure!(att.challenger.is_none(), Error::<T>::AlreadyChallenged);

                let now = <frame_system::Pallet<T>>::block_number();
                ensure!(now <= att.challenge_end, Error::<T>::ChallengeWindowExpired);

                att.challenger = Some(challenger.clone());

                Self::deposit_event(Event::AttestationChallenged {
                    id: attestation_id,
                    challenger,
                });
                Ok(())
            })
        }

        /// Confirm an attestation after challenge window expires with no challenge
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::confirm_attestation())]
        pub fn confirm_attestation(origin: OriginFor<T>, attestation_id: u64) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            Attestations::<T>::try_mutate(attestation_id, |maybe_att| -> DispatchResult {
                let att = maybe_att.as_mut().ok_or(Error::<T>::AttestationNotFound)?;

                ensure!(
                    matches!(att.status, AttestationStatus::Pending),
                    Error::<T>::InvalidStatus
                );
                ensure!(att.challenger.is_none(), Error::<T>::AlreadyChallenged);

                let now = <frame_system::Pallet<T>>::block_number();
                ensure!(now > att.challenge_end, Error::<T>::ChallengeWindowNotExpired);

                att.status = AttestationStatus::Confirmed;

                // Unreserve deposit
                T::Currency::unreserve(&att.attester, att.deposit);

                Self::deposit_event(Event::AttestationConfirmed { id: attestation_id });

                // Trigger settlement (ignore errors to not block confirmation)
                // In a real implementation, merchant and amount would be tracked in the attestation
                // For now, we use placeholder values
                let _ = T::OnAttestationConfirmed::settle_for_attestation(
                    &att.attester, // Using attester as merchant placeholder
                    &att.attester, // miner
                    att.deposit,   // amount placeholder
                    attestation_id,
                );

                Ok(())
            })
        }

        /// Resolve a challenged attestation (simplified: root/committee decides)
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::resolve_challenge())]
        pub fn resolve_challenge(
            origin: OriginFor<T>,
            attestation_id: u64,
            attester_is_guilty: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Attestations::<T>::try_mutate(attestation_id, |maybe_att| -> DispatchResult {
                let att = maybe_att.as_mut().ok_or(Error::<T>::AttestationNotFound)?;

                ensure!(
                    matches!(att.status, AttestationStatus::Pending),
                    Error::<T>::InvalidStatus
                );
                ensure!(att.challenger.is_some(), Error::<T>::InvalidStatus);

                if attester_is_guilty {
                    // Slash deposit
                    let slash_percent = T::SlashPercent::get();
                    let slash_amount =
                        sp_runtime::Perbill::from_percent(slash_percent) * att.deposit;

                    // Slash from reserved
                    let _imbalance = T::Currency::slash_reserved(&att.attester, slash_amount);

                    // Unreserve remainder
                    let remainder = att.deposit.saturating_sub(slash_amount);
                    T::Currency::unreserve(&att.attester, remainder);

                    att.status = AttestationStatus::Slashed;

                    Self::deposit_event(Event::AttestationSlashed {
                        id: attestation_id,
                        attester: att.attester.clone(),
                        slash_amount,
                    });
                } else {
                    // Attester wins, unreserve deposit
                    T::Currency::unreserve(&att.attester, att.deposit);
                    att.status = AttestationStatus::Defended;

                    Self::deposit_event(Event::AttestationDefended { id: attestation_id });
                }

                Ok(())
            })
        }

        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::update_capability())]
        pub fn update_capability(
            origin: OriginFor<T>,
            model_ids: Vec<Vec<u8>>,
            max_concurrent: u32,
            price_per_token: BalanceOf<T>,
            region: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(Nodes::<T>::contains_key(&who), Error::<T>::NodeNotRegistered);

            let bounded_models: BoundedVec<BoundedVec<u8, T::MaxModelIdLen>, T::MaxModelsPerAgent> =
                model_ids
                    .into_iter()
                    .map(|m| m.try_into().map_err(|_| Error::<T>::InvalidModelId))
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| Error::<T>::TooManyModels)?;

            let bounded_region: BoundedVec<u8, ConstU32<16>> =
                region.try_into().map_err(|_| Error::<T>::InvalidRegion)?;

            let old_cap = AgentCapabilities::<T>::get(&who);
            if let Some(ref old) = old_cap {
                for model in old.model_ids.iter() {
                    ModelProviders::<T>::remove(model, &who);
                }
            }

            let model_count = bounded_models.len() as u32;
            for model in bounded_models.iter() {
                ModelProviders::<T>::insert(model, &who, true);
            }

            AgentCapabilities::<T>::insert(
                &who,
                AgentCapability {
                    owner: who.clone(),
                    model_ids: bounded_models,
                    max_concurrent,
                    price_per_token,
                    region: bounded_region,
                    updated_at: <frame_system::Pallet<T>>::block_number(),
                },
            );

            Self::deposit_event(Event::AgentCapabilityUpdated { who, model_count });
            Ok(())
        }

        /// Submit a benchmark claim for a model (miner self-reports score)
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::submit_benchmark_claim())]
        pub fn submit_benchmark_claim(
            origin: OriginFor<T>,
            model_id: Vec<u8>,
            score: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(Nodes::<T>::contains_key(&who), Error::<T>::NodeNotRegistered);

            let model_id_bounded: BoundedVec<u8, T::MaxModelIdLen> =
                model_id.try_into().map_err(|_| Error::<T>::InvalidModelId)?;

            // Check claim limit
            let current_count = MinerClaimCount::<T>::get(&who);

            // Check if replacing an existing claim for same model
            let replacing = MinerBenchmark::<T>::get(&who, &model_id_bounded).is_some();
            if !replacing {
                ensure!(
                    current_count < T::MaxBenchmarkClaims::get(),
                    Error::<T>::TooManyBenchmarkClaims
                );
            }

            // If replacing old claim, refund old deposit
            if let Some(old_claim_id) = MinerBenchmark::<T>::get(&who, &model_id_bounded) {
                if let Some(old_claim) = BenchmarkClaims::<T>::take(old_claim_id) {
                    T::Currency::unreserve(&who, old_claim.deposit);
                    // Don't decrement count since replacing
                }
            }

            // Reserve new deposit
            let deposit = T::BenchmarkDeposit::get();
            T::Currency::reserve(&who, deposit).map_err(|_| Error::<T>::InsufficientDeposit)?;

            // Create new claim
            let claim_id = NextBenchmarkClaimId::<T>::get();
            let next_id = claim_id.checked_add(1).ok_or(Error::<T>::ArithmeticOverflow)?;
            NextBenchmarkClaimId::<T>::put(next_id);

            let now = <frame_system::Pallet<T>>::block_number();
            BenchmarkClaims::<T>::insert(
                claim_id,
                BenchmarkClaim {
                    claimer: who.clone(),
                    model_id: model_id_bounded.clone(),
                    score,
                    deposit,
                    submitted_at: now,
                    status: BenchmarkClaimStatus::Active,
                    challenger: None,
                    challenge_deposit: None,
                    challenged_at: None,
                },
            );

            MinerBenchmark::<T>::insert(&who, &model_id_bounded, claim_id);

            // Increment count only if new model claim
            if !replacing {
                MinerClaimCount::<T>::mutate(&who, |count| {
                    *count = count.saturating_add(1);
                });
            }

            Self::deposit_event(Event::BenchmarkClaimSubmitted { claim_id, claimer: who, score });
            Ok(())
        }

        /// Challenge a benchmark claim with counter-deposit
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::challenge_benchmark())]
        pub fn challenge_benchmark(origin: OriginFor<T>, claim_id: u64) -> DispatchResult {
            let challenger = ensure_signed(origin)?;

            BenchmarkClaims::<T>::try_mutate(claim_id, |maybe_claim| -> DispatchResult {
                let claim = maybe_claim.as_mut().ok_or(Error::<T>::BenchmarkClaimNotFound)?;

                ensure!(
                    matches!(claim.status, BenchmarkClaimStatus::Active),
                    Error::<T>::BenchmarkAlreadyResolved
                );
                ensure!(claim.challenger.is_none(), Error::<T>::BenchmarkAlreadyChallenged);
                ensure!(claim.claimer != challenger, Error::<T>::CannotChallengeSelf);

                let challenge_deposit = T::BenchmarkChallengeDeposit::get();
                T::Currency::reserve(&challenger, challenge_deposit)
                    .map_err(|_| Error::<T>::InsufficientDeposit)?;

                claim.status = BenchmarkClaimStatus::Challenged;
                claim.challenger = Some(challenger.clone());
                claim.challenge_deposit = Some(challenge_deposit);
                claim.challenged_at = Some(<frame_system::Pallet<T>>::block_number());

                Self::deposit_event(Event::BenchmarkChallenged { claim_id, challenger });
                Ok(())
            })
        }

        /// Resolve benchmark challenge (admin decides)
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::resolve_benchmark())]
        pub fn resolve_benchmark(
            origin: OriginFor<T>,
            claim_id: u64,
            claim_is_valid: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;

            BenchmarkClaims::<T>::try_mutate_exists(claim_id, |maybe_claim| -> DispatchResult {
                let claim = maybe_claim.as_mut().ok_or(Error::<T>::BenchmarkClaimNotFound)?;

                ensure!(
                    matches!(claim.status, BenchmarkClaimStatus::Challenged),
                    Error::<T>::BenchmarkNotChallenged
                );

                let challenger = claim
                    .challenger
                    .clone()
                    .ok_or(Error::<T>::BenchmarkNotChallenged)?;
                let challenge_deposit =
                    claim.challenge_deposit.ok_or(Error::<T>::BenchmarkNotChallenged)?;

                if claim_is_valid {
                    // Challenger loses deposit, claimer gets defense success
                    let slash_percent = T::SlashPercent::get();
                    let slash_amount =
                        sp_runtime::Perbill::from_percent(slash_percent) * challenge_deposit;

                    let _imbalance = T::Currency::slash_reserved(&challenger, slash_amount);
                    let remainder = challenge_deposit.saturating_sub(slash_amount);
                    T::Currency::unreserve(&challenger, remainder);

                    claim.status = BenchmarkClaimStatus::Defended;

                    Self::deposit_event(Event::BenchmarkChallengerSlashed {
                        claim_id,
                        challenger,
                        slash_amount,
                    });
                } else {
                    // Claim invalid, claimer loses benchmark deposit
                    let slash_percent = T::SlashPercent::get();
                    let slash_amount = sp_runtime::Perbill::from_percent(slash_percent) * claim.deposit;

                    let _imbalance = T::Currency::slash_reserved(&claim.claimer, slash_amount);
                    let remainder = claim.deposit.saturating_sub(slash_amount);
                    T::Currency::unreserve(&claim.claimer, remainder);

                    // Challenger gets their deposit back
                    T::Currency::unreserve(&challenger, challenge_deposit);

                    // Remove index mapping and decrement claim count for claimer
                    MinerBenchmark::<T>::remove(&claim.claimer, &claim.model_id);
                    MinerClaimCount::<T>::mutate(&claim.claimer, |count| {
                        *count = count.saturating_sub(1);
                    });

                    claim.status = BenchmarkClaimStatus::Slashed;

                    Self::deposit_event(Event::BenchmarkClaimSlashed {
                        claim_id,
                        claimer: claim.claimer.clone(),
                        slash_amount,
                    });
                }

                Ok(())
            })
        }

        /// Update benchmark score for existing claim
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::update_benchmark_score())]
        pub fn update_benchmark_score(
            origin: OriginFor<T>,
            model_id: Vec<u8>,
            new_score: u32,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let model_id_bounded: BoundedVec<u8, T::MaxModelIdLen> =
                model_id.try_into().map_err(|_| Error::<T>::InvalidModelId)?;

            let claim_id =
                MinerBenchmark::<T>::get(&who, &model_id_bounded).ok_or(Error::<T>::BenchmarkClaimNotFound)?;

            BenchmarkClaims::<T>::try_mutate(claim_id, |maybe_claim| -> DispatchResult {
                let claim = maybe_claim.as_mut().ok_or(Error::<T>::BenchmarkClaimNotFound)?;

                ensure!(claim.claimer == who, Error::<T>::NotClaimOwner);
                ensure!(
                    matches!(claim.status, BenchmarkClaimStatus::Active),
                    Error::<T>::BenchmarkAlreadyResolved
                );

                claim.score = new_score;

                Self::deposit_event(Event::BenchmarkClaimUpdated {
                    claim_id,
                    new_score,
                });
                Ok(())
            })
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn get_providers_for_model(
            model_id: &BoundedVec<u8, T::MaxModelIdLen>,
        ) -> Vec<T::AccountId> {
            ModelProviders::<T>::iter_prefix(model_id)
                .filter(|(account, _)| Nodes::<T>::get(account).map_or(false, |n| n.is_active))
                .map(|(account, _)| account)
                .collect()
        }
    }
}

// ============================================================
// Cross-Pallet Integration: TaskCompletionHandler Implementation
// ============================================================

use frame_support::traits::Get;

impl<T: Config> dbc_support::traits::TaskCompletionHandler for Pallet<T> {
    type AccountId = T::AccountId;

    fn on_task_completed(
        attester: &Self::AccountId,
        task_id: u64,
        result_hash: sp_core::H256,
        model_id: &[u8],
        input_tokens: u64,
        output_tokens: u64,
    ) -> Result<u64, &'static str> {
        use frame_support::traits::ReservableCurrency;

        // Get the next attestation ID
        let attestation_id = pallet::NextAttestationId::<T>::get();
        let next_id = attestation_id.checked_add(1).ok_or("Attestation ID overflow")?;
        pallet::NextAttestationId::<T>::put(next_id);

        // Convert model_id to BoundedVec
        let model_id_bounded = model_id.to_vec().try_into().map_err(|_| "Model ID too long")?;

        let now = frame_system::Pallet::<T>::block_number();
        let challenge_end = now.saturating_add(T::ChallengeWindow::get());

        let deposit = T::AttestationDeposit::get();

        // Reserve deposit from attester
        T::Currency::reserve(attester, deposit).map_err(|_| "Failed to reserve deposit")?;

        // Create the attestation
        let attestation = pallet::Attestation {
            id: attestation_id,
            attester: attester.clone(),
            task_id,
            result_hash,
            model_id: model_id_bounded,
            input_tokens,
            output_tokens,
            deposit,
            status: pallet::AttestationStatus::Pending,
            submitted_at: now,
            challenge_end,
            challenger: None,
        };

        pallet::Attestations::<T>::insert(attestation_id, attestation);

        // Increment task count for attester
        pallet::AttesterTaskCount::<T>::mutate(attester, task_id, |count| {
            *count = count.saturating_add(1);
        });

        // Emit event
        Pallet::<T>::deposit_event(pallet::Event::AttestationSubmitted {
            id: attestation_id,
            attester: attester.clone(),
            task_id,
            result_hash,
        });

        Ok(attestation_id)
    }
}

// ============================================================
// Cross-Pallet Integration: BenchmarkScoreProvider Implementation
// ============================================================

impl<T: Config> dbc_support::traits::BenchmarkScoreProvider for Pallet<T> {
    type AccountId = T::AccountId;

    fn get_benchmark_score(miner: &Self::AccountId, model_id: &[u8]) -> Option<u32> {
        let model_id_bounded: frame_support::BoundedVec<u8, T::MaxModelIdLen> = model_id.to_vec().try_into().ok()?;
        let claim_id = pallet::MinerBenchmark::<T>::get(miner, &model_id_bounded)?;
        let claim = pallet::BenchmarkClaims::<T>::get(claim_id)?;
        if matches!(claim.status, pallet::BenchmarkClaimStatus::Active) {
            Some(claim.score)
        } else {
            None
        }
    }

    fn has_slashed_claims(miner: &Self::AccountId) -> bool {
        // Iterate all claims for this miner to check for slashed status
        for (_, claim_id) in pallet::MinerBenchmark::<T>::iter_prefix(miner) {
            if let Some(claim) = pallet::BenchmarkClaims::<T>::get(claim_id) {
                if matches!(claim.status, pallet::BenchmarkClaimStatus::Slashed) {
                    return true;
                }
            }
        }
        false
    }
}
