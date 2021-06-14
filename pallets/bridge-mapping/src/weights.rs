use frame_support::weights::{constants::RocksDbWeight, Weight};

/// Weight functions needed for bridge mapping.
pub trait WeightInfo {
	fn set() -> Weight;
	fn remove() -> Weight;
}

impl WeightInfo for () {
	fn set() -> Weight {
		(100_000 as Weight).saturating_add(RocksDbWeight::get().reads_writes(0, 2))
	}

	fn remove() -> Weight {
		(100_000 as Weight).saturating_add(RocksDbWeight::get().reads_writes(1, 2))
	}
}
