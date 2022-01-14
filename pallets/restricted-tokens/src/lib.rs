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

///! A crate that allows for checking of preconditions before sending tokens.
///! Mimics ORML-tokens Call-Api.
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use frame_support::traits::fungibles::{
	Inspect, InspectHold, Mutate, MutateHold, Transfer, Unbalanced,
};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use scale_info::TypeInfo;

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct TransferDetails<AccountId, CurrencyId, Balance> {
	pub send: AccountId,
	pub recv: AccountId,
	pub id: CurrencyId,
	pub amount: Balance,
}

impl<AccountId, CurrencyId, Balance> TransferDetails<AccountId, CurrencyId, Balance> {
	pub fn new(send: AccountId, recv: AccountId, id: CurrencyId, amount: Balance) -> Self {
		TransferDetails {
			send,
			recv,
			id,
			amount,
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use common_traits::PreConditions;
	use frame_support::scale_info::TypeInfo;
	use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, StaticLookup};
	use frame_support::sp_runtime::ArithmeticError;
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Checks the pre conditions for every transfer
		type PreConditions: PreConditions<
			TransferDetails<Self::AccountId, Self::CurrencyId, Self::Balance>,
		>;

		/// The balance type
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + TypeInfo;

		type Fungibles: Mutate<Self::AccountId>
			+ Transfer<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ Unbalanced<Self::AccountId>
			+ MutateHold<Self::AccountId>;
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
		PreConditionsNotMet,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn transfer(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			ensure!(
				T::PreConditions::check(&TransferDetails::new(
					from.clone(),
					to.clone(),
					currency_id,
					amount
				)),
				Error::<T>::PreConditionsNotMet
			);

			T::Fungibles::transfer(currency_id, &from, &to, amount, false)?;

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount,
			});

			Ok(())
		}

		#[pallet::weight(100)]
		pub fn transfer_keep_alive(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			ensure!(
				T::PreConditions::check(&TransferDetails::new(
					from.clone(),
					to.clone(),
					currency_id,
					amount
				)),
				Error::<T>::PreConditionsNotMet
			);

			T::Fungibles::transfer(currency_id, &from, &to, amount, false)?;

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount,
			});

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn transfer_all(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			keep_alive: bool,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;

			let reducible_balance = T::Fungibles::reducible_balance(currency_id, &from, keep_alive);

			ensure!(
				T::PreConditions::check(&TransferDetails::new(
					from.clone(),
					to.clone(),
					currency_id,
					reducible_balance
				)),
				Error::<T>::PreConditionsNotMet
			);

			T::Fungibles::transfer(currency_id, &from, &to, reducible_balance, keep_alive)?;

			Self::deposit_event(Event::Transfer {
				currency_id,
				from,
				to,
				amount: reducible_balance,
			});
			Ok(())
		}

		#[pallet::weight(100)]
		pub fn force_transfer(
			origin: OriginFor<T>,
			source: <T::Lookup as StaticLookup>::Source,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: T::CurrencyId,
			#[pallet::compact] amount: T::Balance,
		) -> DispatchResult {
			ensure_root(origin)?;
			let from = T::Lookup::lookup(source)?;
			let to = T::Lookup::lookup(dest)?;

			T::Fungibles::transfer(currency_id, &from, &to, amount, false)?;

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
				.checked_add(&new_reserved)
				.ok_or(ArithmeticError::Overflow)?;

			let old_reserved = T::Fungibles::balance_on_hold(currency_id, &who);
			T::Fungibles::release(currency_id, &who, old_reserved, false)?;
			T::Fungibles::set_balance(currency_id, &who, new_total)?;
			T::Fungibles::hold(currency_id, &who, new_reserved)?;

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
