use frame_support::{weights::Weight};

/// Weight functions needed for Fees.
pub trait WeightInfo {
    fn set_fee() -> Weight;
}

impl WeightInfo for () {
    fn set_fee() -> Weight {
        195_000_000
    }
}
