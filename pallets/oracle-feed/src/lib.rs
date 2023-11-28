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
//! This pallet do not aggregate/validate any, it just store them by account as
//! they come. It's expected other pallet read this storage and
//! aggregate/validate the values.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{fees::PayFee, ValueProvider};
	use frame_support::{pallet_prelude::*, traits::Time};
	use frame_system::pallet_prelude::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	type MomentOf<T> = <<T as Config>::Time as Time>::Moment;

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identify an oracle value
		type OracleKey: Parameter + Member + Copy + MaxEncodedLen;

		/// Represent an oracle value
		type OracleValue: Parameter + Member + Copy + MaxEncodedLen;

		/// A way to obtain the current time
		type Time: Time;

		/// Fee for the first time a feeder feeds a value
		type FirstValuePayFee: PayFee<Self::AccountId>;
	}

	/// Store all oracle values indexed by feeder
	#[pallet::storage]
	pub(crate) type FedValues<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::OracleKey,
		(T::OracleValue, MomentOf<T>),
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Fed {
			account_id: T::AccountId,
			key: T::OracleKey,
			value: T::OracleValue,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when an oracle key is fed by first time and the account has
		/// not enough balance to pay the fees
		NotEnoughToPayFees,

		/// The key has not been fed yet
		KeyNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Permissionles call to feed an oracle key from a source with value.
		/// The first time the value is set, an extra fee is required for the
		/// feeder.
		#[pallet::weight(1_000_000)]
		#[pallet::call_index(0)]
		pub fn feed(
			origin: OriginFor<T>,
			key: T::OracleKey,
			value: T::OracleValue,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			FedValues::<T>::mutate(&who, key, |prev_value| {
				if prev_value.is_none() {
					T::FirstValuePayFee::pay(&who)?;
				}

				*prev_value = Some((value, T::Time::now()));

				Ok::<_, DispatchError>(())
			})?;

			Self::deposit_event(Event::<T>::Fed {
				account_id: who,
				key,
				value,
			});

			Ok(())
		}
	}

	impl<T: Config> ValueProvider<T::AccountId, T::OracleKey> for Pallet<T> {
		type Timestamp = MomentOf<T>;
		type Value = T::OracleValue;

		fn get(
			source: &T::AccountId,
			id: &T::OracleKey,
		) -> Result<(Self::Value, Self::Timestamp), DispatchError> {
			FedValues::<T>::get(source, id).ok_or(Error::<T>::KeyNotFound.into())
		}
	}
}
