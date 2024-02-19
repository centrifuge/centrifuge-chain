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
//!   for collected redemptions via `CollectedRedemptionHook`].
//! - The implementer of the pallet's associated `TokenSwaps` type sends
//!   notifications for fulfilled swap orders via the `FulfilledSwapHook`.
//! - The implementer of the pallet's associated
//!   `DecreasedForeignInvestOrderHook` type handles the refund of the decreased
//!   amount to the investor.
//! - The implementer of the pallet's associated
//!   `CollectedForeignRedemptionHook` type handles the transfer of the
//!   collected amount in foreign currency to the investor.

#![cfg_attr(not(feature = "std"), no_std)]

use cfg_traits::Swap;
pub use impls::{CollectedInvestmentHook, CollectedRedemptionHook, FulfilledSwapHook};
pub use pallet::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod entities;
mod impls;

#[derive(
	Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
pub enum Action {
	Investment,
	Redemption,
}

/// Identification of a foreign investment/redemption
pub type ForeignId<T> = (
	<T as frame_system::Config>::AccountId,
	<T as Config>::InvestmentId,
	Action,
);

/// Swap alias
pub type SwapOf<T> = Swap<<T as Config>::SwapBalance, <T as Config>::CurrencyId>;

/// Identification of a swap from foreing-investment perspective
pub type SwapId<T> = (<T as Config>::InvestmentId, Action);

/// TrancheId Identification
pub type TrancheIdOf<T> = <<T as Config>::PoolInspect as cfg_traits::PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::TrancheId;

/// PoolId identification
pub type PoolIdOf<T> = <<T as Config>::PoolInspect as cfg_traits::PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::PoolId;

/// Get the pool currency associated to a investment_id
pub fn pool_currency_of<T: pallet::Config>(
	investment_id: T::InvestmentId,
) -> Result<T::CurrencyId, sp_runtime::DispatchError> {
	use cfg_traits::{investments::TrancheCurrency, PoolInspect};

	T::PoolInspect::currency_for(investment_id.of_pool()).ok_or(Error::<T>::PoolNotFound.into())
}

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		investments::{Investment, InvestmentCollector, TrancheCurrency},
		PoolInspect, StatusNotificationHook, Swaps,
	};
	use cfg_types::investments::{ExecutedForeignCollect, ExecutedForeignDecreaseInvest};
	use frame_support::pallet_prelude::*;
	use sp_runtime::traits::AtLeast32BitUnsigned;

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it
	/// depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Represents a foreign amount
		type ForeignBalance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ Into<Self::SwapBalance>
			+ From<Self::SwapBalance>
			+ Into<Self::PoolBalance>;

		/// Represents a pool amount
		type PoolBalance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ Into<Self::SwapBalance>
			+ From<Self::SwapBalance>
			+ Into<Self::ForeignBalance>;

		/// Represents a tranche token amount
		type TrancheBalance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen;

		/// Any balances used in TokenSwaps
		type SwapBalance: Parameter + Member + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;

		/// The currency type of transferrable tokens
		type CurrencyId: Parameter + Member + Copy + MaxEncodedLen;

		/// The investment identifying type required for the investment type
		type InvestmentId: TrancheCurrency<PoolIdOf<Self>, TrancheIdOf<Self>>
			+ Parameter
			+ Copy
			+ MaxEncodedLen;

		/// The internal investment type which handles the actual investment on
		/// top of the wrapper implementation of this Pallet
		type Investment: Investment<
				Self::AccountId,
				Amount = Self::PoolBalance,
				TrancheAmount = Self::TrancheBalance,
				CurrencyId = Self::CurrencyId,
				Error = DispatchError,
				InvestmentId = Self::InvestmentId,
			> + InvestmentCollector<
				Self::AccountId,
				Error = DispatchError,
				InvestmentId = Self::InvestmentId,
				Result = (),
			>;

		/// The type which exposes token swap order functionality such as
		/// placing and cancelling orders
		type Swaps: Swaps<
			Self::AccountId,
			CurrencyId = Self::CurrencyId,
			Amount = Self::SwapBalance,
			SwapId = SwapId<Self>,
			OrderDetails = SwapOf<Self>,
		>;

		/// The hook type which acts upon a finalized investment decrement.
		type DecreasedForeignInvestOrderHook: StatusNotificationHook<
			Id = (Self::AccountId, Self::InvestmentId),
			Status = ExecutedForeignDecreaseInvest<Self::ForeignBalance, Self::CurrencyId>,
			Error = DispatchError,
		>;

		/// The hook type which acts upon a finalized redemption collection.
		type CollectedForeignRedemptionHook: StatusNotificationHook<
			Id = (Self::AccountId, Self::InvestmentId),
			Status = ExecutedForeignCollect<
				Self::ForeignBalance,
				Self::TrancheBalance,
				Self::TrancheBalance,
				Self::CurrencyId,
			>,
			Error = DispatchError,
		>;

		/// The hook type which acts upon a finalized redemption collection.
		type CollectedForeignInvestmentHook: StatusNotificationHook<
			Id = (Self::AccountId, Self::InvestmentId),
			Status = ExecutedForeignCollect<
				Self::ForeignBalance,
				Self::TrancheBalance,
				Self::ForeignBalance,
				Self::CurrencyId,
			>,
			Error = DispatchError,
		>;

		/// The source of truth for pool currencies.
		type PoolInspect: PoolInspect<Self::AccountId, Self::CurrencyId>;
	}

	/// Contains the information about the foreign investment process
	///
	/// NOTE: The storage is killed once the investment is fully collected, or
	/// decreased.
	#[pallet::storage]
	pub(super) type ForeignInvestmentInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		entities::InvestmentInfo<T>,
	>;

	/// Contains the information about the foreign redemption process
	///
	/// NOTE: The storage is killed once the redemption is fully collected and
	/// fully swapped or decreased
	#[pallet::storage]
	pub(super) type ForeignRedemptionInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		entities::RedemptionInfo<T>,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to retrieve the `ForeignInvestInfo`.
		InfoNotFound,

		/// Failed to retrieve the pool for the given pool id.
		PoolNotFound,

		/// An action for a different foreign currency is currently in process
		/// for the same pool currency, account, and investment.
		/// The currenct foreign actions must be finished before starting with a
		/// different foreign currency investment / redemption.
		MismatchedForeignCurrency,

		/// The decrease is greater than the current investment/redemption
		TooMuchDecrease,
	}
}
