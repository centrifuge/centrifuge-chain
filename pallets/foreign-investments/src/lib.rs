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
pub use pallet::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Reflects the reason for the last token swap update such that it can be
/// updated accordingly if the last and current reason mismatch.
#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
pub enum TokenSwapReason {
	Investment,
	Redemption,
	InvestmentAndRedemption,
}

pub type SwapOf<T> = Swap<<T as Config>::Balance, <T as Config>::CurrencyId>;
pub type ForeignInvestmentInfoOf<T, Reason> = cfg_types::investments::ForeignInvestmentInfo<
	<T as frame_system::Config>::AccountId,
	<T as Config>::InvestmentId,
	Reason,
>;

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
	use sp_runtime::traits::{AtLeast32BitUnsigned, EnsureAdd, EnsureSub, One, Zero};
	use sp_std::cmp::Ordering;

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
		type CurrencyId: Parameter + Member + Copy + TypeInfo + MaxEncodedLen;

		/// The pool id type required for the investment identifier
		type PoolId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ core::fmt::Debug;

		/// The tranche id type required for the investment identifier
		type TrancheId: Member + Parameter + Default + Copy + MaxEncodedLen + TypeInfo;

		/// The investment identifying type required for the investment type
		type InvestmentId: TrancheCurrency<Self::PoolId, Self::TrancheId>
			+ Clone
			+ Member
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
		type DefaultTokenSellRatio: Get<Self::BalanceRatio>;

		/// The token swap order identifying type
		type SwapId: Parameter
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
			OrderId = Self::SwapId,
			OrderDetails = Swap<Self::Balance, Self::CurrencyId>,
			SellRatio = Self::BalanceRatio,
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
			Status = ExecutedForeignCollect<Self::Balance, Self::CurrencyId>,
			Error = DispatchError,
		>;

		/// The hook type which acts upon a finalized redemption collection.
		type CollectedForeignInvestmentHook: StatusNotificationHook<
			Id = cfg_types::investments::ForeignInvestmentInfo<
				Self::AccountId,
				Self::InvestmentId,
				(),
			>,
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

	/// Maps a token swap order id to the corresponding `ForeignInvestmentInfo`
	/// to implicitly enable mapping to `InvestmentState` and `RedemptionState`.
	///
	/// NOTE: The storage is immediately killed when the swap order is
	/// completely fulfilled even if the corresponding investment and/or
	/// redemption might not be fully processed.
	#[pallet::storage]
	#[pallet::getter(fn foreign_investment_info)]
	pub(super) type ForeignInvestmentInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::SwapId, ForeignInvestmentInfoOf<T, TokenSwapReason>>;

	/// Maps an investor and their `InvestmentId` to the corresponding
	/// `SwapId`.
	///
	/// NOTE: The storage is immediately killed when the swap order is
	/// completely fulfilled even if the investment might not be fully
	/// processed.
	#[pallet::storage]
	#[pallet::getter(fn token_swap_order_ids)]
	pub(super) type SwapIds<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		T::SwapId,
	>;

	/// Maps an investor and their `InvestmentId` to the collected investment
	/// amount, i.e., the payment amount of pool currency burned for the
	/// conversion into collected amount of tranche tokens based on the
	/// fulfillment price(s).
	///
	/// NOTE: The lifetime of this storage starts with receiving a notification
	/// of an executed investment via the `CollectedInvestmentHook`. It ends
	/// with transferring the collected tranche tokens by executing
	/// `notify_executed_collect_invest` which is part of
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

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ForeignInvestmentUpdated {
			investor: T::AccountId,
			investment_id: T::InvestmentId,
			order_id: T::SwapId,
		},
		ForeignInvestmentCleared {
			investor: T::AccountId,
			investment_id: T::InvestmentId,
		},
		ForeignRedemptionUpdated {
			investor: T::AccountId,
			investment_id: T::InvestmentId,
			state: T::SwapId,
		},
		ForeignRedemptionCleared {
			investor: T::AccountId,
			investment_id: T::InvestmentId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to retrieve the foreign payment currency for a collected
		/// investment.
		///
		/// NOTE: This error can only occur, if a user tries to collect before
		/// having increased their investment as this would store the payment
		/// currency.
		InvestmentPaymentCurrencyNotFound,
		/// Failed to retrieve the foreign payout currency for a collected
		/// redemption.
		///
		/// NOTE: This error can only occur, if a user tries to collect before
		/// having increased their redemption as this would store the payout
		/// currency.
		RedemptionPayoutCurrencyNotFound,
		/// Failed to retrieve the `TokenSwapReason` from the given
		/// `SwapId`.
		InvestmentInfoNotFound,
		/// Failed to retrieve the `TokenSwapReason` from the given
		/// `SwapId`.
		TokenSwapReasonNotFound,
		/// The fulfilled token swap amount exceeds the sum of active swap
		/// amounts of the corresponding `InvestmentState` and
		/// `RedemptionState`.
		FulfilledTokenSwapAmountOverflow,
		/// Failed to retrieve the pool for the given pool id.
		PoolNotFound,
		SwapOrderNotFound,
	}

	/// Internal type used as result of `Pallet::apply_swap()`
	struct ApplySwapStatus<Balance> {
		/// The amount pending to be swapped
		pending: Balance,

		/// The amount already swapped and available to use
		swapped: Balance,
	}

	impl<T: Config> Pallet<T> {
		/// Returns the `amount_out` of the swap, seeing as the amount used to
		/// swap.
		fn used_swap_amount(swap: SwapOf<T>) -> Result<T::Balance, DispatchError> {
			T::CurrencyConverter::stable_to_stable(
				swap.currency_out,
				swap.currency_in,
				swap.amount, /* in */
			)
		}

		/// Apply a swap over a current possible swap state.
		/// - If there was no previous swap, it adds it.
		/// - If there was a swap in the same direction, it increments it.
		/// - If there was a swap in the opposite direction:
		///   - If the amount is smaller, it decrements it.
		///   - If the amount is the same, it removes the inverse swap.
		///   - If the amount is greater, it removes the inverse swap and create
		///     another with the reminder
		///
		/// The returned status contains the swapped amount after this call and
		/// the pending amount to be swapped.
		fn apply_swap(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			new_swap: SwapOf<T>,
		) -> Result<ApplySwapStatus<T::Balance>, DispatchError> {
			match SwapIds::<T>::get(who, investment_id) {
				None => {
					let id = T::TokenSwaps::place_order(
						who.clone(),
						new_swap.currency_in,
						new_swap.currency_out,
						new_swap.amount, /* in */
						T::BalanceRatio::one(),
					)?;
					SwapIds::<T>::insert(who, investment_id, id);

					Ok(ApplySwapStatus {
						swapped: T::Balance::zero(),
						pending: new_swap.amount,
					})
				}
				Some(id) => {
					let swap = T::TokenSwaps::get_order_details(id)
						.ok_or(Error::<T>::SwapOrderNotFound)?;

					if swap.is_same_direction(&new_swap)? {
						T::TokenSwaps::update_order(
							who.clone(),
							id,
							swap.amount.ensure_add(new_swap.amount)?,
							T::BalanceRatio::one(),
						)?;

						Ok(ApplySwapStatus {
							swapped: T::Balance::zero(),
							pending: new_swap.amount,
						})
					} else {
						let inverse_swap = swap;

						let inverse_swap_amount_in = Self::used_swap_amount(inverse_swap)?;
						let new_swap_amount_out = Self::used_swap_amount(new_swap)?;

						match inverse_swap.amount.cmp(&new_swap_amount_out) {
							Ordering::Less => {
								T::TokenSwaps::update_order(
									who.clone(),
									id,
									swap.amount.ensure_sub(new_swap_amount_out)?,
									T::BalanceRatio::one(),
								)?;

								Ok(ApplySwapStatus {
									swapped: new_swap.amount, /* in */
									pending: T::Balance::zero(),
								})
							}
							Ordering::Equal => {
								T::TokenSwaps::cancel_order(id.clone())?;

								Ok(ApplySwapStatus {
									swapped: new_swap.amount, /* in */
									pending: T::Balance::zero(),
								})
							}
							Ordering::Greater => {
								T::TokenSwaps::cancel_order(id.clone())?;

								let amount_to_swap =
									new_swap.amount.ensure_sub(inverse_swap_amount_in)?;

								let id = T::TokenSwaps::place_order(
									who.clone(),
									new_swap.currency_in,
									new_swap.currency_out,
									amount_to_swap,
									T::BalanceRatio::one(),
								)?;
								SwapIds::<T>::insert(who, investment_id, id);

								Ok(ApplySwapStatus {
									swapped: inverse_swap_amount_in,
									pending: amount_to_swap,
								})
							}
						}
					}
				}
			}
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
			pool_currency: T::CurrencyId,
		) -> DispatchResult {
			let pool_amount = T::CurrencyConverter::stable_to_stable(
				pool_currency,
				foreign_currency,
				foreign_amount,
			)?;

			let amount = Self::apply_swap(
				who,
				investment_id,
				Swap {
                    currency_in: pool_currency,
                    currency_out: foreign_currency,
                    amount /*in*/: pool_amount,
                },
			)?;

			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_add(amount.swapped)?,
			)
		}

		fn decrease_foreign_investment(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			foreign_amount: T::Balance,
			foreign_currency: T::CurrencyId,
			pool_currency: T::CurrencyId,
		) -> DispatchResult {
			let amount = Self::apply_swap(
				who,
				investment_id,
				Swap {
					currency_in: foreign_currency,
					currency_out: pool_currency,
					amount /*in*/: foreign_amount,
				},
			)?;

			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_sub(amount.pending)?,
			)?;

			T::DecreasedForeignInvestOrderHook::notify_status_change(
				ForeignInvestmentInfoOf::<T, ()> {
					owner: who.clone(),
					id: investment_id,
					last_swap_reason: None,
				},
				ExecutedForeignDecreaseInvest {
					amount_decreased: amount.swapped,
					foreign_currency,
					amount_remaining: amount.pending,
				},
			)
		}

		fn increase_foreign_redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			tranche_tokens_amount: T::Balance,
			_payout_currency: T::CurrencyId,
		) -> DispatchResult {
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
			_payout_currency: T::CurrencyId,
		) -> Result<(T::Balance, T::Balance), DispatchError> {
			T::Investment::update_redemption(
				who,
				investment_id,
				T::Investment::redemption(who, investment_id)?.ensure_sub(tranche_tokens_amount)?,
			)?;

			let remaining_amount = T::Investment::redemption(who, investment_id)?;

			Ok((tranche_tokens_amount, remaining_amount))
		}

		fn collect_foreign_investment(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			_foreign_payment_currency: T::CurrencyId,
		) -> DispatchResult {
			T::Investment::collect_investment(who.clone(), investment_id)
		}

		fn collect_foreign_redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			_foreign_payout_currency: T::CurrencyId,
			_pool_currency: T::CurrencyId,
		) -> DispatchResult {
			T::Investment::collect_redemption(who.clone(), investment_id)
		}

		fn investment(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
		) -> Result<T::Balance, DispatchError> {
			T::Investment::investment(who, investment_id)
		}

		fn redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
		) -> Result<T::Balance, DispatchError> {
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
	impl<T: Config> StatusNotificationHook for Pallet<T> {
		type Error = DispatchError;
		type Id = T::SwapId;
		type Status = SwapOf<T>;

		fn notify_status_change(id: T::SwapId, status: SwapOf<T>) -> Result<(), DispatchError> {
			let info =
				ForeignInvestmentInfo::<T>::get(id).ok_or(Error::<T>::InvestmentInfoNotFound)?;

			let foreign_currency = todo!("get from somewhere");

			if status.currency_out == foreign_currency {
				T::Investment::update_investment(
					&info.owner,
					info.id,
					T::Investment::investment(&info.owner, info.id)?.ensure_add(status.amount)?,
				)
			} else {
				T::DecreasedForeignInvestOrderHook::notify_status_change(
					ForeignInvestmentInfoOf::<T, ()> {
						owner: info.owner.clone(),
						id: info.id,
						last_swap_reason: None,
					},
					ExecutedForeignDecreaseInvest {
						amount_decreased: status.amount,
						foreign_currency,
						amount_remaining: todo!("get from order-book somehow"),
					},
				)
			}
		}
	}

	pub struct CollectedInvestmentHook<T>(PhantomData<T>);
	impl<T: Config> StatusNotificationHook for CollectedInvestmentHook<T> {
		type Error = DispatchError;
		type Id = ForeignInvestmentInfoOf<T, ()>;
		type Status = CollectedAmount<T::Balance>;

		fn notify_status_change(
			id: ForeignInvestmentInfoOf<T, ()>,
			status: CollectedAmount<T::Balance>,
		) -> DispatchResult {
			T::CollectedForeignInvestmentHook::notify_status_change(todo!(), todo!())
		}
	}

	pub struct CollectedRedemptionHook<T>(PhantomData<T>);
	impl<T: Config> StatusNotificationHook for CollectedRedemptionHook<T> {
		type Error = DispatchError;
		type Id = ForeignInvestmentInfoOf<T, ()>;
		type Status = CollectedAmount<T::Balance>;

		fn notify_status_change(
			id: ForeignInvestmentInfoOf<T, ()>,
			status: CollectedAmount<T::Balance>,
		) -> DispatchResult {
			T::CollectedForeignRedemptionHook::notify_status_change(todo!(), todo!())
		}
	}
}
