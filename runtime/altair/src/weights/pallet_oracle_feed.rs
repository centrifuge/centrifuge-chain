// TODO: pending to regenerate

use core::marker::PhantomData;

use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_oracle_feed::WeightInfo for WeightInfo<T> {
	fn feed_first() -> Weight {
		Weight::zero()
	}

	fn feed_again() -> Weight {
		Weight::zero()
	}
}
