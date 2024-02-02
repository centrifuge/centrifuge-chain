// TODO: pending to regenerate

use core::marker::PhantomData;

use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_oracle_feed::WeightInfo for WeightInfo<T> {
	fn feed_with_fee() -> Weight {
		Weight::zero()
	}

	fn feed_without_fee() -> Weight {
		Weight::zero()
	}
}
