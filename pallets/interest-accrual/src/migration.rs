use crate::*;
use frame_support::{
	pallet_prelude::OptionQuery, storage_alias, traits::Get, weights::Weight, Blake2_128Concat,
};
use weights::WeightInfo;

mod v0 {
	use super::*;
	#[storage_alias]
	pub type Rate<T: Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		<T as Config>::InterestRate,
		RateDetailsV0Of<T>,
		OptionQuery,
	>;
}

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

#[cfg(test)]
mod test {
	use super::*;
	use frame_support::traits::Hooks;
	use mock::*;

	/// returns the seconds in a given normal day
	fn seconds_per_day() -> Moment {
		3600 * 24
	}

	/// returns the seconds in a given normal year(365 days)
	/// https://docs.centrifuge.io/learn/interest-rate-methodology/
	fn seconds_per_year() -> Moment {
		seconds_per_day() * 365
	}

	/// calculates rate per second from the given nominal interest rate
	/// https://docs.centrifuge.io/learn/interest-rate-methodology/
	fn interest_rate_per_sec<Rate: FixedPointNumber>(rate_per_annum: Rate) -> Option<Rate> {
		rate_per_annum
			.checked_div(&Rate::saturating_from_integer(seconds_per_year() as u128))
			.and_then(|res| res.checked_add(&Rate::one()))
	}

	fn next_block_after(seconds: u64) {
		Timestamp::on_finalize(System::block_number());
		System::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
		Timestamp::on_initialize(System::block_number());
		Timestamp::set(Origin::none(), Timestamp::now() + seconds * SECONDS).unwrap();
	}

	#[test]
	fn migrate_v0_to_v1() {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let rate_info = RateDetailsV0 {
					accumulated_rate: One::one(),
					last_updated: START_DATE,
				};
				let rate_per_sec =
					interest_rate_per_sec(mock::Rate::saturating_from_rational(10, 100)).unwrap();
				v0::Rate::<Test>::insert(rate_per_sec, rate_info);
				StorageVersion::<Test>::set(Release::V0);
				next_block_after(seconds_per_day());
				let now = mock::InterestAccrual::now();
				let weight = v1::migrate::<Test>();
				let expected_weight = <Test as Config>::Weights::calculate_accumulated_rate(17)
					+ <Test as frame_system::Config>::DbWeight::get().reads_writes(2, 2);
				assert_eq!(weight, expected_weight);
				assert_eq!(LastUpdated::<Test>::get(), now);
				let rate_info = crate::Rate::<Test>::get(rate_per_sec).unwrap();
				assert_eq!(0, rate_info.reference_count);
				assert_eq!(1000274010136172548, rate_info.accumulated_rate.into_inner());
			})
	}
}
