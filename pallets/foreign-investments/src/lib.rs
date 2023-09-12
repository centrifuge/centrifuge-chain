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

//! # Foreign Investment pallet
//!
//! Enables investing, redeeming and collecting in foreign and non-foreign
//! currencies. Can be regarded as an extension of `pallet-investment` which
//! provides the same toolset for pool (non-foreign) currencies.
//!
//! - [`Pallet`]
//!
//! ## Assumptions
//!
//! - The implementer of the pallet's associated `Investment` type sends
//!   notifications for collected investments via `CollectedInvestmentHook` and
//!   for collected redemptions via `CollectedRedemptionHook`]. Otherwise the
//!   payment and collected amounts for foreign investments/redemptions are
//!   never incremented.
//! - The implementer of the pallet's associated `TokenSwaps` type sends
//!   notifications for fulfilled swap orders via the `FulfilledSwapOrderHook`.
//!   Otherwise investment/redemption states can never advance the
//!   `ActiveSwapInto*Currency` state.
//! - The implementer of the pallet's associated `TokenSwaps` type sends
//!   notifications for fulfilled swap orders via the `FulfilledSwapOrderHook`.
//!   Otherwise investment/redemption states can never advance the
//!   `ActiveSwapInto*Currency` state.
//! - The implementer of the pallet's associated
//!   `DecreasedForeignInvestOrderHook` type handles the refund of the decreased
//!   amount to the investor.
//! - The implementer of the pallet's associated
//!   `CollectedForeignRedemptionHook` type handles the transfer of the
//!   collected amount in foreign currency to the investor.

#![cfg_attr(not(feature = "std"), no_std)]

use cfg_types::investments::Swap;
/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

pub mod errors;
pub mod hooks;
pub mod impls;
pub mod types;

pub type SwapOf<T> = Swap<<T as Config>::Balance, <T as Config>::CurrencyId>;
pub type ForeignInvestmentInfoOf<T> = cfg_types::investments::ForeignInvestmentInfo<
	<T as frame_system::Config>::AccountId,
	<T as Config>::InvestmentId,
	crate::types::TokenSwapReason,
>;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		investments::{Investment as InvestmentT, InvestmentCollector, TrancheCurrency},
		PoolInspect, StatusNotificationHook, TokenSwaps,
	};
	use cfg_types::investments::{
		CollectedAmount, ExecutedForeignCollectRedeem, ExecutedForeignDecreaseInvest,
	};
	use errors::{InvestError, RedeemError};
	use frame_support::{dispatch::HasCompact, pallet_prelude::*};
	use sp_runtime::traits::AtLeast32BitUnsigned;
	use types::{InvestState, RedeemState};

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
		type Investment: InvestmentT<
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

		/// Type for price ratio for cost of incoming currency relative to
		/// outgoing
		type Rate: Parameter
			+ Member
			+ sp_runtime::FixedPointNumber
			+ sp_runtime::traits::EnsureMul
			+ sp_runtime::traits::EnsureDiv
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// The default sell rate for token swaps which will be applied to all
		/// swaps created/updated through Foreign Investments.
		///
		/// Example: Say this rate is set to 3/2, then the incoming currency
		/// should never cost more than 1.5 of the outgoing currency.
		///
		/// NOTE: Can be removed once we implement a
		/// more sophisticated swap price discovery. For now, this should be set
		/// to one.
		#[pallet::constant]
		type DefaultTokenSellRate: Get<Self::Rate>;

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
			OrderDetails = Swap<Self::Balance, Self::CurrencyId>,
			SellRatio = Self::Rate,
		>;

		/// The hook type which acts upon a finalized investment decrement.
		type DecreasedForeignInvestOrderHook: StatusNotificationHook<
			Id = cfg_types::investments::ForeignInvestmentInfo<
				Self::AccountId,
				Self::InvestmentId,
				(),
			>,
			Status = ExecutedForeignDecreaseInvest<Self::Balance, Self::CurrencyId>,
			Error = DispatchError,
		>;

		/// The hook type which acts upon a finalized redemption collection.
		type CollectedForeignRedemptionHook: StatusNotificationHook<
			Id = cfg_types::investments::ForeignInvestmentInfo<
				Self::AccountId,
				Self::InvestmentId,
				(),
			>,
			Status = ExecutedForeignCollectRedeem<Self::Balance, Self::CurrencyId>,
			Error = DispatchError,
		>;

		/// Type which provides a conversion from one currency amount to another
		/// currency amount.
		type CurrencyConverter: cfg_traits::SimpleCurrencyConversion<
			Balance = Self::Balance,
			Currency = Self::CurrencyId,
			Error = DispatchError,
		>;

		/// The source of truth for pool currencies.
		type PoolInspect: PoolInspect<
			Self::AccountId,
			Self::CurrencyId,
			PoolId = Self::PoolId,
			TrancheId = Self::TrancheId,
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
	pub type InvestmentState<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		InvestState<T>,
		ValueQuery,
	>;

	/// Maps an investor and their `InvestmentId` to the corresponding
	/// `RedeemState`.
	///
	/// NOTE: The lifetime of this storage starts with increasing a redemption
	/// which requires owning at least the amount of tranche tokens by which the
	/// redemption shall be increased by. It ends with transferring back
	/// the swapped return currency to the corresponding source domain from
	/// which the investment originated. The lifecycle must go through the
	/// following stages:
	/// 	1. Increase redemption --> Initialize storage
	/// 	2. Fully process pending redemption
	/// 	3. Collect redemption
	/// 	4. Trigger swap from pool to return currency
	/// 	5. Completely fulfill swap order
	/// 	6. Transfer back to source domain --> Kill storage entry
	#[pallet::storage]
	pub type RedemptionState<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		RedeemState<T::Balance, T::CurrencyId>,
		ValueQuery,
	>;

	/// Maps a token swap order id to the corresponding `ForeignInvestmentInfo`
	/// to implicitly enable mapping to `InvestmentState` and `RedemptionState`.
	///
	/// NOTE: The storage is immediately killed when the swap order is
	/// completely fulfilled even if the corresponding investment and/or
	/// redemption might not be fully processed.
	#[pallet::storage]
	#[pallet::getter(fn foreign_investment_info)]
	pub(super) type ForeignInvestmentInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::TokenSwapOrderId, ForeignInvestmentInfoOf<T>>;

	/// Maps an investor and their `InvestmentId` to the corresponding
	/// `TokenSwapOrderId`.
	///
	/// NOTE: The storage is immediately killed when the swap order is
	/// completely fulfilled even if the investment might not be fully
	/// processed.
	#[pallet::storage]
	#[pallet::getter(fn token_swap_order_ids)]
	pub(super) type TokenSwapOrderIds<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		T::TokenSwapOrderId,
	>;

	/// Maps an investor and their `InvestmentId` to the collected investment
	/// amount, i.e., the payment amount of pool currency burned for the
	/// conversion into collected amount of tranche tokens based on the
	/// fulfillment price(s).
	///
	/// NOTE: The lifetime of this storage starts with receiving a notification
	/// of an executed investment via the `CollectedInvestmentHook`. It ends
	/// with transferring the collected tranche tokens by executing
	/// `transfer_collected_investment` which is part of
	/// `collect_foreign_investment`.
	#[pallet::storage]
	pub type CollectedInvestment<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		CollectedAmount<T::Balance>,
		ValueQuery,
	>;

	/// Maps an investor and their `InvestmentId` to the collected redemption
	/// amount, i.e., the payment amount of tranche tokens burned for the
	/// conversion into collected pool currency based on the
	/// fulfillment price(s).
	///
	/// NOTE: The lifetime of this storage starts with receiving a notification
	/// of an executed redemption collection into pool currency via the
	/// `CollectedRedemptionHook`. It ends with having swapped the entire amount
	/// to foreign currency which is assumed to be asynchronous.
	#[pallet::storage]
	pub type CollectedRedemption<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		CollectedAmount<T::Balance>,
		ValueQuery,
	>;

	/// Maps an investor and their investment id to the foreign payout currency
	/// requested on the initial redemption increment.
	///
	/// NOTE: The lifetime of this storage mirrors the one of `RedemptionState`.
	#[pallet::storage]
	pub type RedemptionPayoutCurrency<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		T::CurrencyId,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ForeignInvestmentUpdated {
			investor: T::AccountId,
			investment_id: T::InvestmentId,
			state: InvestState<T>,
		},
		ForeignInvestmentCleared {
			investor: T::AccountId,
			investment_id: T::InvestmentId,
		},
		ForeignRedemptionUpdated {
			investor: T::AccountId,
			investment_id: T::InvestmentId,
			state: RedeemState<T::Balance, T::CurrencyId>,
		},
		ForeignRedemptionCleared {
			investor: T::AccountId,
			investment_id: T::InvestmentId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to retrieve the `TokenSwapReason` from the given
		/// `TokenSwapOrderId`.
		InvestmentInfoNotFound,
		/// The provided currency does not match the one provided when the first
		/// redemption increase was triggered.
		///
		/// NOTE: As long as the `RedemptionState` has not been cleared, the
		/// payout currency cannot change from the initially provided one.
		InvalidRedemptionPayoutCurrency,
		/// Failed to retrieve the `TokenSwapReason` from the given
		/// `TokenSwapOrderId`.
		TokenSwapReasonNotFound,
		/// Failed to transition the `InvestState`.
		InvestError(InvestError),
		/// Failed to transition the `RedeemState.`
		RedeemError(RedeemError),
	}

	impl<T> From<InvestError> for Error<T> {
		fn from(error: InvestError) -> Self {
			Error::<T>::InvestError(error)
		}
	}

	impl<T> From<RedeemError> for Error<T> {
		fn from(error: RedeemError) -> Self {
			Error::<T>::RedeemError(error)
		}
	}
}
