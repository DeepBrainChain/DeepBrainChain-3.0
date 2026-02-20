#![allow(clippy::unnecessary_cast)]

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn create_task_definition() -> Weight;
    fn update_task_definition() -> Weight;
    fn create_task_order() -> Weight;
    fn mark_order_completed() -> Weight;
    fn settle_task_order() -> Weight;
}

impl WeightInfo for () {
    fn create_task_definition() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn update_task_definition() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn create_task_order() -> Weight {
        Weight::from_parts(50_000, 0)
    }

    fn mark_order_completed() -> Weight {
        Weight::from_parts(20_000, 0)
    }

    fn settle_task_order() -> Weight {
        Weight::from_parts(50_000, 0)
    }
}
