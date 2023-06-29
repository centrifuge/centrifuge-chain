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
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, OptionQuery, StorageDoubleMap, StorageNMap, *},
		traits::{tokens::AssetId, Currency, ReservableCurrency},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::*;

	use super::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		pub type DepositBalanceOf<T> = <<T as Config>::ReserveCurrency as Currency<
			<T as frame_system::Config>::AccountId,
		>>::Balance;

		/// Currency for orderbook fees
		type ReserveCurrency: ReservableCurrency<Self::AccountId>;
		/// Id type of Currency Exchanges will take place for
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
		type OrderId: AssetId
			+ Parameter
			+ Debug
			+ Default
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// Type for trade-able currency
		type TradeableAsset: MultiReservableCurrency<
			Self::AccountId,
			Balance = DepositBalanceOf<Self>,
			CurrencyId = CurrencyId,
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

	/// Stores Nonce for orders for an account placing orders with a
	/// specific currency pair. This allows us to easily generate a
	/// deterministic unique order id for each new order, and allowing
	/// accounts to create multiple orders for a particular currency pair.
	#[pallet::storage]
	pub type AccountCurrenciesNonce<T> = StorageMap<_, u64, OptionQuery>;
}
