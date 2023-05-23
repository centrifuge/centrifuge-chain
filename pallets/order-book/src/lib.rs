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

// #[cfg(test)]
// pub(crate) mod mock;

// #[cfg(test)]
// mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use frame_support::{
	pallet_prelude::{DispatchResult, Member, OptionQuery, StorageDoubleMap, StorageNMap, *},
	traits::{tokens::AssetId, Currency, ReservableCurrency},
	Twox64Concat,
};
use orml_traits::{MultiCurrency, MultiReservableCurrency};
pub use pallet::*;

// will def have to update for dealing with multiple foreign asset types
pub type BalanceOf<T> =
	<<T as Config>::ReserveCurrency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	/// Id type of Currency to exchange
	/// Can likely combine in/out into one, separating now
	type CurrencyIn: AssetId
		+ Parameter
		+ Debug
		+ Default
		+ Member
		+ Copy
		+ MaybeSerializeDeserialize
		+ Ord
		+ TypeInfo
		+ MaxEncodedLen;

	/// Id type of Currency to exchange
	/// Can likely combine in/out into one, separating now
	type CurrencyId: AssetId
		+ Parameter
		+ Debug
		+ Default
		+ Member
		+ Copy
		+ MaybeSerializeDeserialize
		+ Ord
		+ TypeInfo
		+ MaxEncodedLen;

	type AssetRegistry: asset_registry::Inspect<
		AssetId = CurrencyIdOf<Self>,
		Balance = <Self as Config>::Balance,
		CustomMetadata = CustomMetadata,
	>;

	type SwapCurreny: MultiReservableCurrency<
		Self::AccountId,
		Balance = BalanceOf<Self>,
		CurrencyId = CurrencyId,
	>;
	type CurrencyMetadata: Member
		+ Copy
		+ Default
		+ PartialOrd
		+ Ord
		+ PartialEq
		+ Eq
		+ Debug
		+ Encode
		+ Decode
		+ TypeInfo
		+ MaxEncodedLen;

	// Storage
	#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
	pub struct SwapOrder<T> {
		pub asset_out: CurrencyId,
		pub asset_in: CurrencyId,
		pub amount_out: BalanceOf<T>,
		pub minimum_sell_ratio: BalanceOf<T>,
	}
}
