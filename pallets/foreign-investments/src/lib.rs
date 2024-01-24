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

use cfg_traits::{
	investments::{Investment, TrancheCurrency},
	PoolInspect, TokenSwaps,
};
use cfg_types::investments::{CollectedAmount, Swap};
use frame_support::{dispatch::DispatchResult, ensure};
pub use pallet::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureAdd, EnsureAddAssign, EnsureSub, EnsureSubAssign, Saturating, Zero},
	ArithmeticError, DispatchError,
};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

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

/// Hold the base information of a foreign investment/redemption
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct BaseInfo<T: Config> {
	foreign_currency: T::CurrencyId,
	collected: CollectedAmount<T::Balance>,
}

impl<T: Config> BaseInfo<T> {
	fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			foreign_currency,
			collected: CollectedAmount::default(),
		})
	}

	fn ensure_same_foreign(&self, foreign_currency: T::CurrencyId) -> DispatchResult {
		ensure!(
			self.foreign_currency == foreign_currency,
			Error::<T>::MismatchedForeignCurrency
		);

		Ok(())
	}
}

/// Hold the information of a foreign investment
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InvestmentInfo<T: Config> {
	/// General info
	base: BaseInfo<T>,

	/// Amount of pool currency for increased for this investment
	total_pool_amount: T::Balance,

	/// Total swapped amount pending to execute for decreasing the investment.
	decrease_swapped_amount: T::Balance,

	/// Amount that has not been decremented from an investment as part of a
	/// decrease investment because such amount was already pending to be
	/// swapped in the opposite direction.
	pending_decrement_not_invested: T::Balance,
}

impl<T: Config> InvestmentInfo<T> {
	fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(foreign_currency)?,
			total_pool_amount: T::Balance::default(),
			decrease_swapped_amount: T::Balance::default(),
			pending_decrement_not_invested: T::Balance::default(),
		})
	}

	/// Increase an investment taking into account that a previous decrement
	/// could be pending
	fn increase_investment(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		pool_amount: T::Balance,
	) -> DispatchResult {
		let amount_to_invest = pool_amount.saturating_sub(self.pending_decrement_not_invested);

		self.pending_decrement_not_invested
			.ensure_sub_assign(pool_amount.ensure_sub(amount_to_invest)?)?;

		if !amount_to_invest.is_zero() {
			dbg!(amount_to_invest, self.pending_decrement_not_invested);
			T::Investment::update_investment(
				&who,
				investment_id,
				T::Investment::investment(&who, investment_id)?.ensure_add(amount_to_invest)?,
			)?;
		}

		Ok(())
	}

	/// Decrease an investment taking into account that a previous increment
	/// could be pending
	fn decrease_investment(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		pool_amount: T::Balance,
	) -> DispatchResult {
		let pool_currency = pool_currency_of::<T>(investment_id)?;
		let pending_pool_amount_increment =
			ForeignIdToSwapId::<T>::get((who, investment_id, Action::Investment))
				.map(|swap_id| T::TokenSwaps::get_order_details(swap_id))
				.flatten()
				.filter(|swap| swap.currency_in == pool_currency)
				.map(|swap| swap.amount_in)
				.unwrap_or(T::Balance::default());

		let decrement = pool_amount.saturating_sub(pending_pool_amount_increment);
		if !decrement.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_sub(decrement)?,
			)?;
		}

		self.pending_decrement_not_invested
			.ensure_add_assign(pool_amount.ensure_sub(decrement)?)?;

		Ok(())
	}

	fn remaining_pool_amount(&self) -> Result<T::Balance, ArithmeticError> {
		self.total_pool_amount
			.ensure_sub(self.base.collected.amount_payment)
	}
}

/// Hold the information of an foreign redemption
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RedemptionInfo<T: Config> {
	/// General info
	base: BaseInfo<T>,

	/// Amount of tranche tokens pending to redeem
	pending_tranche_tokens: T::Balance,

	/// Total swapped amount pending to execute.
	swapped_amount: T::Balance,
}

impl<T: Config> RedemptionInfo<T> {
	fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(foreign_currency)?,
			pending_tranche_tokens: T::Balance::default(),
			swapped_amount: T::Balance::default(),
		})
	}

	fn collected_tranche_tokens(&self) -> T::Balance {
		self.base.collected.amount_payment
	}
}

pub type SwapOf<T> = Swap<<T as Config>::Balance, <T as Config>::CurrencyId>;

fn pool_currency_of<T: Config>(
	investment_id: T::InvestmentId,
) -> Result<T::CurrencyId, DispatchError> {
	T::PoolInspect::currency_for(investment_id.of_pool()).ok_or(Error::<T>::PoolNotFound.into())
}

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		investments::{ForeignInvestment, Investment, InvestmentCollector, TrancheCurrency},
		IdentityCurrencyConversion, PoolInspect, StatusNotificationHook, TokenSwaps,
	};
	use cfg_types::investments::{
		CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest,
	};
	use frame_support::{dispatch::HasCompact, pallet_prelude::*};
	use sp_runtime::{
		traits::{
			AtLeast32BitUnsigned, EnsureAdd, EnsureAddAssign, EnsureSub, EnsureSubAssign, One, Zero,
		},
		FixedPointOperand,
	};
	use sp_std::cmp::Ordering;

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it
	/// depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Type representing the weight of this pallet
		type WeightInfo: frame_system::WeightInfo;

		/// The source of truth for the balance of accounts
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ FixedPointOperand
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The currency type of transferrable tokens
		type CurrencyId: Parameter + Member + Copy + MaxEncodedLen;

		/// The pool id type required for the investment identifier
		type PoolId: Member + Parameter + Copy + HasCompact + MaxEncodedLen + core::fmt::Debug;

		/// The tranche id type required for the investment identifier
		type TrancheId: Member + Parameter + Default + Copy + MaxEncodedLen;

		/// The investment identifying type required for the investment type
		type InvestmentId: TrancheCurrency<Self::PoolId, Self::TrancheId>
			+ Parameter
			+ Copy
			+ MaxEncodedLen;

		/// The internal investment type which handles the actual investment on
		/// top of the wrapper implementation of this Pallet
		type Investment: Investment<
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
		type BalanceRatio: Parameter
			+ Member
			+ sp_runtime::FixedPointNumber
			+ sp_runtime::traits::EnsureMul
			+ sp_runtime::traits::EnsureDiv
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The token swap order identifying type
		type SwapId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord + MaxEncodedLen;

		/// The type which exposes token swap order functionality such as
		/// placing and cancelling orders
		type TokenSwaps: TokenSwaps<
			Self::AccountId,
			CurrencyId = Self::CurrencyId,
			Balance = Self::Balance,
			OrderId = Self::SwapId,
			OrderDetails = Swap<Self::Balance, Self::CurrencyId>,
			SellRatio = Self::BalanceRatio,
		>;

		/// The hook type which acts upon a finalized investment decrement.
		type DecreasedForeignInvestOrderHook: StatusNotificationHook<
			Id = (Self::AccountId, Self::InvestmentId),
			Status = ExecutedForeignDecreaseInvest<Self::Balance, Self::CurrencyId>,
			Error = DispatchError,
		>;

		/// The hook type which acts upon a finalized redemption collection.
		type CollectedForeignRedemptionHook: StatusNotificationHook<
			Id = (Self::AccountId, Self::InvestmentId),
			Status = ExecutedForeignCollect<Self::Balance, Self::CurrencyId>,
			Error = DispatchError,
		>;

		/// The hook type which acts upon a finalized redemption collection.
		type CollectedForeignInvestmentHook: StatusNotificationHook<
			Id = (Self::AccountId, Self::InvestmentId),
			Status = ExecutedForeignCollect<Self::Balance, Self::CurrencyId>,
			Error = DispatchError,
		>;

		/// Type which provides a conversion from one currency amount to another
		/// currency amount.
		///
		/// NOTE: Restricting to `IdentityCurrencyConversion` is solely a
		/// short-term MVP solution. In the near future, this type must be
		/// restricted to a more sophisticated trait which provides
		/// unidirectional conversions based on an oracle, dynamic prices or at
		/// least conversion ratios based on specific currency pairs.
		type CurrencyConverter: IdentityCurrencyConversion<
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

	/// Contains the information about the foreign investment process
	///
	/// NOTE: The storage is killed once the investment is fully collected.
	#[pallet::storage]
	pub(super) type ForeignInvestmentInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		InvestmentInfo<T>,
	>;

	/// Contains the information about the foreign redemption process
	///
	/// NOTE: The storage is killed once the redemption is fully collected and
	/// fully swapped
	#[pallet::storage]
	pub(super) type ForeignRedemptionInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		RedemptionInfo<T>,
	>;

	/// Maps a `SwapId` to its corresponding `ForeignId`
	///
	/// NOTE: The storage is killed when the swap order no longer exists
	#[pallet::storage]
	pub(super) type SwapIdToForeignId<T: Config> =
		StorageMap<_, Blake2_128Concat, T::SwapId, ForeignId<T>>;

	/// Maps a `ForeignId` to its corresponding `SwapId`
	///
	/// NOTE: The storage is killed when the swap order no longer exists
	#[pallet::storage]
	pub(super) type ForeignIdToSwapId<T: Config> =
		StorageMap<_, Blake2_128Concat, ForeignId<T>, T::SwapId>;

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to retrieve the `ForeignInvestInfo`.
		InfoNotFound,
		/// Failed to retrieve the swap order.
		SwapOrderNotFound,
		/// Failed to retrieve the pool for the given pool id.
		PoolNotFound,
		/// An action for a different foreign currency is currently in process
		/// for the same pool currency, account, and investment.
		/// The currenct foreign actions must be finished before starting with a
		/// different foreign currency investment / redemption.
		MismatchedForeignCurrency,
	}

	/// Internal type used as result of `Pallet::apply_swap()`
	/// Amounts are donominated referenced by the `new_swap` paramenter given to
	/// `apply_swap()`
	#[derive(Debug, PartialEq)]
	pub(crate) struct SwapStatus<T: Config> {
		/// The amount (in) already swapped and available to use.
		pub swapped: T::Balance,

		/// The amount (in) pending to be swapped
		pub pending: T::Balance,

		/// The amount (out) swapped by the inverse order
		pub swapped_inverse: T::Balance,

		/// The amount (out) pending to be swapped by the inverse order
		pub pending_inverse: T::Balance,

		/// The swap id for a possible reminder swap order after `apply_swap()`
		pub swap_id: Option<T::SwapId>,
	}

	impl<T: Config> Pallet<T> {
		/// Inserts, updates or removes a swap id associated to a foreign
		/// action.
		fn update_swap_id(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			action: Action,
			new_swap_id: Option<T::SwapId>,
		) -> DispatchResult {
			let previous_swap_id = ForeignIdToSwapId::<T>::get((who, investment_id, action));

			if previous_swap_id != new_swap_id {
				if let Some(new_id) = new_swap_id {
					SwapIdToForeignId::<T>::insert(new_id, (who.clone(), investment_id, action));
					ForeignIdToSwapId::<T>::insert((who.clone(), investment_id, action), new_id);
				}

				if let Some(old_id) = previous_swap_id {
					SwapIdToForeignId::<T>::remove(old_id);
					ForeignIdToSwapId::<T>::remove((who.clone(), investment_id, action));
				}
			}

			Ok(())
		}

		/// Apply a swap over a current possible swap state.
		/// - If there was no previous swap, it adds it.
		/// - If there was a swap in the same direction, it increments it.
		/// - If there was a swap in the opposite direction:
		///   - If the amount is smaller, it decrements it.
		///   - If the amount is the same, it removes the inverse swap.
		///   - If the amount is greater, it removes the inverse swap and create
		///     another with the excess
		///
		/// The returned status contains the swapped amounts after this call and
		/// the pending amounts to be swapped of both swap directions.
		pub(crate) fn apply_swap(
			who: &T::AccountId,
			new_swap: SwapOf<T>,
			over_swap_id: Option<T::SwapId>,
		) -> Result<SwapStatus<T>, DispatchError> {
			match over_swap_id {
				None => {
					let swap_id = T::TokenSwaps::place_order(
						who.clone(),
						new_swap.currency_in,
						new_swap.currency_out,
						new_swap.amount_in,
						T::BalanceRatio::one(),
					)?;

					Ok(SwapStatus {
						swapped: T::Balance::zero(),
						pending: new_swap.amount_in,
						swapped_inverse: T::Balance::zero(),
						pending_inverse: T::Balance::zero(),
						swap_id: Some(swap_id),
					})
				}
				Some(swap_id) => {
					let swap = T::TokenSwaps::get_order_details(swap_id)
						.ok_or(Error::<T>::SwapOrderNotFound)?;

					if swap.is_same_direction(&new_swap)? {
						let amount_to_swap = swap.amount_in.ensure_add(new_swap.amount_in)?;
						T::TokenSwaps::update_order(
							who.clone(),
							swap_id,
							amount_to_swap,
							T::BalanceRatio::one(),
						)?;

						Ok(SwapStatus {
							swapped: T::Balance::zero(),
							pending: amount_to_swap,
							swapped_inverse: T::Balance::zero(),
							pending_inverse: T::Balance::zero(),
							swap_id: Some(swap_id),
						})
					} else {
						let inverse_swap = swap;

						let new_swap_amount_out = T::CurrencyConverter::stable_to_stable(
							new_swap.currency_out,
							new_swap.currency_in,
							new_swap.amount_in,
						)?;

						match inverse_swap.amount_in.cmp(&new_swap_amount_out) {
							Ordering::Greater => {
								let amount_to_swap =
									inverse_swap.amount_in.ensure_sub(new_swap_amount_out)?;

								T::TokenSwaps::update_order(
									who.clone(),
									swap_id,
									amount_to_swap,
									T::BalanceRatio::one(),
								)?;

								Ok(SwapStatus {
									swapped: new_swap.amount_in,
									pending: T::Balance::zero(),
									swapped_inverse: new_swap_amount_out,
									pending_inverse: amount_to_swap,
									swap_id: Some(swap_id),
								})
							}
							Ordering::Equal => {
								T::TokenSwaps::cancel_order(swap_id)?;

								Ok(SwapStatus {
									swapped: new_swap.amount_in,
									pending: T::Balance::zero(),
									swapped_inverse: inverse_swap.amount_in,
									pending_inverse: T::Balance::zero(),
									swap_id: None,
								})
							}
							Ordering::Less => {
								T::TokenSwaps::cancel_order(swap_id)?;

								let inverse_swap_amount_out =
									T::CurrencyConverter::stable_to_stable(
										inverse_swap.currency_out,
										inverse_swap.currency_in,
										inverse_swap.amount_in,
									)?;

								let amount_to_swap =
									new_swap.amount_in.ensure_sub(inverse_swap_amount_out)?;

								let swap_id = T::TokenSwaps::place_order(
									who.clone(),
									new_swap.currency_in,
									new_swap.currency_out,
									amount_to_swap,
									T::BalanceRatio::one(),
								)?;

								Ok(SwapStatus {
									swapped: inverse_swap_amount_out,
									pending: amount_to_swap,
									swapped_inverse: inverse_swap.amount_in,
									pending_inverse: T::Balance::zero(),
									swap_id: Some(swap_id),
								})
							}
						}
					}
				}
			}
		}

		/// A wrap over `apply_swap()` that takes care of updating the swap id
		/// and notify
		fn apply_swap_and_notify(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			action: Action,
			new_swap: SwapOf<T>,
		) -> DispatchResult {
			let swap_id = ForeignIdToSwapId::<T>::get((who, investment_id, action));

			let status = Self::apply_swap(who, new_swap.clone(), swap_id)?;

			Self::update_swap_id(who, investment_id, action, status.swap_id)?;

			if !status.swapped_inverse.is_zero() {
				Self::notify_swap_done(
					who,
					investment_id,
					action,
					new_swap.currency_in,
					status.swapped_inverse,
					status.pending_inverse,
				)?;
			}

			if !status.swapped.is_zero() {
				Self::notify_swap_done(
					who,
					investment_id,
					action,
					new_swap.currency_out,
					status.swapped,
					status.pending,
				)?;
			}

			Ok(())
		}

		/// Notifies that a partial swap has been done and applies the result to
		/// an `InvestmentInfo` or `RedemptionInfo`
		fn notify_swap_done(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			action: Action,
			currency_out: T::CurrencyId,
			swapped_amount: T::Balance,
			pending_amount: T::Balance,
		) -> DispatchResult {
			match action {
				Action::Investment => Pallet::<T>::notify_investment_swap_done(
					&who,
					investment_id,
					currency_out,
					swapped_amount,
					pending_amount,
				),
				Action::Redemption => Pallet::<T>::notify_redemption_swap_done(
					&who,
					investment_id,
					swapped_amount,
					pending_amount,
				),
			}
		}

		/// Notifies that a partial swap has been done and applies the result to
		/// an `InvestmentInfo`
		fn notify_investment_swap_done(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			currency_out: T::CurrencyId,
			swapped_amount: T::Balance,
			pending_amount: T::Balance,
		) -> DispatchResult {
			ForeignInvestmentInfo::<T>::mutate_exists(&who, investment_id, |maybe_info| {
				let info = maybe_info.as_mut().ok_or(Error::<T>::InfoNotFound)?;

				if currency_out == info.base.foreign_currency {
					info.increase_investment(who, investment_id, swapped_amount)?;
				} else {
					info.decrease_swapped_amount
						.ensure_add_assign(swapped_amount)?;

					if pending_amount.is_zero() {
						// NOTE: How make this works with market ratios?
						let remaining_foreign_amount = T::CurrencyConverter::stable_to_stable(
							info.base.foreign_currency,
							pool_currency_of::<T>(investment_id)?,
							T::Investment::investment(&who, investment_id)?,
						)?;

						T::DecreasedForeignInvestOrderHook::notify_status_change(
							(who.clone(), investment_id),
							ExecutedForeignDecreaseInvest {
								amount_decreased: info.decrease_swapped_amount,
								foreign_currency: info.base.foreign_currency,
								amount_remaining: remaining_foreign_amount,
							},
						)?;

						info.decrease_swapped_amount = T::Balance::default();

						if info.remaining_pool_amount()?.is_zero() {
							*maybe_info = None;
						}
					}
				}

				Ok(())
			})
		}

		/// Notifies that a partial swap has been done and applies the result to
		/// an `RedemptionInfo`
		fn notify_redemption_swap_done(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			swapped_amount: T::Balance,
			pending_amount: T::Balance,
		) -> DispatchResult {
			ForeignRedemptionInfo::<T>::mutate_exists(&who, investment_id, |maybe_info| {
				let info = maybe_info.as_mut().ok_or(Error::<T>::InfoNotFound)?;

				info.swapped_amount.ensure_add_assign(swapped_amount)?;
				if pending_amount.is_zero() {
					let redemption = T::Investment::redemption(&who, investment_id)?;

					T::CollectedForeignRedemptionHook::notify_status_change(
						(who.clone(), investment_id),
						ExecutedForeignCollect {
							currency: info.base.foreign_currency,
							amount_currency_payout: info.swapped_amount,
							amount_tranche_tokens_payout: info.collected_tranche_tokens(),
							amount_remaining: redemption,
						},
					)?;

					info.pending_tranche_tokens
						.ensure_sub_assign(info.collected_tranche_tokens())?;

					info.base.collected = CollectedAmount::default();
					info.swapped_amount = T::Balance::default();

					if info.pending_tranche_tokens.is_zero() {
						*maybe_info = None;
					}
				}

				Ok(())
			})
		}
	}

	impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
		type Amount = T::Balance;
		type CurrencyId = T::CurrencyId;
		type Error = DispatchError;
		type InvestmentId = T::InvestmentId;

		fn increase_foreign_investment(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			foreign_amount: T::Balance,
			foreign_currency: T::CurrencyId,
		) -> DispatchResult {
			// NOTE: This line will be removed with market ratios
			let pool_amount = T::CurrencyConverter::stable_to_stable(
				pool_currency_of::<T>(investment_id)?,
				foreign_currency,
				foreign_amount,
			)?;

			ForeignInvestmentInfo::<T>::mutate(&who, investment_id, |info| {
				let info = info.get_or_insert(InvestmentInfo::new(foreign_currency)?);

				info.base.ensure_same_foreign(foreign_currency)?;
				info.total_pool_amount.ensure_add_assign(pool_amount)?;

				Ok::<_, DispatchError>(())
			})?;

			Self::apply_swap_and_notify(
				who,
				investment_id,
				Action::Investment,
				Swap {
					currency_in: pool_currency_of::<T>(investment_id)?,
					currency_out: foreign_currency,
					amount_in: pool_amount,
				},
			)
		}

		fn decrease_foreign_investment(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			foreign_amount: T::Balance,
			foreign_currency: T::CurrencyId,
		) -> DispatchResult {
			// NOTE: This line will be removed with market ratios
			let pool_amount = T::CurrencyConverter::stable_to_stable(
				pool_currency_of::<T>(investment_id)?,
				foreign_currency,
				foreign_amount,
			)?;

			ForeignInvestmentInfo::<T>::mutate(&who, investment_id, |info| {
				let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;

				info.base.ensure_same_foreign(foreign_currency)?;
				info.total_pool_amount.ensure_sub_assign(pool_amount)?;
				info.decrease_investment(who, investment_id, pool_amount)?;

				Ok::<_, DispatchError>(())
			})?;

			Self::apply_swap_and_notify(
				who,
				investment_id,
				Action::Investment,
				Swap {
					currency_in: foreign_currency,
					currency_out: pool_currency_of::<T>(investment_id)?,
					amount_in: foreign_amount,
				},
			)
		}

		fn increase_foreign_redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			tranche_tokens_amount: T::Balance,
			payout_foreign_currency: T::CurrencyId,
		) -> DispatchResult {
			ForeignRedemptionInfo::<T>::mutate(who, investment_id, |info| -> DispatchResult {
				let info = info.get_or_insert(RedemptionInfo::new(payout_foreign_currency)?);

				info.base.ensure_same_foreign(payout_foreign_currency)?;
				info.pending_tranche_tokens
					.ensure_add_assign(tranche_tokens_amount)?;

				Ok(())
			})?;

			T::Investment::update_redemption(
				who,
				investment_id,
				T::Investment::redemption(who, investment_id)?.ensure_add(tranche_tokens_amount)?,
			)
		}

		fn decrease_foreign_redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			tranche_tokens_amount: T::Balance,
			payout_foreign_currency: T::CurrencyId,
		) -> DispatchResult {
			ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
				let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;

				info.base.ensure_same_foreign(payout_foreign_currency)?;
				info.pending_tranche_tokens
					.ensure_sub_assign(tranche_tokens_amount)?;

				Ok::<_, DispatchError>(())
			})?;

			T::Investment::update_redemption(
				who,
				investment_id,
				T::Investment::redemption(who, investment_id)?.ensure_sub(tranche_tokens_amount)?,
			)
		}

		fn collect_foreign_investment(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			payment_foreign_currency: T::CurrencyId,
		) -> DispatchResult {
			ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
				let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
				info.base.ensure_same_foreign(payment_foreign_currency)?;
				Ok::<_, DispatchError>(())
			})?;

			T::Investment::collect_investment(who.clone(), investment_id)
		}

		fn collect_foreign_redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			payout_foreign_currency: T::CurrencyId,
			_pool_currency: T::CurrencyId,
		) -> DispatchResult {
			ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
				let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
				info.base.ensure_same_foreign(payout_foreign_currency)?;
				Ok::<_, DispatchError>(())
			})?;

			T::Investment::collect_redemption(who.clone(), investment_id)
		}

		fn investment(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
		) -> Result<T::Balance, DispatchError> {
			ensure!(
				ForeignInvestmentInfo::<T>::contains_key(&who, investment_id),
				Error::<T>::InfoNotFound
			);

			T::Investment::investment(who, investment_id)
		}

		fn redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
		) -> Result<T::Balance, DispatchError> {
			ensure!(
				ForeignRedemptionInfo::<T>::contains_key(&who, investment_id),
				Error::<T>::InfoNotFound
			);

			T::Investment::redemption(who, investment_id)
		}

		fn accepted_payment_currency(
			investment_id: T::InvestmentId,
			currency: T::CurrencyId,
		) -> bool {
			if T::Investment::accepted_payment_currency(investment_id, currency) {
				true
			} else {
				T::PoolInspect::currency_for(investment_id.of_pool())
					.map(|pool_currency| T::TokenSwaps::valid_pair(pool_currency, currency))
					.unwrap_or(false)
			}
		}

		fn accepted_payout_currency(
			investment_id: T::InvestmentId,
			currency: T::CurrencyId,
		) -> bool {
			if T::Investment::accepted_payout_currency(investment_id, currency) {
				true
			} else {
				T::PoolInspect::currency_for(investment_id.of_pool())
					.map(|pool_currency| T::TokenSwaps::valid_pair(currency, pool_currency))
					.unwrap_or(false)
			}
		}
	}

	pub struct FulfilledSwapOrderHook<T>(PhantomData<T>);
	impl<T: Config> StatusNotificationHook for FulfilledSwapOrderHook<T> {
		type Error = DispatchError;
		type Id = T::SwapId;
		type Status = SwapOf<T>;

		fn notify_status_change(
			swap_id: T::SwapId,
			last_swap: SwapOf<T>,
		) -> Result<(), DispatchError> {
			let (who, investment_id, action) =
				SwapIdToForeignId::<T>::get(swap_id).ok_or(Error::<T>::SwapOrderNotFound)?;

			let pending_amount = match T::TokenSwaps::get_order_details(swap_id) {
				Some(swap) => swap.amount_in,
				None => {
					Pallet::<T>::update_swap_id(&who, investment_id, action, None)?;
					T::Balance::default()
				}
			};

			Pallet::<T>::notify_swap_done(
				&who,
				investment_id,
				action,
				last_swap.currency_out,
				last_swap.amount_in,
				pending_amount,
			)
		}
	}

	pub struct CollectedInvestmentHook<T>(PhantomData<T>);
	impl<T: Config> StatusNotificationHook for CollectedInvestmentHook<T> {
		type Error = DispatchError;
		type Id = (T::AccountId, T::InvestmentId);
		type Status = CollectedAmount<T::Balance>;

		fn notify_status_change(
			(who, investment_id): (T::AccountId, T::InvestmentId),
			collected: CollectedAmount<T::Balance>,
		) -> DispatchResult {
			ForeignInvestmentInfo::<T>::mutate_exists(&who, investment_id, |maybe_info| {
				let info = maybe_info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
				info.base.collected.increase(&collected)?;

				// NOTE: How make this works with market ratios?
				let remaining_foreign_amount = T::CurrencyConverter::stable_to_stable(
					info.base.foreign_currency,
					pool_currency_of::<T>(investment_id)?,
					T::Investment::investment(&who, investment_id)?,
				)?;

				// NOTE: How make this works with market ratios?
				let collected_foreign_amount = T::CurrencyConverter::stable_to_stable(
					info.base.foreign_currency,
					pool_currency_of::<T>(investment_id)?,
					collected.amount_payment,
				)?;

				T::CollectedForeignInvestmentHook::notify_status_change(
					(who.clone(), investment_id),
					ExecutedForeignCollect {
						currency: info.base.foreign_currency,
						amount_currency_payout: collected_foreign_amount,
						amount_tranche_tokens_payout: collected.amount_collected,
						amount_remaining: remaining_foreign_amount,
					},
				)?;

				if info.remaining_pool_amount()?.is_zero() {
					*maybe_info = None;
				}

				Ok(())
			})
		}
	}

	pub struct CollectedRedemptionHook<T>(PhantomData<T>);
	impl<T: Config> StatusNotificationHook for CollectedRedemptionHook<T> {
		type Error = DispatchError;
		type Id = (T::AccountId, T::InvestmentId);
		type Status = CollectedAmount<T::Balance>;

		fn notify_status_change(
			(who, investment_id): (T::AccountId, T::InvestmentId),
			collected: CollectedAmount<T::Balance>,
		) -> DispatchResult {
			let info = ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
				let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
				info.base.collected.increase(&collected)?;
				Ok::<_, DispatchError>(info.clone())
			})?;

			Pallet::<T>::apply_swap_and_notify(
				&who,
				investment_id,
				Action::Redemption,
				Swap {
					currency_in: info.base.foreign_currency,
					currency_out: pool_currency_of::<T>(investment_id)?,
					amount_in: collected.amount_collected,
				},
			)
		}
	}
}
