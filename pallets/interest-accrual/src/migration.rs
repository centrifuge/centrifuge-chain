use crate::*;
use frame_support::{traits::Get, weights::Weight};
use weights::WeightInfo;

pub mod v1 {
	use super::*;
	pub fn migrate<T: Config>() -> Weight {
		let mut weight = T::DbWeight::get().reads_writes(1, 1);
		let now = Pallet::<T>::now();
		LastUpdated::<T>::set(now);
		Rate::<T>::translate(|per_sec, rate: RateDetailsV0Of<T>| {
			let delta = now - rate.last_updated;
			let bits = Moment::BITS - delta.leading_zeros();
			weight += T::DbWeight::get().reads_writes(1, 1);
			weight += T::Weights::calculate_accumulated_rate(bits);
			Pallet::<T>::calculate_accumulated_rate(
				per_sec,
				rate.accumulated_rate,
				rate.last_updated,
				now,
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
