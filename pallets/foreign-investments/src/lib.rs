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

use cfg_traits::{investments::TrancheCurrency, PoolInspect};
use cfg_types::investments::{CollectedAmount, Swap};
use frame_support::{dispatch::DispatchResult, ensure};
pub use pallet::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{traits::EnsureSub, ArithmeticError, DispatchError};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Hold the base information of a foreign investment/redemption
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct BaseInfo<T: Config> {
	foreign_currency: T::CurrencyId,
	pool_currency: T::CurrencyId,
	collected: CollectedAmount<T::Balance>,
}

impl<T: Config> BaseInfo<T> {
	fn new(
		investment_id: T::InvestmentId,
		foreign_currency: T::CurrencyId,
	) -> Result<Self, DispatchError> {
		Ok(Self {
			foreign_currency,
			pool_currency: T::PoolInspect::currency_for(investment_id.of_pool())
				.ok_or(Error::<T>::PoolNotFound)?,
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
	base: BaseInfo<T>,
	increase_swap_id: Option<T::SwapId>,
	decrease_swap_id: Option<T::SwapId>,
	total_foreign_amount: T::Balance,
}

impl<T: Config> InvestmentInfo<T> {
	fn new(
		investment_id: T::InvestmentId,
		foreign_currency: T::CurrencyId,
	) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(investment_id, foreign_currency)?,
			total_foreign_amount: T::Balance::default(),
			increase_swap_id: None,
			decrease_swap_id: None,
		})
	}

	fn remaining_foreign_amount(&self) -> Result<T::Balance, ArithmeticError> {
		self.total_foreign_amount
			.ensure_sub(self.base.collected.amount_collected)
	}
}

/// Hold the information of an foreign investment
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RedemptionInfo<T: Config> {
	base: BaseInfo<T>,
	swap_id: Option<T::SwapId>,
	total_tranche_tokens: T::Balance,
	swapped_amount: T::Balance,
}

impl<T: Config> RedemptionInfo<T> {
	fn new(
		investment_id: T::InvestmentId,
		foreign_currency: T::CurrencyId,
	) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(investment_id, foreign_currency)?,
			swap_id: None,
			total_tranche_tokens: T::Balance::default(),
			swapped_amount: T::Balance::default(),
		})
	}

	fn remaining_tranche_tokens(&self) -> Result<T::Balance, ArithmeticError> {
		self.total_tranche_tokens
			.ensure_sub(self.base.collected.amount_payment)
	}
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
	use sp_runtime::{
		traits::{
			AtLeast32BitUnsigned, EnsureAdd, EnsureAddAssign, EnsureSub, EnsureSubAssign, One, Zero,
		},
		FixedPointOperand,
	};

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
			+ FixedPointOperand
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
		InvestmentInfo<T>,
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
		RedemptionInfo<T>,
	>;

	/// Maps a `SwapId` to their corresponding `AccountId` and `InvestmentId`
	///
	/// NOTE: The storage is killed when the swap order no longer exists
	#[pallet::storage]
	pub(super) type SwapidToForeignId<T: Config> =
		StorageMap<_, Blake2_128Concat, T::SwapId, (T::AccountId, T::InvestmentId)>;

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
		/// An action for a different foreign currency is currently in process
		/// for the same pool currency, account, and investment.
		/// The currenct foreign actions must be finished before starting with a
		/// different foreign currency investment / redemption.
		MismatchedForeignCurrency,
	}

	impl<T: Config> Pallet<T> {
		fn apply_order(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			foreign_currency: T::CurrencyId,
			amount_in: T::Balance,
			maybe_swap_id: Option<T::SwapId>,
		) -> Result<T::SwapId, DispatchError> {
			match maybe_swap_id {
				Some(swap_id) => {
					let swap = T::TokenSwaps::get_order_details(swap_id)
						.ok_or(Error::<T>::SwapOrderNotFound)?;

					T::TokenSwaps::update_order(
						who.clone(),
						swap_id,
						swap.amount_in.ensure_add(amount_in)?,
						T::BalanceRatio::one(),
					)?;

					Ok(swap_id)
				}
				None => {
					let pool_currency = T::PoolInspect::currency_for(investment_id.of_pool())
						.ok_or(Error::<T>::PoolNotFound)?;

					let swap_id = T::TokenSwaps::place_order(
						who.clone(),
						pool_currency,
						foreign_currency,
						amount_in,
						T::BalanceRatio::one(),
					)?;

					SwapidToForeignId::<T>::insert(swap_id, (who.clone(), investment_id));

					Ok(swap_id)
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

			ForeignInvestmentInfo::<T>::mutate(&who, investment_id, |info| {
				let info =
					info.get_or_insert(InvestmentInfo::new(investment_id, foreign_currency)?);

				info.base.ensure_same_foreign(foreign_currency)?;
				info.total_foreign_amount
					.ensure_add_assign(foreign_amount)?;

				let swap_id = Self::apply_order(
					who,
					investment_id,
					foreign_currency,
					pool_amount,
					info.increase_swap_id,
				)?;

				info.increase_swap_id = Some(swap_id);

				Ok(())
			})
		}

		fn decrease_foreign_investment(
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

			ForeignInvestmentInfo::<T>::mutate(&who, investment_id, |info| {
				let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;

				info.base.ensure_same_foreign(foreign_currency)?;
				info.total_foreign_amount
					.ensure_sub_assign(foreign_amount)?;

				let swap_id = Self::apply_order(
					who,
					investment_id,
					foreign_currency,
					pool_amount,
					info.decrease_swap_id,
				)?;

				info.decrease_swap_id = Some(swap_id);

				Ok::<_, DispatchError>(())
			})?;

			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_sub(pool_amount)?,
			)
		}

		fn increase_foreign_redemption(
			who: &T::AccountId,
			investment_id: T::InvestmentId,
			tranche_tokens_amount: T::Balance,
			payout_foreign_currency: T::CurrencyId,
		) -> DispatchResult {
			ForeignRedemptionInfo::<T>::mutate(who, investment_id, |info| -> DispatchResult {
				let info = info
					.get_or_insert(RedemptionInfo::new(investment_id, payout_foreign_currency)?);

				info.base.ensure_same_foreign(payout_foreign_currency)?;
				info.total_tranche_tokens
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
		) -> Result<(T::Balance, T::Balance), DispatchError> {
			let remaining_tranche_tokens =
				ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
					let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;

					info.base.ensure_same_foreign(payout_foreign_currency)?;
					info.total_tranche_tokens
						.ensure_sub_assign(tranche_tokens_amount)?;

					Ok::<_, DispatchError>(info.remaining_tranche_tokens()?)
				})?;

			T::Investment::update_redemption(
				who,
				investment_id,
				T::Investment::redemption(who, investment_id)?.ensure_sub(tranche_tokens_amount)?,
			)?;

			let remaining_amount = T::Investment::redemption(who, investment_id)?;

			Ok((remaining_tranche_tokens, remaining_amount))
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
	impl<T: Config> StatusNotificationHook for FulfilledSwapOrderHook<T> {
		type Error = DispatchError;
		type Id = T::SwapId;
		type Status = SwapOf<T>;

		fn notify_status_change(
			swap_id: T::SwapId,
			last_swap: SwapOf<T>,
		) -> Result<(), DispatchError> {
			let (who, investment_id) =
				SwapidToForeignId::<T>::get(swap_id).ok_or(Error::<T>::SwapOrderNotFound)?;

			let pending_amount = match T::TokenSwaps::get_order_details(swap_id) {
				Some(swap) => swap.amount_in,
				None => {
					SwapidToForeignId::<T>::remove(swap_id);
					T::Balance::zero()
				}
			};

			ForeignInvestmentInfo::<T>::mutate_exists(&who, investment_id, |maybe_info| {
				if let Some(info) = maybe_info {
					if info.increase_swap_id == Some(swap_id) {
						T::Investment::update_investment(
							&who,
							investment_id,
							T::Investment::investment(&who, investment_id)?
								.ensure_add(last_swap.amount_in)?,
						)?;

						if pending_amount.is_zero() {
							*maybe_info = None;
						}
					} else if info.decrease_swap_id == Some(swap_id) {
						T::DecreasedForeignInvestOrderHook::notify_status_change(
							(who.clone(), investment_id),
							ExecutedForeignDecreaseInvest {
								amount_decreased: last_swap.amount_in,
								foreign_currency: info.base.foreign_currency,
								amount_remaining: pending_amount,
							},
						)?;

						if pending_amount.is_zero() {
							*maybe_info = None;
						}
					}
				}

				Ok::<_, DispatchError>(())
			})?;

			ForeignRedemptionInfo::<T>::mutate_exists(&who, investment_id, |maybe_info| {
				if let Some(info) = maybe_info {
					if info.swap_id == Some(swap_id) {
						info.swapped_amount.ensure_add_assign(last_swap.amount_in)?;

						if info.base.collected.amount_collected == info.swapped_amount {
							T::CollectedForeignRedemptionHook::notify_status_change(
								(who.clone(), investment_id),
								ExecutedForeignCollect {
									currency: info.base.foreign_currency,
									amount_currency_payout: last_swap.amount_in,
									amount_tranche_tokens_payout: T::Balance::zero(),
									amount_remaining: info.remaining_tranche_tokens()?,
								},
							)?;

							if info.remaining_tranche_tokens()?.is_zero() {
								*maybe_info = None;
							}
						}
					}
				}

				Ok(())
			})
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
			let info = ForeignInvestmentInfo::<T>::mutate(&who, investment_id, |maybe_info| {
				let info = maybe_info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
				info.base.collected.increase(&collected)?;
				Ok::<_, DispatchError>(info.clone())
			})?;

			if info.remaining_foreign_amount()?.is_zero() {
				ForeignInvestmentInfo::<T>::remove(&who, investment_id);
			}

			T::CollectedForeignInvestmentHook::notify_status_change(
				(who.clone(), investment_id),
				ExecutedForeignCollect {
					currency: info.base.foreign_currency,
					amount_currency_payout: collected.amount_payment,
					amount_tranche_tokens_payout: collected.amount_collected,
					amount_remaining: info.remaining_foreign_amount()?,
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
			collected: CollectedAmount<T::Balance>,
		) -> DispatchResult {
			ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
				let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
				info.base.collected.increase(&collected)?;

				let swap_id = Pallet::<T>::apply_order(
					&who,
					investment_id,
					info.base.foreign_currency,
					collected.amount_collected,
					info.swap_id,
				)?;

				info.swap_id = Some(swap_id);

				Ok(())
			})
		}
	}
}
