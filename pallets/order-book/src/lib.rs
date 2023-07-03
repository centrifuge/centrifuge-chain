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

// This pallet was made using the ZeitGeist Orderbook pallet as a reference;
// with much of the code being copied or adapted from that pallet.
// The ZeitGeist Orderbook pallet can be found here: https://github.com/zeitgeistpm/zeitgeist/tree/main/zrml/orderbook-v1

#![cfg_attr(not(feature = "std"), no_std)]

//! This module adds an orderbook pallet, allowing oders for currency swaps to
//! be placed and fulfilled for currencies in an asset registry.

#[cfg(test)]
pub(crate) mod mock;

pub use pallet::*;

#[frame_support::pallet(dev_mode)]
pub mod pallet {

	use core::fmt::Debug;

	use cfg_types::tokens::{CustomMetadata, GeneralCurrencyIndex};
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, OptionQuery, StorageDoubleMap, StorageNMap, *},
		traits::{tokens::AssetId, Currency, ReservableCurrency},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::*;
	use orml_traits::{
		asset_registry::{self, Inspect as _},
		MultiCurrency, MultiReservableCurrency,
	};
	use scale_info::TypeInfo;
	use sp_runtime::traits::{AtLeast32BitUnsigned, Hash};

	use super::*;

	pub type CurrencyIdOf<T> = <T as Config>::CurrencyId;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]

	pub struct Pallet<T>(_);
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type AssetRegistry: asset_registry::Inspect<
			AssetId = CurrencyIdOf<Self>,
			Balance = <Self as Config>::Balance,
			CustomMetadata = CustomMetadata,
		>;

		/// Id type of Currency exchanges will take place for
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

		/// Id type for placed Orders
		type OrderId: Parameter
			+ Debug
			+ Default
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// Type for placed Orders
		type Nonce: Parameter
			+ Debug
			+ Default
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ AtLeast32BitUnsigned
			+ MaxEncodedLen;

		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// Type for trade-able currency
		type TradeableAsset: MultiReservableCurrency<
			Self::AccountId,
			Balance = <Self as pallet::Config>::Balance,
			CurrencyId = CurrencyIdOf<Self>,
		>;
	}
	//
	// Storage and storage types
	//
	pub struct Order<AccountId, Balance, TradeableAsset> {
		placing_account: AccountId,
		asset_out_type: TradeableAsset,
		asset_in_type: TradeableAsset,
		price: Balance,
		sell_amount: Balance,
	}

	pub struct Claim<AccountId, OrderId> {
		claiming_account: AccountId,
		order_claiming: OrderId,
	}

	/// Stores Nonce for orders placed
	/// Given that Nonce is to ensure that all orders have a unique ID, we can
	/// use just one Nonce, which means that we only have one val in storage,
	/// and we don't have to insert new map values upon a new account/currency
	/// order creation.
	#[pallet::storage]
	pub type NonceStore<T: Config> = StorageValue<_, T::Nonce, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	impl<T: Config> Pallet<T> {
		pub fn gen_hash(
			placer: &T::AccountId,
			asset_out: T::CurrencyId,
			asset_in: T::CurrencyId,
			nonce: T::Nonce,
		) -> T::Hash {
			(&placer, asset_in, asset_out, nonce).using_encoded(T::Hashing::hash)
		}
	}
}
