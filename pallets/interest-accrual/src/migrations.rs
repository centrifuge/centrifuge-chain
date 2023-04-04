use frame_support::{
	pallet_prelude::{OptionQuery, ValueQuery},
	storage_alias,
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
	Blake2_128Concat,
};

use crate::*;

pub mod v1 {
	use super::*;
	#[storage_alias]
	pub type Rate<T: Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		<T as Config>::InterestRate,
		RateDetailsV1<<T as Config>::InterestRate>,
		OptionQuery,
	>;

	#[storage_alias]
	pub type RateCount<T: Config> = StorageValue<Pallet<T>, u32, ValueQuery>;
}

pub mod v2 {
	use super::*;

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config> OnRuntimeUpgrade for Migration<T> {
		fn on_runtime_upgrade() -> Weight {
			let version = StorageVersion::<T>::get();
			if version != Release::V1 {
				log::warn!("Skipping interest_accrual migration: Storage is at incorrect version");
				return T::DbWeight::get().reads(1);
			}
			let rates: Vec<_> = v1::Rate::<T>::drain()
				.map(|(interest_rate_per_sec, details)| {
					let RateDetailsV1 {
						accumulated_rate,
						reference_count,
					} = details;
					RateDetails {
						interest_rate_per_sec,
						accumulated_rate,
						reference_count,
					}
				})
				.collect();
			let count: u64 = rates
				.len()
				.try_into()
				.expect("WASM usize will always fit in a u64");

			Rates::<T>::set(
				rates
					.try_into()
					.expect("Input to this vector was already bounded"),
			);
			v1::RateCount::<T>::kill();
			StorageVersion::<T>::set(Release::V2);

			// Reads: storage version + each rate
			// Writes: each rate (storage killed), rates count (storage killed),
			// rates vector, storage version
			T::DbWeight::get().reads_writes(1 + count, 3 + count)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			let version = StorageVersion::<T>::get();
			let old_rates: Option<
				Vec<(
					<T as Config>::InterestRate,
					RateDetailsV1<<T as Config>::InterestRate>,
				)>,
			> = if version == Release::V1 {
				Some(v1::Rate::<T>::iter().collect())
			} else {
				None
			};
			Ok(old_rates.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
			let old_rates = Option::<
				Vec<(
					<T as Config>::InterestRate,
					RateDetailsV1<<T as Config>::InterestRate>,
				)>,
			>::decode(&mut state.as_ref())
			.map_err(|_| "Error decoding pre-upgrade state")?;

			for (rate_per_sec, old_rate) in old_rates.into_iter().flatten() {
				let rate_per_year = rate_per_sec
					.checked_sub(&One::one())
					.unwrap()
					.saturating_mul(T::InterestRate::saturating_from_integer(SECONDS_PER_YEAR));

				let new_rate = Pallet::<T>::get_rate(rate_per_year)
					.map_err(|_| "Expected rate not found in new state")?;
				if new_rate.accumulated_rate != old_rate.accumulated_rate {
					return Err("Accumulated rate was not correctly migrated");
				}

				if new_rate.reference_count != old_rate.reference_count {
					return Err("Reference count was not correctly migrated");
				}
			}

			Ok(())
		}
	}
}

pub mod centrifuge {
	use super::*;

	pub struct SetStorageVersionToV2<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config> OnRuntimeUpgrade for SetStorageVersionToV2<T> {
		fn on_runtime_upgrade() -> Weight {
			if StorageVersion::<T>::get() != Release::V2 {
				StorageVersion::<T>::set(Release::V2);
				T::DbWeight::get().reads_writes(1, 1)
			} else {
				T::DbWeight::get().reads(1)
			}
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{mock::*, test_utils::*};
	#[cfg(feature = "try-runtime")]
	#[test]
	fn migrate_to_v2() {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let rate = interest_rate_per_sec(
					<Runtime as Config>::InterestRate::saturating_from_rational(10, 100),
				)
				.unwrap();
				v1::Rate::<Runtime>::insert(
					&rate,
					RateDetailsV1 {
						accumulated_rate: rate.clone(),
						reference_count: 42,
					},
				);
				StorageVersion::<Runtime>::put(Release::V1);
				let state = v2::Migration::<Runtime>::pre_upgrade().unwrap();
				v2::Migration::<Runtime>::on_runtime_upgrade();
				v2::Migration::<Runtime>::post_upgrade(state).unwrap();
			})
	}
}
