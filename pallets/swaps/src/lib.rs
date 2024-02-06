// Copyright 2024 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! # Swaps pallet: Enables applying swaps independiently of previous swaps in the same or opposite
//! directions.

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{StatusNotificationHook, Swaps, TokenSwaps};
	use cfg_types::investments::{Swap, SwapState};
	use frame_support::pallet_prelude::*;
	use sp_runtime::traits::AtLeast32BitUnsigned;

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it
	/// depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Represents an amount that can be swapped
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// An identification for a swap
		type SwapId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + MaxEncodedLen;

		/// An identification for an order
		type OrderId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + MaxEncodedLen;

		/// The currency type of transferrable tokens
		type CurrencyId: Parameter + Member + Copy + MaxEncodedLen;

		/// The type which exposes token swap order functionality
		type OrderBook: TokenSwaps<
			Self::AccountId,
			CurrencyId = Self::CurrencyId,
			BalanceIn = Self::Balance,
			BalanceOut = Self::Balance,
			OrderId = Self::OrderId,
			OrderDetails = Swap<Self::Balance, Self::CurrencyId>,
		>;

		/// The hook which acts upon a (partially) fulfilled the swap
		type FulfilledOrderHook: StatusNotificationHook<
			Id = Self::SwapId,
			Status = SwapState<Self::Balance, Self::Balance, Self::CurrencyId>,
			Error = DispatchError,
		>;
	}

	/// Maps a `OrderId` to its corresponding `AccountId` and `SwapId`
	///
	/// NOTE: The storage is killed when the swap order no longer exists
	#[pallet::storage]
	pub(super) type OrderIdToSwapId<T: Config> =
		StorageMap<_, Blake2_128Concat, T::OrderId, (T::AccountId, T::SwapId)>;

	/// Maps an `AccountId` and `SwapId` to its corresponding `OrderId`
	///
	/// NOTE: The storage is killed when the swap order no longer exists
	#[pallet::storage]
	pub(super) type SwapIdToOrderId<T: Config> =
		StorageMap<_, Blake2_128Concat, (T::AccountId, T::SwapId), T::OrderId>;

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to retrieve the order.
		OrderIdNotFound,

		/// Failed to retrieve the swap.
		SwapIdNotFound,
	}

	impl<T: Config> Pallet<T> {
		pub fn swap_id(order_id: T::OrderId) -> Result<(T::AccountId, T::SwapId), DispatchError> {
			OrderIdToSwapId::<T>::get(order_id).ok_or(Error::<T>::SwapIdNotFound.into())
		}

		pub fn order_id(
			account: &T::AccountId,
			swap_id: T::SwapId,
		) -> Result<T::OrderId, DispatchError> {
			SwapIdToOrderId::<T>::get((account, swap_id)).ok_or(Error::<T>::OrderIdNotFound.into())
		}

		fn update_id(
			who: &T::AccountId,
			swap_id: T::SwapId,
			new_order_id: Option<T::OrderId>,
		) -> DispatchResult {
			let previous_order_id = SwapIdToOrderId::<T>::get((who, swap_id));

			if previous_order_id != new_order_id {
				if let Some(old_id) = previous_order_id {
					OrderIdToSwapId::<T>::remove(old_id);
					SwapIdToOrderId::<T>::remove((who.clone(), swap_id));
				}

				if let Some(new_id) = new_order_id {
					OrderIdToSwapId::<T>::insert(new_id, (who.clone(), swap_id));
					SwapIdToOrderId::<T>::insert((who.clone(), swap_id), new_id);
				}
			}

			Ok(())
		}
	}

	/// Trait to perform swaps without handling directly an order book
	impl<T: Config> Swaps<T::AccountId> for Pallet<T> {
		type Amount = T::Balance;
		type CurrencyId = T::CurrencyId;
		type SwapId = T::SwapId;

		fn apply_swap(who: &T::AccountId, swap_id: Self::SwapId) -> DispatchResult {
			todo!()
		}

		fn pending_amount(
			who: &T::AccountId,
			swap_id: Self::SwapId,
			from_currency: Self::CurrencyId,
		) -> Result<Self::Amount, DispatchError> {
			todo!()
		}

		fn valid_pair(currency_in: Self::CurrencyId, currency_out: Self::CurrencyId) -> bool {
			todo!()
		}
	}

	impl<T: Config> StatusNotificationHook for Pallet<T> {
		type Error = DispatchError;
		type Id = T::OrderId;
		type Status = SwapState<T::Balance, T::Balance, T::CurrencyId>;

		fn notify_status_change(
			order_id: T::OrderId,
			swap_state: SwapState<T::Balance, T::Balance, T::CurrencyId>,
		) -> DispatchResult {
			todo!()
		}
	}
}
