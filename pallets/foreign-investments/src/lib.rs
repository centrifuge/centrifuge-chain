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

/// Hold the information of an foreign investment
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Info<T: Config> {
	foreign_currency: T::CurrencyId,
	pool_currency: T::CurrencyId,
}

/// Specify the type of foreign investment
#[derive(
	Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
enum ForeignSwapKind {
	Investment,
	Redemption,
}

pub type SwapOf<T> = Swap<<T as Config>::Balance, <T as Config>::CurrencyId>;

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
	/// NOTE: The storage is killed once the investment is collected or
	/// redemption process is collected and fully swapped
	#[pallet::storage]
	pub(super) type ForeignInvestmentInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		Info<T>,
	>;

	/// Contains the information about the foreign investment process
	///
	/// NOTE: The storage is killed once the investment is collected or
	/// redemption process is collected and fully swapped
	#[pallet::storage]
	pub(super) type ForeignRedemptionInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		Info<T>,
	>;

	/// Maps an account and their `InvestmentId` with the associated foreign
	/// swap kind to the corresponding `SwapId`.
	///
	/// NOTE: The storage is killed when the swap order no longer exists
	#[pallet::storage]
	pub(super) type ForeignSwapIdToSwapId<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		(T::InvestmentId, ForeignSwapKind),
		T::SwapId,
	>;

	/// Maps a `SwapId` to their corresponding foreing swap identification by
	/// `AccountId`, `InvestmentId` and `ForeignSwapKind`
	///
	/// NOTE: The storage is killed when the swap order no longer exists
	#[pallet::storage]
	pub(super) type SwapidToForeignSwapId<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::SwapId,
		(T::AccountId, T::InvestmentId, ForeignSwapKind),
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
		/// Failed to retrieve the `ForeignInvestInfo`.
		InfoNotFound,
		/// Failed to retrieve the swap order.
		SwapOrderNotFound,
		/// Failed to retrieve the pool for the given pool id.
		PoolNotFound,
	}

	/// Internal type used as result of `Pallet::apply_swap()`
	struct SwapStatus<T: Config> {
		/// The amount pending to be swapped
		pending: T::Balance,

		/// The amount already swapped and available to use
		swapped: T::Balance,
	}

	impl<T: Config> Pallet<T> {
		fn register_swap(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			kind: ForeignSwapKind,
			swap_id: T::SwapId,
		) {
			ForeignSwapIdToSwapId::<T>::insert(&who, (investment_id, kind), swap_id);
			SwapidToForeignSwapId::<T>::insert(swap_id, (who.clone(), investment_id, kind));
		}

		fn unregister_swap(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			kind: ForeignSwapKind,
			swap_id: T::SwapId,
		) {
			ForeignSwapIdToSwapId::<T>::remove(&who, (investment_id, kind));
			SwapidToForeignSwapId::<T>::remove(swap_id);
		}

		/// Returns the `amount_out` of the swap, seeing as the amount used to
		/// swap.
		fn amount_used_to_swap(swap: &SwapOf<T>) -> Result<T::Balance, DispatchError> {
			T::CurrencyConverter::stable_to_stable(
				swap.currency_out,
				swap.currency_in,
				swap.amount_in,
			)
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
		/// The returned status contains the swapped amount after this call and
		/// the pending amount to be swapped.
		fn apply_swap(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			kind: ForeignSwapKind,
			new_swap: SwapOf<T>,
		) -> Result<SwapStatus<T>, DispatchError> {
			match ForeignSwapIdToSwapId::<T>::get(&who, (investment_id, kind)) {
				None => {
					let swap_id = T::TokenSwaps::place_order(
						who.clone(),
						new_swap.currency_in,
						new_swap.currency_out,
						new_swap.amount_in,
						T::BalanceRatio::one(),
					)?;
					Self::register_swap(&who, investment_id, kind, swap_id);

					Ok(SwapStatus {
						swapped: T::Balance::zero(),
						pending: new_swap.amount_in,
					})
				}
				Some(swap_id) => {
					let swap = T::TokenSwaps::get_order_details(swap_id)
						.ok_or(Error::<T>::SwapOrderNotFound)?;

					if swap.is_same_direction(&new_swap)? {
						T::TokenSwaps::update_order(
							who.clone(),
							swap_id,
							swap.amount_in.ensure_add(new_swap.amount_in)?,
							T::BalanceRatio::one(),
						)?;

						Ok(SwapStatus {
							swapped: T::Balance::zero(),
							pending: new_swap.amount_in,
						})
					} else {
						let inverse_swap = swap;
						let new_swap_amount_out = Self::amount_used_to_swap(&new_swap)?;

						match inverse_swap.amount_in.cmp(&new_swap_amount_out) {
							Ordering::Less => {
								T::TokenSwaps::update_order(
									who.clone(),
									swap_id,
									inverse_swap.amount_in.ensure_sub(new_swap_amount_out)?,
									T::BalanceRatio::one(),
								)?;

								Ok(SwapStatus {
									swapped: new_swap.amount_in,
									pending: T::Balance::zero(),
								})
							}
							Ordering::Equal => {
								T::TokenSwaps::cancel_order(swap_id.clone())?;
								Self::unregister_swap(&who, investment_id, kind, swap_id);

								Ok(SwapStatus {
									swapped: new_swap.amount_in,
									pending: T::Balance::zero(),
								})
							}
							Ordering::Greater => {
								T::TokenSwaps::cancel_order(swap_id.clone())?;
								Self::unregister_swap(&who, investment_id, kind, swap_id);

								let inverse_swap_amount_out =
									Self::amount_used_to_swap(&inverse_swap)?;

								let amount_to_swap =
									new_swap.amount_in.ensure_sub(inverse_swap_amount_out)?;

								let swap_id = T::TokenSwaps::place_order(
									who.clone(),
									new_swap.currency_in,
									new_swap.currency_out,
									amount_to_swap,
									T::BalanceRatio::one(),
								)?;
								Self::register_swap(&who, investment_id, kind, swap_id);

								Ok(SwapStatus {
									swapped: inverse_swap_amount_out,
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
			ForeignInvestmentInfo::<T>::insert(
				who,
				investment_id,
				Info {
					foreign_currency,
					pool_currency,
				},
			);

			let pool_amount = T::CurrencyConverter::stable_to_stable(
				pool_currency,
				foreign_currency,
				foreign_amount,
			)?;

			let status = Self::apply_swap(
				who,
				investment_id,
				ForeignSwapKind::Investment,
				Swap {
					currency_in: pool_currency,
					currency_out: foreign_currency,
					amount_in: pool_amount,
				},
			)?;

			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_add(status.swapped)?,
			)
		}

		fn decrease_foreign_investment(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			foreign_amount: T::Balance,
			foreign_currency: T::CurrencyId,
			pool_currency: T::CurrencyId,
		) -> DispatchResult {
			let status = Self::apply_swap(
				who,
				investment_id,
				ForeignSwapKind::Investment,
				Swap {
					currency_in: foreign_currency,
					currency_out: pool_currency,
					amount_in: foreign_amount,
				},
			)?;

			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_sub(status.pending)?,
			)?;

			T::DecreasedForeignInvestOrderHook::notify_status_change(
				(who.clone(), investment_id),
				ExecutedForeignDecreaseInvest {
					amount_decreased: status.swapped,
					foreign_currency,
					amount_remaining: status.pending,
				},
			)
		}

		fn increase_foreign_redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			tranche_tokens_amount: T::Balance,
			payout_foreign_currency: T::CurrencyId,
		) -> DispatchResult {
			ForeignRedemptionInfo::<T>::insert(
				who,
				investment_id,
				Info {
					foreign_currency: payout_foreign_currency,
					pool_currency: T::PoolInspect::currency_for(investment_id.of_pool())
						.ok_or(Error::<T>::PoolNotFound)?,
				},
			);

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
			_payout_foreign_currency: T::CurrencyId,
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

		fn notify_status_change(
			swap_id: T::SwapId,
			last_swap: SwapOf<T>,
		) -> Result<(), DispatchError> {
			let (who, investment_id, kind) =
				SwapidToForeignSwapId::<T>::get(swap_id).ok_or(Error::<T>::SwapOrderNotFound)?;

			let amount_remaining = match T::TokenSwaps::get_order_details(swap_id) {
				Some(swap) => swap.amount_in,
				None => {
					Self::unregister_swap(&who, investment_id, kind, swap_id);
					T::Balance::zero()
				}
			};

			match kind {
				ForeignSwapKind::Investment => {
					let info = ForeignInvestmentInfo::<T>::get(&who, investment_id)
						.ok_or(Error::<T>::InfoNotFound)?;

					if last_swap.currency_out == info.foreign_currency {
						T::Investment::update_investment(
							&who,
							investment_id,
							T::Investment::investment(&who, investment_id)?
								.ensure_add(last_swap.amount_in)?,
						)
					} else {
						T::DecreasedForeignInvestOrderHook::notify_status_change(
							(who.clone(), investment_id),
							ExecutedForeignDecreaseInvest {
								amount_decreased: last_swap.amount_in,
								foreign_currency: info.foreign_currency,
								amount_remaining,
							},
						)
					}
				}
				ForeignSwapKind::Redemption => {
					let info = ForeignRedemptionInfo::<T>::get(&who, investment_id)
						.ok_or(Error::<T>::InfoNotFound)?;

					if amount_remaining.is_zero() {
						ForeignRedemptionInfo::<T>::remove(&who, investment_id);
					}

					T::CollectedForeignRedemptionHook::notify_status_change(
						(who.clone(), investment_id),
						ExecutedForeignCollect {
							currency: info.foreign_currency,
							amount_currency_payout: last_swap.amount_in,
							amount_tranche_tokens_payout: todo!(),
							amount_remaining,
						},
					)
				}
			}
		}
	}

	pub struct CollectedInvestmentHook<T>(PhantomData<T>);
	impl<T: Config> StatusNotificationHook for CollectedInvestmentHook<T> {
		type Error = DispatchError;
		type Id = (T::AccountId, T::InvestmentId);
		type Status = CollectedAmount<T::Balance>;

		fn notify_status_change(
			(who, investment_id): (T::AccountId, T::InvestmentId),
			status: CollectedAmount<T::Balance>,
		) -> DispatchResult {
			let info = ForeignInvestmentInfo::<T>::get(&who, investment_id)
				.ok_or(Error::<T>::InfoNotFound)?;

			ForeignInvestmentInfo::<T>::remove(&who, investment_id);

			T::CollectedForeignInvestmentHook::notify_status_change(
				(who.clone(), investment_id),
				ExecutedForeignCollect {
					currency: info.foreign_currency,
					amount_currency_payout: status.amount_payment,
					amount_tranche_tokens_payout: todo!(),
					amount_remaining: T::Balance::zero(),
				},
			)
		}
	}

	pub struct CollectedRedemptionHook<T>(PhantomData<T>);
	impl<T: Config> StatusNotificationHook for CollectedRedemptionHook<T> {
		type Error = DispatchError;
		type Id = (T::AccountId, T::InvestmentId);
		type Status = CollectedAmount<T::Balance>;

		fn notify_status_change(
			(who, investment_id): (T::AccountId, T::InvestmentId),
			status: CollectedAmount<T::Balance>,
		) -> DispatchResult {
			let info = ForeignRedemptionInfo::<T>::get(&who, investment_id)
				.ok_or(Error::<T>::InfoNotFound)?;

			let status = Pallet::<T>::apply_swap(
				&who,
				investment_id,
				ForeignSwapKind::Redemption,
				Swap {
					currency_in: info.foreign_currency,
					currency_out: info.pool_currency,
					amount_in: status.amount_collected,
				},
			)?;

			if status.swapped > T::Balance::zero() {
				if status.pending.is_zero() {
					ForeignRedemptionInfo::<T>::remove(&who, investment_id);
				}

				T::CollectedForeignRedemptionHook::notify_status_change(
					(who.clone(), investment_id),
					ExecutedForeignCollect {
						currency: info.foreign_currency,
						amount_currency_payout: status.swapped,
						amount_tranche_tokens_payout: todo!(),
						amount_remaining: status.pending,
					},
				)?;
			}

			Ok(())
		}
	}
}
