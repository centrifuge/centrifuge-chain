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
	}
}

#[cfg(test)]
mod test {}
