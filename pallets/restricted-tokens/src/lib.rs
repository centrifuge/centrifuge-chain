// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate frame_system;

///! A crate that defines a simple permissions logic for our infrastructure.
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use common_traits::Permissions;
use frame_support::traits::fungibles;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use common_traits::GetProperties;
	use common_types::{PoolRole, UNION};
	use frame_benchmarking::BenchmarkParameter::k;
	use frame_support::scale_info::TypeInfo;
	use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;
	use frame_support::sp_runtime::ArithmeticError;
	use frame_support::traits::Contains;
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// PoolId
		type PoolId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + TypeInfo;

		/// PoolId
		type TrancheId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + TypeInfo;

		/// The balance type
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + TypeInfo;

		type Restricted: Contains<CurrencyId>
			+ GetProperties<From = Self::CurrencyId, Property = (Self::PoolId, Self::TranchId)>;

		type Fungibles: fungibles::Mutate<Self::AccountId>
			+ fungibles::Transfer<Self::AccountId>
			+ fungibles::Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ fungibles::Unbalanced<Self::AccountId>
			+ fungibles::MutateHold<Self::AccountId>;

		type Permissions: Permissions<
			Self::AccountId,
			Location = Self::PoolId,
			Role = PoolRole,
			Error = DispatchError,
		>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Transfer succeeded.
		Transfer {
			currency_id: T::CurrencyId,
			from: T::AccountId,
			to: T::AccountId,
			amount: T::Balance,
		},
		/// A balance was set by root.
		BalanceSet {
			currency_id: T::CurrencyId,
			who: T::AccountId,
			free: T::Balance,
			reserved: T::Balance,
		},
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		NoPermissionForRestrictedToken,
		PropertiesOfCurrencyIdNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn transfer(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] amount: T::Balance,
			keep_alive: bool,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			if T::Restricted::contains(currency_id) {
				if let Some((pool_id, tranche_id)) = T::Restricted::property(currency_id) {
					ensure!(
						T::Permissions::has_permission(
							pool_id,
							from.clone(),
							PoolRole::TrancheInvestor(tranche_id, UNION)
						) && T::Permissions::has_permission(
							pool_id,
							to.clone(),
							PoolRole::TrancheInvestor(tranche_id, UNION)
						),
						Error::<T>::NoPermissionForRestrictedToken
					);
				} else {
					Error::<T>::PropertiesOfCurrencyIdNotFound
				}
			}

			T::Fungibles::transfer(currency_id, from, to, amount, keep_alive)?;

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount,
			});

			Ok(())
		}

		pub fn force_transfer(
			#[pallet::weight(100)] origin: OriginFor<T>,
			source: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			ensure_root(origin)?;
			let from = T::Lookup::lookup(source)?;
			let to = T::Lookup::lookup(dest)?;

			T::Fungibles::transfer(currency_id, from, to, amount, false)?;

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount,
			});

			Ok(())
		}

		#[pallet::weight(100)]
		pub fn set_balance(
			origin: OriginFor<T>,
			who: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] new_free: T::Balance,
			#[pallet::compact] new_reserved: T::Balance,
		) -> DispatchResult {
			ensure_root(origin)?;
			let who = T::Lookup::lookup(who)?;

			let new_total = new_free
				.checked_add(new_reserved)
				.ok_or(Err(ArithmeticError::Overflow))?;

			T::Fungibles::set_balance(currency_id, who.clone(), 0)?;
			T::Fungibles::set_balance(currency_id, who.clone(), new_total)?;
			T::Fungibles::hold(currency_id, who.clone(), new_reserved)?;

			Self::deposit_event(Event::BalanceSet {
				currency_id,
				who: who,
				free: new_free,
				reserved: new_reserved,
			});

			Ok(())
		}
	}
}
