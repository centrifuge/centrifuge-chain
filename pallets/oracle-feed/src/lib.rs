// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Oracle pallet to feed values.
//! Feeding is permissionless given an initial fee for each key.
//!
//!
//! //! ### Assumptions
//!
//! This pallet neither aggregates nor validates anything. It just stores values
//! by account as they come. It's expected that another pallet reads the storage
//! of this pallet and provides aggregation and validation methods to the
//! values.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{fees::PayFee, ValueProvider};
	use frame_support::{
		pallet_prelude::*,
		traits::{OriginTrait, Time},
	};
	use frame_system::pallet_prelude::*;

	use crate::weights::WeightInfo;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	type MomentOf<T> = <<T as Config>::Time as Time>::Moment;
	type Feeder<T> = <<T as frame_system::Config>::RuntimeOrigin as OriginTrait>::PalletsOrigin;

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identify an oracle value
		type OracleKey: Parameter + Member + Copy + MaxEncodedLen;

		/// Represent an oracle value
		type OracleValue: Parameter + Member + Copy + MaxEncodedLen + Default;

		/// A way to obtain the current time
		type Time: Time;

		/// Fee for the first time a feeder feeds a value
		type FirstValuePayFee: PayFee<<Self::RuntimeOrigin as OriginTrait>::AccountId>;

		/// The weight information for this pallet extrinsics.
		type WeightInfo: WeightInfo;

		/// Ensure the feeder origin
		type FeederOrigin: EnsureOrigin<Self::RuntimeOrigin>;
	}

	/// Store all oracle values indexed by feeder
	#[pallet::storage]
	pub(crate) type FedValues<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		Feeder<T>,
		Blake2_128Concat,
		T::OracleKey,
		(T::OracleValue, MomentOf<T>),
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Fed {
			feeder: Feeder<T>,
			key: T::OracleKey,
			value: T::OracleValue,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Permissionles call to feed an oracle key from a source with value.
		/// The first time a value is set for a key, an extra fee is required
		/// for the feeder.
		#[pallet::weight(T::WeightInfo::feed_with_fee())]
		#[pallet::call_index(0)]
		pub fn feed(
			origin: OriginFor<T>,
			key: T::OracleKey,
			value: T::OracleValue,
		) -> DispatchResultWithPostInfo {
			let _ = T::FeederOrigin::ensure_origin(origin.clone())?;

			let feeder = origin.clone().into_caller();
			let signed_account = origin.as_signed();

			FedValues::<T>::mutate(&feeder, key, |prev_value| {
				let new_weight = match (&prev_value, signed_account) {
					(None, Some(account_id)) => {
						T::FirstValuePayFee::pay(&account_id)?;

						// The weight used is the predefined one.
						None
					}
					_ => {
						// The weight used is less than the predefined,
						// because we do not need to pay an extra fee
						Some(T::WeightInfo::feed_without_fee())
					}
				};

				*prev_value = Some((value, T::Time::now()));

				Self::deposit_event(Event::<T>::Fed {
					feeder: feeder.clone(),
					key,
					value,
				});

				Ok(new_weight.into())
			})
		}
	}

	impl<T: Config> ValueProvider<T::RuntimeOrigin, T::OracleKey> for Pallet<T> {
		type Value = (T::OracleValue, MomentOf<T>);

		fn get(
			source: &T::RuntimeOrigin,
			id: &T::OracleKey,
		) -> Result<Option<Self::Value>, DispatchError> {
			Ok(FedValues::<T>::get(source.caller(), id))
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn set(source: &T::RuntimeOrigin, key: &T::OracleKey, value: Self::Value) {
			FedValues::<T>::insert(source.caller(), key, value)
		}
	}
}
