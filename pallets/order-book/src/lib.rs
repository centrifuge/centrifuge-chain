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

#[cfg(test)]
mod tests;

pub use cfg_traits::TokenSwaps;
pub use pallet::*;
#[frame_support::pallet(dev_mode)]
pub mod pallet {

	use core::fmt::Debug;

	use cfg_traits::fees::Fees;
	use cfg_types::tokens::{CustomMetadata, GeneralCurrencyIndex};
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{
		pallet_prelude::{
			DispatchResult, Member, OptionQuery, StorageDoubleMap, StorageNMap, StorageValue, *,
		},
		traits::{tokens::AssetId, Currency, ReservableCurrency},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use orml_traits::{
		asset_registry::{self, Inspect as _},
		MultiCurrency, MultiReservableCurrency,
	};
	use scale_info::TypeInfo;
	use sp_runtime::{
		traits::{AtLeast32BitUnsigned, EnsureAdd, EnsureMul, EnsureSub, Hash, One, Zero},
		FixedPointNumber, FixedPointOperand,
	};

	use super::*;

	/// Balance type for the reserve/deposit made when creating an Allowance
	pub type DepositBalanceOf<T> = <<T as Config>::ReserveCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
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
			AssetId = Self::AssetCurrencyId,
			Balance = Self::ForeignCurrencyBalance,
			CustomMetadata = CustomMetadata,
		>;

		/// CurrencyId of Asset
		type AssetCurrencyId: AssetId
			+ Parameter
			+ Debug
			+ Default
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// Currency for Reserve/Unreserve with allowlist adding/removal,
		/// given that the allowlist will be in storage
		type ReserveCurrency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		/// Fee handler for the reserve/unreserve
		/// Currently just stores the amounts, will be extended to handle
		/// reserve/unreserve as well in future
		type Fees: Fees<
			AccountId = <Self as frame_system::Config>::AccountId,
			Balance = DepositBalanceOf<Self>,
		>;

		/// Fee Key used to find amount for allowance reserve/unreserve
		type OrderFeeKey: Get<<Self::Fees as Fees>::FeeKey>;

		type ForeignCurrencyBalance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u128>
			+ TypeInfo
			+ TryInto<u128>;

		type Nonce: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ EnsureAdd
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// Type for trade-able currency
		type TradeableAsset: MultiReservableCurrency<
			Self::AccountId,
			Balance = <Self as pallet::Config>::ForeignCurrencyBalance,
			CurrencyId = Self::AssetCurrencyId,
		>;
	}
	//
	// Storage and storage types
	//
	#[derive(Clone, Copy, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
	pub struct Order<OrderId, AccountId, AssetId, ForeignCurrencyBalance> {
		pub order_id: OrderId,
		pub placing_account: AccountId,
		pub asset_in_id: AssetId,
		pub asset_out_id: AssetId,
		/// How many tokens of asset in to buy
		pub buy_amount: ForeignCurrencyBalance,
		/// Original buy amount, used for tracking amount fulfilled
		pub initial_buy_amount: ForeignCurrencyBalance,
		/// How much currency being purchased (asset in) costs with asset sold
		/// (asset out)
		pub price: ForeignCurrencyBalance,
		/// Minimum amount of an order that can be fulfilled
		/// for partial fulfillment
		pub min_fullfillment_amount: ForeignCurrencyBalance,
		/// Maximum amount of outgoing currency that can be sold
		pub max_sell_amount: ForeignCurrencyBalance,
	}

	#[derive(Clone, Copy, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
	pub struct Claim<T: Config> {
		claiming_account: T::AccountId,
		order_claiming: T::Hash,
	}

	#[pallet::storage]
	pub type Orders<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::Hash,
		Order<T::Hash, T::AccountId, T::AssetCurrencyId, T::ForeignCurrencyBalance>,
		ResultQuery<Error<T>::OrderNotFound>,
	>;

	/// Map of orders for a particular user
	/// Used to query orders for a particular user using the
	/// account id of the user as prefix
	#[pallet::storage]
	pub type UserOrders<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		T::Hash,
		Order<T::Hash, T::AccountId, T::AssetCurrencyId, T::ForeignCurrencyBalance>,
		ResultQuery<Error<T>::OrderNotFound>,
	>;

	/// Stores Nonce for orders placed
	/// Given that Nonce is to ensure that all orders have a unique ID, we can
	/// use just one Nonce, which means that we only have one val in storage,
	/// and we don't have to insert new map values upon a new account/currency
	/// order creation.
	#[pallet::storage]
	pub type NonceStore<T: Config> = StorageValue<_, T::Nonce, ValueQuery>;
	#[pallet::storage]
	pub type AssetPairOrders<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AssetCurrencyId,
		Twox64Concat,
		T::AssetCurrencyId,
		BoundedVec<T::Hash, ConstU32<1_000_000>>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		OrderCreated {
			order_id: T::Hash,
			creator_account: T::AccountId,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			buy_amount: T::ForeignCurrencyBalance,
			min_fullfillment_amount: T::ForeignCurrencyBalance,
			sell_price_limit: T::ForeignCurrencyBalance,
		},
		OrderCancelled {
			account: T::AccountId,
			order_id: T::Hash,
		},
		OrderUpdated {
			order_id: T::Hash,
			account: T::AccountId,
			buy_amount: T::ForeignCurrencyBalance,
			sell_price_limit: T::ForeignCurrencyBalance,
			min_fullfillment_amount: T::ForeignCurrencyBalance,
		},
		OrderFulfillment {
			order_id: T::Hash,
			placing_account: T::AccountId,
			fulfilling_account: T::AccountId,
			partial_fulfillment: bool,
			fulfillment_amount: T::ForeignCurrencyBalance,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			sell_price_limit: T::ForeignCurrencyBalance,
		},
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		AssetPairOrdersOverflow,
		ConflictingAssetIds,
		InsufficientAssetFunds,
		InsufficientReserveFunds,
		InvalidBuyAmount,
		InvalidMinPrice,
		InvalidAssetId,
		OrderNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as frame_system::Config>::Hash: PartialEq<<T as frame_system::Config>::Hash>,
	{
		/// Create an order, with the minimum fulfillment amount set to the buy
		/// amount, as the first iteration will not have partial fulfillment
		#[pallet::call_index(0)]
		// dummy weight for now
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(2, 2).ref_time())]
		pub fn create_order_v1(
			origin: OriginFor<T>,
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
			buy_amount: T::ForeignCurrencyBalance,
			price: T::ForeignCurrencyBalance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			Self::place_order(
				account_id, asset_in, asset_out, buy_amount, price, buy_amount,
			)?;
			Ok(())
		}

		#[pallet::call_index(1)]
		// dummy weight for now
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(2, 2).ref_time())]
		pub fn user_cancel_order(origin: OriginFor<T>, order_id: T::Hash) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			// verify order matches account
			// UserOrders using Resultquery, if signed account
			// does not match user for order id, we will get an Err Result
			Self::cancel_order(order_id)?;
			Ok(())
		}

		#[pallet::call_index(2)]
		// dummy weight for now
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(2, 2).ref_time())]
		pub fn fill_order_full(origin: OriginFor<T>, order_id: T::Hash) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id)?;
			// maybe move to ensure if we don't need these later
			// might need decimals from currency, but should hopefully be able to use FP
			// price/amounts from FP balance
			let asset_out = T::AssetRegistry::metadata(&order.asset_out_id)
				.ok_or(Error::<T>::InvalidAssetId)?;

			let asset_in =
				T::AssetRegistry::metadata(&order.asset_in_id).ok_or(Error::<T>::InvalidAssetId)?;

			let sell_amount = order.buy_amount.ensure_mul(order.price)?;

			ensure!(
				T::TradeableAsset::can_reserve(order.asset_out_id, &account_id, sell_amount),
				Error::<T>::InsufficientAssetFunds,
			);
			T::TradeableAsset::unreserve(
				order.asset_out_id,
				&order.placing_account,
				order.max_sell_amount,
			);
			T::ReserveCurrency::unreserve(
				&order.placing_account,
				T::Fees::fee_value(T::OrderFeeKey::get()),
			);
			T::TradeableAsset::transfer(
				order.asset_in_id,
				&account_id,
				&order.placing_account,
				order.buy_amount,
			)?;
			T::TradeableAsset::transfer(
				order.asset_out_id,
				&order.placing_account,
				&account_id,
				sell_amount,
			)?;
			Self::remove_order(order.order_id)?;
			Self::deposit_event(Event::OrderFulfillment {
				order_id,
				placing_account: order.placing_account,
				fulfilling_account: account_id,
				partial_fulfillment: true,
				currency_in: order.asset_in_id,
				currency_out: order.asset_out_id,
				fulfillment_amount: order.buy_amount,
				sell_price_limit: order.price,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get all orders for an account
		/// Provided for frontend to grab orders for an individual account
		pub fn get_account_orders(
			account_id: T::AccountId,
		) -> Result<
			sp_std::vec::Vec<(
				T::Hash,
				Order<T::Hash, T::AccountId, T::AssetCurrencyId, T::ForeignCurrencyBalance>,
			)>,
			Error<T>,
		> {
			Ok(<UserOrders<T>>::iter_prefix(account_id).collect())
		}

		/// Get all orders
		/// Provided for frontend to grab all open orders
		pub fn get_all_orders() -> Result<
			sp_std::vec::Vec<(
				T::Hash,
				Order<T::Hash, T::AccountId, T::AssetCurrencyId, T::ForeignCurrencyBalance>,
			)>,
			Error<T>,
		> {
			Ok(<Orders<T>>::iter().collect())
		}

		pub fn remove_order(order_id: T::Hash) -> DispatchResult {
			let order = <Orders<T>>::get(order_id)?;
			<UserOrders<T>>::remove(&order.placing_account, order.order_id);
			<Orders<T>>::remove(order.order_id);
			let mut orders = <AssetPairOrders<T>>::get(order.asset_in_id, order.asset_out_id);
			orders.retain(|o| *o != order.order_id);
			<AssetPairOrders<T>>::insert(order.asset_in_id, order.asset_out_id, orders);
			Ok(())
		}

		pub fn gen_hash(
			placer: &T::AccountId,
			asset_out: T::AssetCurrencyId,
			asset_in: T::AssetCurrencyId,
			nonce: T::Nonce,
		) -> T::Hash {
			(&placer, asset_in, asset_out, nonce).using_encoded(T::Hashing::hash)
		}
	}

	impl<T: Config> TokenSwaps<T::AccountId> for Pallet<T>
	where
		<T as frame_system::Config>::Hash: PartialEq<<T as frame_system::Config>::Hash>,
	{
		type Balance = T::ForeignCurrencyBalance;
		type CurrencyId = T::AssetCurrencyId;
		type OrderId = T::Hash;

		fn place_order(
			account: T::AccountId,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			buy_amount: T::ForeignCurrencyBalance,
			sell_price_limit: T::ForeignCurrencyBalance,
			min_fullfillment_amount: T::ForeignCurrencyBalance,
		) -> Result<Self::OrderId, DispatchError> {
			ensure!(currency_in != currency_out, Error::<T>::ConflictingAssetIds);

			ensure!(
				buy_amount != T::ForeignCurrencyBalance::zero(),
				Error::<T>::InvalidBuyAmount
			);
			ensure!(
				sell_price_limit != T::ForeignCurrencyBalance::zero(),
				Error::<T>::InvalidMinPrice
			);
			ensure!(
				T::AssetRegistry::metadata(&currency_in).is_some(),
				Error::<T>::InvalidAssetId
			);
			ensure!(
				T::AssetRegistry::metadata(&currency_out).is_some(),
				Error::<T>::InvalidAssetId
			);
			ensure!(
				T::ReserveCurrency::can_reserve(
					&account,
					T::Fees::fee_value(T::OrderFeeKey::get())
				),
				Error::<T>::InsufficientReserveFunds,
			);
			<NonceStore<T>>::try_mutate(|n| {
				*n = n.ensure_add(T::Nonce::one())?;
				Ok::<_, DispatchError>(())
			})?;
			let max_sell_amount = buy_amount.ensure_mul(sell_price_limit)?;

			ensure!(
				T::TradeableAsset::can_reserve(currency_out, &account, max_sell_amount),
				Error::<T>::InsufficientAssetFunds,
			);

			let new_nonce = <NonceStore<T>>::get();
			let order_id = Self::gen_hash(&account, currency_in, currency_out, new_nonce);
			let new_order = Order {
				order_id,
				placing_account: account.clone(),
				asset_in_id: currency_in,
				asset_out_id: currency_out,
				buy_amount,
				price: sell_price_limit,
				initial_buy_amount: buy_amount,
				min_fullfillment_amount,
				max_sell_amount,
			};

			<AssetPairOrders<T>>::try_mutate(currency_in, currency_out, |orders| {
				orders
					.try_push(order_id)
					.map_err(|_| Error::<T>::AssetPairOrdersOverflow)
			})?;

			T::ReserveCurrency::reserve(&account, T::Fees::fee_value(T::OrderFeeKey::get()))?;

			T::TradeableAsset::reserve(currency_out, &account, max_sell_amount)?;

			<Orders<T>>::insert(order_id, new_order.clone());
			<UserOrders<T>>::insert(&account, order_id, new_order);
			Self::deposit_event(Event::OrderCreated {
				creator_account: account,
				sell_price_limit,
				order_id,
				buy_amount,
				currency_in,
				currency_out,
				min_fullfillment_amount,
			});
			Ok(order_id)
		}

		fn cancel_order(order: Self::OrderId) -> DispatchResult {
			let order = <Orders<T>>::get(order)?;
			let account_id = order.placing_account;

			T::ReserveCurrency::unreserve(&account_id, T::Fees::fee_value(T::OrderFeeKey::get()));
			T::TradeableAsset::unreserve(order.asset_out_id, &account_id, order.max_sell_amount);
			Self::remove_order(order.order_id)?;
			Self::deposit_event(Event::OrderCancelled {
				account: account_id,
				order_id: order.order_id,
			});

			Ok(())
		}

		fn update_order(
			account: T::AccountId,
			order_id: Self::OrderId,
			buy_amount: T::ForeignCurrencyBalance,
			sell_price_limit: T::ForeignCurrencyBalance,
			min_fullfillment_amount: T::ForeignCurrencyBalance,
		) -> DispatchResult {
			ensure!(
				buy_amount != T::ForeignCurrencyBalance::zero(),
				Error::<T>::InvalidBuyAmount
			);
			ensure!(
				sell_price_limit != T::ForeignCurrencyBalance::zero(),
				Error::<T>::InvalidMinPrice
			);

			let order =
				<Orders<T>>::try_mutate_exists(order_id, |maybe_order| -> DispatchResult {
					let mut order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;

					let max_sell_amount = buy_amount.ensure_mul(sell_price_limit)?;
					// ensure proper amount can be, and is reserved of outgoing currency for updated
					// order.
					// Also minimise reserve/unreserve operations.
					if buy_amount != order.buy_amount || sell_price_limit != order.price {
						if max_sell_amount > order.max_sell_amount {
							let sell_reserve_diff =
								max_sell_amount.ensure_sub(order.max_sell_amount)?;
							ensure!(
								T::TradeableAsset::can_reserve(
									order.asset_out_id,
									&account,
									sell_reserve_diff
								),
								Error::<T>::InsufficientAssetFunds,
							);
							T::TradeableAsset::reserve(
								order.asset_out_id,
								&account,
								sell_reserve_diff,
							)?;
						} else {
							T::TradeableAsset::unreserve(
								order.asset_out_id,
								&account,
								order.max_sell_amount.ensure_sub(max_sell_amount)?,
							);
						}
					};
					order.buy_amount = buy_amount;
					order.price = sell_price_limit;
					order.min_fullfillment_amount = min_fullfillment_amount;
					order.max_sell_amount = max_sell_amount;

					Ok(())
				})?;
			<UserOrders<T>>::try_mutate_exists(
				&account,
				order_id,
				|maybe_order| -> DispatchResult {
					let mut order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
					let max_sell_amount = buy_amount.ensure_mul(sell_price_limit)?;
					order.buy_amount = buy_amount;
					order.price = sell_price_limit;
					order.min_fullfillment_amount = min_fullfillment_amount;
					order.max_sell_amount = max_sell_amount;
					Ok(())
				},
			)?;
			Self::deposit_event(Event::OrderUpdated {
				account,
				order_id,
				buy_amount,
				sell_price_limit,
				min_fullfillment_amount,
			});

			Ok(())
		}

		fn is_active(order: Self::OrderId) -> bool {
			<Orders<T>>::contains_key(order)
		}
	}
}
