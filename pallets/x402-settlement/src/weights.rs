#![allow(clippy::unnecessary_cast)]

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn submit_payment_intent() -> Weight;
    fn verify_settlement() -> Weight;
    fn finalize_settlement() -> Weight;
    fn fail_payment_intent() -> Weight;
}

impl WeightInfo for () {
    fn submit_payment_intent() -> Weight { Weight::from_parts(10_000, 0) }
    fn verify_settlement() -> Weight { Weight::from_parts(5_000, 0) }
    fn finalize_settlement() -> Weight { Weight::from_parts(15_000, 0) }
    fn fail_payment_intent() -> Weight { Weight::from_parts(5_000, 0) }
}
