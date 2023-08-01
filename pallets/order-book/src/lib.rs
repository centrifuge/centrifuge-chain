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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

pub use cfg_traits::TokenSwaps;
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {

	use core::fmt::Debug;

	use cfg_traits::fees::Fees;
	use cfg_types::tokens::CustomMetadata;
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, StorageDoubleMap, StorageValue, *},
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
		traits::{AtLeast32BitUnsigned, EnsureAdd, EnsureMul, EnsureSub, One, Zero},
		FixedPointOperand,
	};

	use super::*;

	/// Balance type for the reserve/deposit made when creating an Allowance
	pub type DepositBalanceOf<T> = <<T as Config>::ReserveCurrency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	/// Order of pallet config type
	pub type OrderOf<T> = Order<
		<T as Config>::OrderIdNonce,
		<T as frame_system::Config>::AccountId,
		<T as Config>::AssetCurrencyId,
		<T as Config>::ForeignCurrencyBalance,
	>;

	pub type FeeBalance<T> = <<T as Config>::ReserveCurrency as Currency<
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

		/// Asset registry for foreign currencies we can take orders for.
		type AssetRegistry: asset_registry::Inspect<
			AssetId = Self::AssetCurrencyId,
			Balance = Self::ForeignCurrencyBalance,
			CustomMetadata = CustomMetadata,
		>;

		/// CurrencyId of Assets that an order can be made for
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
		/// Uses default balance type as opposed to ForeignCurrencyBalance
		type Fees: Fees<
			AccountId = <Self as frame_system::Config>::AccountId,
			Balance = DepositBalanceOf<Self>,
		>;

		/// Fee Key used to find amount for allowance reserve/unreserve
		type OrderFeeKey: Get<<Self::Fees as Fees>::FeeKey>;

		/// Token Id for token used for fee reserving
		/// as represented by `AssetCurrencyId`.
		/// Used for reserve-able fund checking to ensure
		/// amount traded and storage fees can be reserved
		/// when trading for fee currency.
		/// This should typically be native chain currency.
		type FeeCurrencyId: Get<Self::AssetCurrencyId>;

		/// Balance type for currencies we can place orders for
		/// Seperate type from Balance in case different type used for other
		/// currencies, i.e. when Balance is u64, but foreign currencies using
		/// u128
		type ForeignCurrencyBalance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ TypeInfo
			+ TryInto<<Self::ReserveCurrency as Currency<Self::AccountId>>::Balance>;

		/// Type used for OrderId. OrderIdNonce ensures each
		/// OrderId is unique. OrderIdNonce incremented with each new order.
		type OrderIdNonce: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ EnsureAdd
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// Type for currency orders can be made for
		type TradeableAsset: MultiReservableCurrency<
			Self::AccountId,
			Balance = <Self as pallet::Config>::ForeignCurrencyBalance,
			CurrencyId = Self::AssetCurrencyId,
		>;

		/// Size of order id bounded vec in storage
		#[pallet::constant]
		type OrderPairVecSize: Get<u32>;

		/// Type for pallet weights
		type Weights: WeightInfo;
	}
	//
	// Storage and storage types
	//
	/// Order Storage item.
	/// Contains fields relevant to order information
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

	/// Map of Orders to look up orders by their order id.
	#[pallet::storage]
	pub type Orders<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::OrderIdNonce,
		Order<T::OrderIdNonce, T::AccountId, T::AssetCurrencyId, T::ForeignCurrencyBalance>,
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
		T::OrderIdNonce,
		OrderOf<T>,
		ResultQuery<Error<T>::OrderNotFound>,
	>;

	/// Stores OrderIdNonce for orders placed
	/// Given that OrderIdNonce is to ensure that all orders have a unique ID,
	/// we can use just one OrderIdNonce, which means that we only have one val
	/// in storage, and we don't have to insert new map values upon a new
	/// account/currency order creation.
	#[pallet::storage]
	pub type OrderIdNonceStore<T: Config> = StorageValue<_, T::OrderIdNonce, ValueQuery>;

	/// Map of Vec containing OrderIds of same asset in/out pairs.
	/// Allows looking up orders available corresponding pairs.
	#[pallet::storage]
	pub type AssetPairOrders<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AssetCurrencyId,
		Twox64Concat,
		T::AssetCurrencyId,
		BoundedVec<T::OrderIdNonce, T::OrderPairVecSize>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event emitted when an order is created.
		OrderCreated {
			order_id: T::OrderIdNonce,
			creator_account: T::AccountId,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			buy_amount: T::ForeignCurrencyBalance,
			min_fullfillment_amount: T::ForeignCurrencyBalance,
			sell_price_limit: T::ForeignCurrencyBalance,
		},
		/// Event emitted when an order is cancelled.
		OrderCancelled {
			account: T::AccountId,
			order_id: T::OrderIdNonce,
		},
		/// Event emitted when an order is updated.
		OrderUpdated {
			order_id: T::OrderIdNonce,
			account: T::AccountId,
			buy_amount: T::ForeignCurrencyBalance,
			sell_price_limit: T::ForeignCurrencyBalance,
			min_fullfillment_amount: T::ForeignCurrencyBalance,
		},
		/// Event emitted when an order is fulfilled.
		/// Can be for either partial or total fulfillment.
		/// Contains amount fulfilled, and whether fulfillment was partial or
		/// full.
		OrderFulfillment {
			order_id: T::OrderIdNonce,
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
		/// Error when the number of orders for a trading pair has exceeded the
		/// BoundedVec size for the order pair for the currency pair in
		/// question.
		AssetPairOrdersOverflow,
		/// Error when order is placed attempting to exchange assets of the same
		/// type.
		ConflictingAssetIds,
		/// Error when an account cannot reserve or transfer the amount
		/// specified for trade, or amount to be fulfilled.
		InsufficientAssetFunds,
		/// Error when an account does not have enough funds in chains reserve
		/// currency to place order.
		InsufficientReserveFunds,
		/// Error when account tries to buy an invalid amount of currency --
		/// currently `0`.
		InvalidBuyAmount,
		/// Error when an account specifies an invalid buy price -- currently
		/// `0`.
		InvalidMinPrice,
		/// Error when an order is placed with a currency that is not in the
		/// asset registry.
		InvalidAssetId,
		/// Error when an operation is attempted on an order id that is not in
		/// storage.
		OrderNotFound,
		/// Error when a user attempts an action on an order they are not
		/// authorised to perform, such as cancelling another accounts order.
		Unauthorised,
		/// Error when unable to convert fee balance to asset balance when asset
		/// out matches fee currency
		BalanceConversionErr,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as frame_system::Config>::Hash: PartialEq<<T as frame_system::Config>::Hash>,
	{
		/// Create an order, with the minimum fulfillment amount set to the buy
		/// amount, as the first iteration will not have partial fulfillment
		#[pallet::call_index(0)]
		#[pallet::weight(T::Weights::create_order_v1())]
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

		///  Cancel an existing order that had been created by calling account.
		#[pallet::call_index(1)]
		#[pallet::weight(T::Weights::user_cancel_order())]
		pub fn user_cancel_order(
			origin: OriginFor<T>,
			order_id: T::OrderIdNonce,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			// verify order matches account
			// UserOrders using Resultquery, if signed account
			// does not match user for order id, we will get an Err Result
			let order = <Orders<T>>::get(order_id)?;
			if account_id == order.placing_account {
				Self::cancel_order(order_id)?;
				Ok(())
			} else {
				Err(DispatchError::from(Error::<T>::Unauthorised))
			}
		}

		/// Fill an existing order, fulfilling the entire order.
		#[pallet::call_index(2)]
		#[pallet::weight(T::Weights::fill_order_full())]
		pub fn fill_order_full(origin: OriginFor<T>, order_id: T::OrderIdNonce) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id)?;
			// maybe move to ensure if we don't need these later
			// might need decimals from currency, but should hopefully be able to use FP
			// price/amounts from FP balance

			let sell_amount = order.buy_amount.ensure_mul(order.price)?;

			ensure!(
				T::TradeableAsset::can_reserve(order.asset_in_id, &account_id, order.buy_amount),
				Error::<T>::InsufficientAssetFunds,
			);

			Self::unreserve_order(&order)?;
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
		/// Remove an order from storage
		pub fn remove_order(order_id: T::OrderIdNonce) -> DispatchResult {
			let order = <Orders<T>>::get(order_id)?;
			<UserOrders<T>>::remove(&order.placing_account, order.order_id);
			<Orders<T>>::remove(order.order_id);
			let mut orders = <AssetPairOrders<T>>::get(order.asset_in_id, order.asset_out_id);
			orders.retain(|o| *o != order.order_id);
			<AssetPairOrders<T>>::insert(order.asset_in_id, order.asset_out_id, orders);
			Ok(())
		}

		/// Get reserve amount when fee and out currency are the same
		pub fn get_combined_reserve(
			sell_amount: T::ForeignCurrencyBalance,
		) -> Result<FeeBalance<T>, DispatchError> {
			let fee_amount = T::Fees::fee_value(T::OrderFeeKey::get());

			let sell_reserve_balance: FeeBalance<T> = sell_amount
				.try_into()
				.map_err(|_| Error::<T>::BalanceConversionErr)?;
			Ok(sell_reserve_balance.ensure_add(fee_amount)?)
		}

		/// Unreserve funds for an order that is finished either
		/// through fulfillment or cancellation.
		pub fn unreserve_order(order: &OrderOf<T>) -> Result<(), DispatchError> {
			if T::FeeCurrencyId::get() == order.asset_out_id {
				let total_reserve_amount = Self::get_combined_reserve(order.max_sell_amount)?;
				T::ReserveCurrency::unreserve(&order.placing_account, total_reserve_amount);
			} else {
				T::TradeableAsset::unreserve(
					order.asset_out_id,
					&order.placing_account,
					order.max_sell_amount,
				);
				T::ReserveCurrency::unreserve(
					&order.placing_account,
					T::Fees::fee_value(T::OrderFeeKey::get()),
				);
			};
			Ok(())
		}
	}

	impl<T: Config> TokenSwaps<T::AccountId> for Pallet<T>
	where
		<T as frame_system::Config>::Hash: PartialEq<<T as frame_system::Config>::Hash>,
	{
		type Balance = T::ForeignCurrencyBalance;
		type CurrencyId = T::AssetCurrencyId;
		type OrderId = T::OrderIdNonce;

		/// Creates an order.
		/// Verify funds available in, and reserve for  both chains fee currency
		/// for storage fee, and amount of outgoing currency as determined by
		/// the buy amount and price.
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
			<OrderIdNonceStore<T>>::try_mutate(|n| {
				*n = n.ensure_add(T::OrderIdNonce::one())?;
				Ok::<_, DispatchError>(())
			})?;
			let max_sell_amount = buy_amount.ensure_mul(sell_price_limit)?;

			let fee_amount = T::Fees::fee_value(T::OrderFeeKey::get());
			if T::FeeCurrencyId::get() == currency_out {
				let total_reserve_amount = Self::get_combined_reserve(max_sell_amount)?;
				T::ReserveCurrency::reserve(&account, total_reserve_amount)?;
			} else {
				T::ReserveCurrency::reserve(&account, fee_amount)?;

				T::TradeableAsset::reserve(currency_out, &account, max_sell_amount)?;
			}

			let order_id = <OrderIdNonceStore<T>>::get();
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

		/// Cancel an existing order.
		/// Unreserve currency reserved for trade as well storage fee.
		fn cancel_order(order: Self::OrderId) -> DispatchResult {
			let order = <Orders<T>>::get(order)?;
			let account_id = order.placing_account.clone();

			Self::unreserve_order(&order)?;
			Self::remove_order(order.order_id)?;
			Self::deposit_event(Event::OrderCancelled {
				account: account_id,
				order_id: order.order_id,
			});

			Ok(())
		}

		/// Update an existing order.
		/// Update outgoing asset currency reserved to match new amount or price
		/// if either have changed.
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
						if T::FeeCurrencyId::get() == order.asset_out_id {
							let sell_reserve_diff_balance: FeeBalance<T> = sell_reserve_diff
								.try_into()
								.map_err(|_| Error::<T>::BalanceConversionErr)?;
							T::ReserveCurrency::reserve(&account, sell_reserve_diff_balance)?
						} else {
							T::TradeableAsset::reserve(
								order.asset_out_id,
								&account,
								sell_reserve_diff,
							)?;
						}
					} else {
						let sell_reserve_diff =
							order.max_sell_amount.ensure_sub(max_sell_amount)?;
						if T::FeeCurrencyId::get() == order.asset_out_id {
							let sell_reserve_diff_balance: FeeBalance<T> = sell_reserve_diff
								.try_into()
								.map_err(|_| Error::<T>::BalanceConversionErr)?;
							T::ReserveCurrency::unreserve(&account, sell_reserve_diff_balance);
						} else {
							T::TradeableAsset::unreserve(
								order.asset_out_id,
								&account,
								sell_reserve_diff,
							);
						}
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

		/// Check whether an order is active.
		fn is_active(order: Self::OrderId) -> bool {
			<Orders<T>>::contains_key(order)
		}
	}
}
