// TODO: pending to regenerate

use core::marker::PhantomData;

use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_oracle_data_collection::WeightInfo for WeightInfo<T> {
	fn propose_update_feeders(_: u32) -> Weight {
		Weight::zero()
	}

	fn apply_update_feeders(_: u32) -> Weight {
		Weight::zero()
	}

	fn update_collection(_: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn set_collection_info() -> Weight {
		Weight::zero()
	}
}
