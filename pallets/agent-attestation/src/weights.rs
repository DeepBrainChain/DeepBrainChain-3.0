#![allow(clippy::unnecessary_cast)]

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn register_node() -> Weight;
    fn heartbeat() -> Weight;
    fn submit_attestation() -> Weight;
    fn challenge_attestation() -> Weight;
    fn confirm_attestation() -> Weight;
    fn resolve_challenge() -> Weight;
}

impl WeightInfo for () {
    fn register_node() -> Weight { Weight::from_parts(10_000, 0) }
    fn heartbeat() -> Weight { Weight::from_parts(5_000, 0) }
    fn submit_attestation() -> Weight { Weight::from_parts(15_000, 0) }
    fn challenge_attestation() -> Weight { Weight::from_parts(10_000, 0) }
    fn confirm_attestation() -> Weight { Weight::from_parts(10_000, 0) }
    fn resolve_challenge() -> Weight { Weight::from_parts(20_000, 0) }
}
