// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

// #[cfg(test)]
// mod mock;

// #[cfg(test)]
// mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;
// pub mod weights;
// pub use weights::*;
use crate::types::Swap;

pub mod impls;
pub mod types;

pub type SwapOf<T> = Swap<<T as Config>::Balance, <T as Config>::CurrencyId>;
pub type ForeignInvestmentInfoOf<T> = cfg_types::investments::ForeignInvestmentInfo<
	<T as frame_system::Config>::AccountId,
	<T as Config>::InvestmentId,
>;

// TODO: Remove dev_mode before merging
#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::{InvestmentCollector, StatusNotificationHook, TokenSwaps, TrancheCurrency};
	use cfg_types::investments::{ExecutedCollect, ExecutedDecrease};
	use frame_support::{dispatch::HasCompact, pallet_prelude::*};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::AtLeast32BitUnsigned;
	use types::InvestState;

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it
	/// depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's
		/// definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Type representing the weight of this pallet
		type WeightInfo: frame_system::WeightInfo;

		// TODO: Check whether we actually want something like CurrencyBalance
		/// The source of truth for the balance of accounts
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The currency type of transferrable tokens
		type CurrencyId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// The pool id type required for the investment identifier
		type PoolId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ core::fmt::Debug;

		/// The tranche id type required for the investment identifier
		type TrancheId: Member
			+ Parameter
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ TypeInfo
			+ From<[u8; 16]>;

		/// The investment identifying type required for the investment type
		type InvestmentId: TrancheCurrency<Self::PoolId, Self::TrancheId>
			+ Into<Self::CurrencyId>
			+ Clone
			+ Member
			+ Parameter
			+ Copy
			+ MaxEncodedLen;

		/// The internal investment type which handles the actual investment on
		/// top of the wrapper implementation of this Pallet
		type Investment: cfg_traits::Investment<
				Self::AccountId,
				Amount = Self::Balance,
				CurrencyId = Self::CurrencyId,
				Error = DispatchError,
				InvestmentId = Self::InvestmentId,
			> + InvestmentCollector<
				Self::AccountId,
				Error = DispatchError,
				InvestmentId = Self::InvestmentId,
				Result = (),
			>;

		/// The default sell price limit for token swaps which defines the
		/// lowest acceptable buy price.
		///
		/// TODO(@review): Since we will only support stable coins from the
		/// beginning, a global default value could be feasible or do we want to
		/// have better granularity?
		///
		/// NOTE: Can be removed once we implement a
		/// more sophisticated swap price discovery.
		type DefaultTokenSwapSellPriceLimit: Get<Self::Balance>;

		/// The default minimum fulfillment amount for token swaps.
		///
		/// TODO(@review): Since we will only support stable coins from the
		/// beginning, a global default value could be feasible or do we want to
		/// have better granularity?
		///
		/// NOTE: Can be removed once we implement a more sophisticated swap
		/// price discovery.
		type DefaultTokenMinFulfillmentAmount: Get<Self::Balance>;

		/// The token swap order identifying type
		type TokenSwapOrderId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// The type which exposes token swap order functionality such as
		/// placing and cancelling orders
		type TokenSwaps: TokenSwaps<
			Self::AccountId,
			CurrencyId = Self::CurrencyId,
			Balance = Self::Balance,
			OrderId = Self::TokenSwapOrderId,
		>;

		type ExecutedDecreaseHook: StatusNotificationHook<
			Id = ForeignInvestmentInfoOf<Self>,
			// TODO: Move to type
			Status = ExecutedDecrease<Self::Balance>,
			Error = DispatchError,
		>;

		type ExecutedCollectHook: StatusNotificationHook<
			Id = ForeignInvestmentInfoOf<Self>,
			// TODO: Convert to type
			Status = ExecutedCollect<Self::Balance, Self::CurrencyId>,
			Error = DispatchError,
		>;
	}

	/// Maps an investor and their `InvestmentId` to the corresponding
	/// `InvestState`.
	///
	/// NOTE: The lifetime of this storage starts with initializing a currency
	/// swap into the required pool currency and ends upon fully processing the
	/// investment after the potential swap. In case a swap is not required, the
	/// investment starts with `InvestState::InvestmentOngoing`.
	#[pallet::storage]
	pub(super) type InvestmentState<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		InvestState<T::Balance, T::CurrencyId>,
	>;

	/// Maps `TokenSwapOrders` to `InvestmentInfo` to implicitly enable mapping
	/// to `InvestmentState`.
	///
	/// NOTE: Here, the `payment_currency` refers to the `pool_currency`.
	///
	/// NOTE: The storage is immediately killed when the swap order is
	/// completely fulfilled even if the investment might not be fully
	/// processed.
	#[pallet::storage]
	pub(super) type ForeignInvestmentInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::TokenSwapOrderId, ForeignInvestmentInfoOf<T>>;

	/// Maps an investor and their `InvestmentId` to the corresponding
	/// `TokenSwapOrderId`.
	///
	/// NOTE: The storage is immediately killed when the swap order is
	/// completely fulfilled even if the investment might not be fully
	/// processed.
	#[pallet::storage]
	pub(super) type TokenSwapOrderIds<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		T::TokenSwapOrderId,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SomethingStored { something: u32, who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to retrieve the `InvestmentInfo` from the given
		/// `TokenSwapOrderId`.
		///
		/// NOTE: We must ensure, this can practically never happen!
		InvestmentInfoNotFound,
		/// Failed to determine whether the corresponding currency can be either
		/// used for payment or payout of an investment.
		///
		/// NOTE: We must ensure, this can practically never happen!
		InvalidInvestmentCurrency,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}
