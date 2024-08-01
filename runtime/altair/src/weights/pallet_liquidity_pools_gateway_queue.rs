use core::marker::PhantomData;

use frame_support::weights::Weight;

/// Defensive weights for LP gateway queue extrinsics.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_liquidity_pools_gateway_queue::WeightInfo for WeightInfo<T> {
	fn process_message() -> Weight {
		Weight::from_parts(50_000_000, 0)
	}

	fn process_failed_message() -> Weight {
		Weight::from_parts(50_000_000, 0)
	}
}
