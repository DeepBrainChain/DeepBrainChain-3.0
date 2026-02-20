#![allow(clippy::unnecessary_cast)]

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn submit_proof() -> Weight;
    fn verify_task() -> Weight;
    fn claim_reward() -> Weight;
}

impl WeightInfo for () {
    fn submit_proof() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn verify_task() -> Weight {
        Weight::from_parts(20_000, 0)
    }

    fn claim_reward() -> Weight {
        Weight::from_parts(15_000, 0)
    }
}
