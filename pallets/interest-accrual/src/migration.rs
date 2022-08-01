use crate::*;
use frame_support::{traits::Get, weights::Weight};

pub mod v1 {
	use super::*;
	pub fn migrate<T: Config>() -> Weight {
		let mut weight = T::DbWeight::get().reads_writes(1, 1);
		let now = Pallet::<T>::now();
		LastUpdated::<T>::set(now);
		Rate::<T>::translate(|per_sec, rate: RateDetailsV0Of<T>| {
			let delta = now - rate.last_updated;
			let _bits = Moment::BITS - delta.leading_zeros();
			weight += T::DbWeight::get().reads_writes(1, 1);
			// weight += T::Weight::calculate_accumulated_rate(_bits);
			Pallet::<T>::calculate_accumulated_rate(
				per_sec,
				rate.accumulated_rate,
				rate.last_updated,
			)
			.ok()
			.map(|accumulated_rate| RateDetailsOf::<T> {
				accumulated_rate,
				reference_count: 0,
			})
		});
		weight
	}
}
