// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

#[cfg(test)]
pub(crate) mod mock;

#[cfg(test)]
mod tests;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use core::fmt::Debug;

	use cfg_types::tokens::CustomMetadata;
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, OptionQuery, StorageDoubleMap, StorageNMap, *},
		traits::{tokens::AssetId, Currency, ReservableCurrency},
		Blake2_128Concat, Identity, Twox64Concat,
	};
	use orml_traits::{
		asset_registry::{self, Inspect as _},
		MultiCurrency, MultiReservableCurrency,
	};
	use scale_info::TypeInfo;
	use sp_runtime::{
		traits::{AtLeast32BitUnsigned, Hash},
		Saturating,
	};

	use super::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	// will def have to update for dealing with multiple foreign asset types
	pub type BalanceOf<T> = <<T as Config>::ExchangeableCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]

	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Currency for Reserve/Unreserve with allowlist adding/removal,
		/// given that the allowlist will be in storage
		type ExchangeableCurrency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		// /// Id type of Currency to exchange
		// /// Can likely combine in/out into one, separating now
		// type CurrencyId: AssetId
		// 	+ Parameter
		// 	+ Debug
		// 	+ Default
		// 	+ Member
		// 	+ Copy
		// 	+ MaybeSerializeDeserialize
		// 	+ Ord
		// 	+ TypeInfo
		// 	+ MaxEncodedLen;

		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		type AssetRegistry: asset_registry::Inspect<
			AssetId = Self::CurrencyId,
			Balance = <Self as Config>::Balance,
			CustomMetadata = CustomMetadata,
		>;

		// type SwapCurreny: MultiReservableCurrency<
		// 	Self::AccountId,
		// 	Balance = Self::Balance,
		// 	CurrencyId = Self::CurrencyId,
		// >;
		// type CurrencyMetadata: Member
		// 	+ Copy
		// 	+ Default
		// 	+ PartialOrd
		// 	+ Ord
		// 	+ PartialEq
		// 	+ Eq
		// 	+ Debug
		// 	+ Encode
		// 	+ Decode
		// 	+ TypeInfo
		// 	+ MaxEncodedLen;
	}

	// Storage
	#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
	pub struct SwapOrder<CurrencyId, Balance> {
		pub asset_out: CurrencyId,
		pub asset_in: CurrencyId,
		pub amount_out: Balance,
		pub minimum_sell_ratio: Balance,
	}

	// alternatively we can store by nmap with account/currencies
	// route
	#[pallet::storage]
	pub type AccountCurrencyTransferCountDelay<T: Config> =
		StorageMap<_, Identity, T::Hash, SwapOrder<T::CurrencyId, BalanceOf<T>>, OptionQuery>;

	//
	// Pallet Errors
	//
	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	impl<T: Config> Pallet<T> {
		pub fn gen_hash(
			account_id: T::AccountId,
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
		) -> T::Hash {
			(account_id, currency_in, currency_out).using_encoded(T::Hashing::hash)
		}
	}
}
