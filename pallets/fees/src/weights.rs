use frame_support::{weights::{Weight, constants::RocksDbWeight}};

/// Weight functions needed for Fees.
pub trait WeightInfo {
    fn set_fee() -> Weight;
}

impl WeightInfo for () {
    fn set_fee() -> Weight {
        (100_000 as Weight).saturating_add(
            RocksDbWeight::get().reads_writes(0, 1)
        )
    }
}
