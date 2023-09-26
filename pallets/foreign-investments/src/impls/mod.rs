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
#![allow(clippy::map_identity)]

use cfg_traits::{
	investments::{ForeignInvestment, Investment, InvestmentCollector, TrancheCurrency},
	ConversionToAssetBalance, IdentityCurrencyConversion, PoolInspect, StatusNotificationHook,
	TokenSwaps,
};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest, Swap,
};
use frame_support::{ensure, traits::Get, transactional};
use sp_runtime::{
	traits::{EnsureAdd, EnsureAddAssign, EnsureSub, Zero},
	DispatchError, DispatchResult,
};

use crate::{
	errors::{InvestError, RedeemError},
	types::{InvestState, InvestTransition, RedeemState, RedeemTransition, TokenSwapReason},
	CollectedInvestment, CollectedRedemption, Config, Error, Event, ForeignInvestmentInfo,
	ForeignInvestmentInfoOf, InvestmentPaymentCurrency, InvestmentState, Pallet,
	RedemptionPayoutCurrency, RedemptionState, SwapOf, TokenSwapOrderIds,
};

#[cfg(feature = "runtime-benchmarks")]
mod benchmark_utils;
mod invest;
mod redeem;

impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
	type Amount = T::Balance;
	type CurrencyId = T::CurrencyId;
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;

	#[transactional]
	fn increase_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
		foreign_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
	) -> Result<(), DispatchError> {
		ensure!(
			!T::Investment::investment_requires_collect(who, investment_id),
			Error::<T>::InvestError(InvestError::CollectRequired)
		);

		// NOTE: For the MVP, we restrict the investment to the payment currency of the
		// one from the initial increase. Once the `InvestmentState` has been cleared,
		// another payment currency can be introduced.
		let currency_matches = InvestmentPaymentCurrency::<T>::try_mutate_exists(
			who,
			investment_id,
			|maybe_currency| {
				if let Some(currency) = maybe_currency {
					Ok::<bool, DispatchError>(currency == &foreign_currency)
				} else {
					*maybe_currency = Some(foreign_currency);
					Ok::<bool, DispatchError>(true)
				}
			},
		)
		// An error reflects the payment currency has not been set yet
		.unwrap_or(true);
		ensure!(
			currency_matches,
			Error::<T>::InvestError(InvestError::InvalidPaymentCurrency)
		);

		let amount_pool_denominated =
			T::CurrencyConverter::stable_to_stable(pool_currency, foreign_currency, amount)?;
		let pre_state = InvestmentState::<T>::get(who, investment_id);
		let post_state = pre_state
			.transition(InvestTransition::IncreaseInvestOrder(Swap {
				currency_in: pool_currency,
				currency_out: foreign_currency,
				amount: amount_pool_denominated,
			}))
			.map_err(|e| {
				// Inner error holds finer granularity but should never occur
				log::debug!("InvestState transition error: {:?}", e);
				Error::<T>::from(InvestError::IncreaseTransition)
			})?;
		Pallet::<T>::apply_invest_state_transition(who, investment_id, post_state, true)?;
		Ok(())
	}

	#[transactional]
	fn decrease_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
		foreign_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
	) -> Result<(), DispatchError> {
		ensure!(
			!T::Investment::investment_requires_collect(who, investment_id),
			Error::<T>::InvestError(InvestError::CollectRequired)
		);
		let payment_currency = InvestmentPaymentCurrency::<T>::get(who, investment_id)?;
		ensure!(
			payment_currency == foreign_currency,
			Error::<T>::InvestError(InvestError::InvalidPaymentCurrency)
		);

		let pre_state = InvestmentState::<T>::get(who, investment_id);
		ensure!(
			pre_state.get_investing_amount() >= amount,
			Error::<T>::InvestError(InvestError::DecreaseAmountOverflow)
		);

		let post_state = pre_state
			.transition(InvestTransition::DecreaseInvestOrder(Swap {
				currency_in: foreign_currency,
				currency_out: pool_currency,
				amount,
			}))
			.map_err(|e| {
				// Inner error holds finer granularity but should never occur
				log::debug!("InvestState transition error: {:?}", e);
				Error::<T>::from(InvestError::DecreaseTransition)
			})?;
		Pallet::<T>::apply_invest_state_transition(who, investment_id, post_state, true)?;

		Ok(())
	}

	#[transactional]
	fn increase_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
		payout_currency: T::CurrencyId,
	) -> Result<(), DispatchError> {
		let currency_matches = RedemptionPayoutCurrency::<T>::try_mutate_exists(
			who,
			investment_id,
			|maybe_currency| {
				if let Some(currency) = maybe_currency {
					Ok::<bool, DispatchError>(currency == &payout_currency)
				} else {
					*maybe_currency = Some(payout_currency);
					Ok::<bool, DispatchError>(true)
				}
			},
		)
		// An error reflects the payout currency has not been set yet
		.unwrap_or(true);
		ensure!(
			currency_matches,
			Error::<T>::RedeemError(RedeemError::InvalidPayoutCurrency)
		);
		ensure!(
			!T::Investment::redemption_requires_collect(who, investment_id),
			Error::<T>::RedeemError(RedeemError::CollectRequired)
		);

		let pre_state = RedemptionState::<T>::get(who, investment_id);
		let post_state = pre_state
			.transition(RedeemTransition::IncreaseRedeemOrder(amount))
			.map_err(|e| {
				// Inner error holds finer granularity but should never occur
				log::debug!("RedeemState transition error: {:?}", e);
				Error::<T>::from(RedeemError::IncreaseTransition)
			})?;
		Pallet::<T>::apply_redeem_state_transition(who, investment_id, post_state)?;

		Ok(())
	}

	#[transactional]
	fn decrease_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
		payout_currency: T::CurrencyId,
	) -> Result<(T::Balance, T::Balance), DispatchError> {
		let stored_payout_currency = RedemptionPayoutCurrency::<T>::get(who, investment_id)?;
		ensure!(
			stored_payout_currency == payout_currency,
			Error::<T>::RedeemError(RedeemError::InvalidPayoutCurrency)
		);
		ensure!(
			!T::Investment::redemption_requires_collect(who, investment_id),
			Error::<T>::RedeemError(RedeemError::CollectRequired)
		);

		let pre_state = RedemptionState::<T>::get(who, investment_id);
		let post_state = pre_state
			.transition(RedeemTransition::DecreaseRedeemOrder(amount))
			.map_err(|e| {
				log::debug!("RedeemState transition error: {:?}", e);
				Error::<T>::from(RedeemError::DecreaseTransition)
			})?;
		Pallet::<T>::apply_redeem_state_transition(who, investment_id, post_state)?;

		let remaining_amount = T::Investment::redemption(who, investment_id)?;

		Ok((amount, remaining_amount))
	}

	#[transactional]
	fn collect_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_payment_currency: T::CurrencyId,
	) -> DispatchResult {
		let payment_currency = InvestmentPaymentCurrency::<T>::get(who, investment_id)?;
		ensure!(
			payment_currency == foreign_payment_currency,
			Error::<T>::InvestError(InvestError::InvalidPaymentCurrency)
		);

		// Note: We assume the configured Investment trait to notify about the collected
		// amounts via the `CollectedInvestmentHook` which handles incrementing the
		// `CollectedInvestment` amount and notifying any consumer of
		// `ExecutedForeignInvestmentHook` which is expected to dispatch
		// `ExecutedCollectInvest`.
		T::Investment::collect_investment(who.clone(), investment_id)?;

		Ok(())
	}

	#[transactional]
	fn collect_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_payout_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
	) -> Result<(), DispatchError> {
		let payout_currency = RedemptionPayoutCurrency::<T>::get(who, investment_id)?;
		ensure!(
			payout_currency == foreign_payout_currency,
			Error::<T>::RedeemError(RedeemError::InvalidPayoutCurrency)
		);
		ensure!(T::PoolInspect::currency_for(investment_id.of_pool())
			.map(|currency| currency == pool_currency)
			.unwrap_or_else(|| {
				log::debug!("Corruption: Failed to derive pool currency from investment id when collecting foreign redemption. Should never occur if redemption has been increased beforehand");
				false
			}),
			DispatchError::Corruption
		);

		// Note: We assume the configured Investment trait to notify about the collected
		// amounts via the `CollectedRedemptionHook` which handles incrementing the
		// `CollectedRedemption` amount.
		T::Investment::collect_redemption(who.clone(), investment_id)?;

		Ok(())
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

	fn accepted_payment_currency(investment_id: T::InvestmentId, currency: T::CurrencyId) -> bool {
		if T::Investment::accepted_payment_currency(investment_id, currency) {
			true
		} else {
			T::PoolInspect::currency_for(investment_id.of_pool())
				.map(|pool_currency| T::TokenSwaps::valid_pair(pool_currency, currency))
				.unwrap_or(false)
		}
	}

	fn accepted_payout_currency(investment_id: T::InvestmentId, currency: T::CurrencyId) -> bool {
		if T::Investment::accepted_payout_currency(investment_id, currency) {
			true
		} else {
			T::PoolInspect::currency_for(investment_id.of_pool())
				.map(|pool_currency| T::TokenSwaps::valid_pair(currency, pool_currency))
				.unwrap_or(false)
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Applies in-memory transitions of `InvestState` to chain storage. Always
	/// updates/removes `InvestmentState` and the current investment. Depending
	/// on the state, also kills/updates the current token swap order as well as
	/// notifies `ExecutedDecreasedHook`.
	///
	/// The following execution order must not be changed:
	///
	/// 1. If the `InvestState` includes `SwapIntoForeignDone` without
	/// `ActiveSwapIntoForeignCurrency`: Prepare "executed decrease" hook &
	/// transition state into its form without `SwapIntoForeignDone`. If the
	/// state is just `SwapIntoForeignDone`, kill it.
	///
	/// 2. Update the `InvestmentState` storage. This step is required as the
	/// next step reads this storage entry.
	///
	/// 3. Handle the token swap order by either creating, updating or killing
	/// it. Depending on the current swap order and the previous and current
	/// reason to update it, both the current `InvestmentState` as well as
	/// `RedemptionState` might require an update.
	///
	/// 4. If the token swap handling resulted in a new `InvestState`, update
	/// `InvestmentState` again. Additionally, emit `ForeignInvestmentUpdate` or
	/// `ForeignInvestmentCleared`.
	///
	/// 5. If the token swap handling resulted in a new `RedeemState`, update
	/// `RedemptionState` again. If the result includes `SwapIntoForeignDone`
	/// without `ActiveSwapIntoForeignCurrency`, remove the
	/// `SwapIntoForeignDone` part or kill it. Additionally, emit
	/// `ForeignRedemptionUpdate` or `ForeignRedemptionCleared`.
	///
	/// 6. Update the investment. This also includes setting it to zero. We
	/// assume the impl of `<T as Config>::Investment` handles this case.
	///
	/// 7. If "executed decrease" happened, send notification.
	///
	/// NOTES:
	/// * Must be called after transitioning any `InvestState` via
	/// `transition` to update the chain storage.
	/// * When updating token swap orders, only `handle_swap_order` should
	/// be called.
	#[transactional]
	pub(crate) fn apply_invest_state_transition(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		state: InvestState<T>,
		update_swap_order: bool,
	) -> DispatchResult {
		// Must not send executed decrease notification before updating redemption
		let mut maybe_executed_decrease: Option<(T::CurrencyId, T::Balance)> = None;
		// Do first round of updates and forward state, swap as well as invest amount

		match state {
			InvestState::NoState => {
				InvestmentState::<T>::remove(who, investment_id);
				InvestmentPaymentCurrency::<T>::remove(who, investment_id);

				Ok((InvestState::NoState, None, Zero::zero()))
			},
			InvestState::InvestmentOngoing { invest_amount } => {
				InvestmentState::<T>::insert(who, investment_id, state.clone());

				Ok((state, None, invest_amount))
			},
			InvestState::ActiveSwapIntoPoolCurrency { swap } |
			InvestState::ActiveSwapIntoForeignCurrency { swap } |
			// We don't care about `done_amount` until swap into foreign is fulfilled
			InvestState::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, .. } => {
				InvestmentState::<T>::insert(who, investment_id, state.clone());
				Ok((state, Some(swap), Zero::zero()))
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount } |
			InvestState::ActiveSwapIntoForeignCurrencyAndInvestmentOngoing { swap, invest_amount } |
			// We don't care about `done_amount` until swap into foreign is fulfilled
			InvestState::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDoneAndInvestmentOngoing { swap,invest_amount, .. } => {
				InvestmentState::<T>::insert(who, investment_id, state.clone());
				Ok((state, Some(swap), invest_amount))
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				maybe_executed_decrease = Some((swap.currency_out, done_amount));

				let new_state = InvestState::ActiveSwapIntoPoolCurrency { swap };
				InvestmentState::<T>::insert(who, investment_id, new_state.clone());

				Ok((new_state, Some(swap), Zero::zero()))
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoForeignDoneAndInvestmentOngoing { swap, done_amount, invest_amount } => {
				maybe_executed_decrease = Some((swap.currency_out, done_amount));

				let new_state = InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount };
				InvestmentState::<T>::insert(who, investment_id, new_state.clone());

				Ok((new_state, Some(swap), invest_amount))
			},
			InvestState::SwapIntoForeignDone { done_swap } => {
				maybe_executed_decrease = Some((done_swap.currency_in, done_swap.amount));

				InvestmentState::<T>::remove(who, investment_id);
				InvestmentPaymentCurrency::<T>::remove(who, investment_id);

				Ok((InvestState::NoState, None, Zero::zero()))
			},
			InvestState::SwapIntoForeignDoneAndInvestmentOngoing { done_swap, invest_amount } => {
				maybe_executed_decrease = Some((done_swap.currency_in, done_swap.amount));

				let new_state = InvestState::InvestmentOngoing { invest_amount };
				InvestmentState::<T>::insert(who, investment_id, new_state.clone());

				Ok((new_state, None, invest_amount))
			},
		}
		.map(|(invest_state, maybe_swap, invest_amount)| {
			// Must update investment amount before handling swap as in case of decrease, 
			// updating the swap transfers the currency from the investment account to the 
			// investor which is required for placing the swap order
			if T::Investment::investment(who, investment_id)? != invest_amount {
				T::Investment::update_investment(who, investment_id, invest_amount)?;
			}

			// No need to handle swap order, if redeem state transition is applied afterwards
			let final_invest_state = if update_swap_order {
				Self::handle_swap_order(who, investment_id, maybe_swap, TokenSwapReason::Investment).map(|(maybe_invest_state, maybe_redeem_state)| {
					Self::deposit_redemption_event(who, investment_id, maybe_redeem_state);
					maybe_invest_state.unwrap_or(invest_state)
				})?
			} else {
				invest_state
			};
			Self::deposit_investment_event(who, investment_id, Some(final_invest_state));

			// Send notification after updating invest as else funds are still locked in investment account
			if let Some((foreign_currency, decreased_amount)) = maybe_executed_decrease {
				Self::notify_executed_decrease_invest(who, investment_id, foreign_currency, decreased_amount)?;
			}

			Ok(())
		})
		.map_err(|e: DispatchError| e)?
	}

	/// Applies in-memory transitions of `RedeemState` to chain storage. Always
	/// updates/removes `RedemptionState` and the current redemption. Depending
	/// on the state, also kills/updates the current token swap order.
	///
	/// The following execution order must not be changed:
	///
	/// 1. If the `RedeemState` includes `SwapIntoForeignDone` without
	/// `ActiveSwapIntoForeignCurrency`, remove the `SwapIntoForeignDone` part
	/// or kill it.
	///
	/// 2. Update the `RedemptionState` storage. This step is required as the
	/// next step reads this storage entry.
	///
	/// 3. Handle the token swap order by either creating, updating or killing
	/// it. Depending on the current swap order and the previous and current
	/// reason to update it, both the current `RedemptionState` as well as
	/// `RedemptionState` might require an update.
	///
	/// 4. If the token swap handling resulted in a new `RedeemState`, update
	/// `RedemptionState` again. If the result includes `SwapIntoForeignDone`
	/// without `ActiveSwapIntoForeignCurrency`, remove the
	/// `SwapIntoForeignDone` part or kill it. Additionally, emit
	/// `ForeignRedemptionUpdate` or `ForeignRedemptionCleared`.
	///
	/// 5. If the token swap handling resulted in a new `InvestState`,
	/// update `InvestmentState`. Additionally, emit `ForeignInvestmentUpdate`
	/// or `ForeignInvestmentCleared`.
	///
	/// 6. Update the redemption. This also includes setting it to zero. We
	/// assume the impl of `<T as Config>::Investment` handles this case.
	///
	/// NOTES:
	/// * Must be called after transitioning g any `RedeemState` via
	/// `transition` to update the chain storage.
	/// * When updating token swap orders, only `handle_swap_order` should
	/// be called.
	#[transactional]
	pub(crate) fn apply_redeem_state_transition(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		state: RedeemState<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let redeeming_amount = state.get_redeeming_amount();

		// Do first round of updates and forward state as well as swap
		match state {
			RedeemState::NoState => {
				RedemptionState::<T>::remove(who, investment_id);
				RedemptionPayoutCurrency::<T>::remove(who, investment_id);
				Ok((Some(RedeemState::NoState), None))
			}
			RedeemState::Redeeming { .. } => {
				RedemptionState::<T>::insert(who, investment_id, state);
				Ok((Some(state), None))
			}
			RedeemState::RedeemingAndActiveSwapIntoForeignCurrency { swap, .. }
			| RedeemState::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
				swap,
				..
			}
			| RedeemState::ActiveSwapIntoForeignCurrency { swap, .. }
			| RedeemState::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, .. } => {
				RedemptionState::<T>::insert(who, investment_id, state);
				Ok((Some(state), Some(swap)))
			}
			// Only states left include `SwapIntoForeignDone` without
			// `ActiveSwapIntoForeignCurrency` such that we can notify collect
			swap_done_state => {
				let maybe_new_state =
					Self::apply_collect_redeem_transition(who, investment_id, swap_done_state)?;
				Ok((maybe_new_state, None))
			}
		}
		.map(|(maybe_new_state, maybe_swap)| {
			let (maybe_new_invest_state, maybe_new_state_prio) = Self::handle_swap_order(
				who,
				investment_id,
				maybe_swap,
				TokenSwapReason::Redemption,
			)?;

			// Dispatch transition event, post swap state has priority if it exists as it is
			// the result of the latest update
			if let Some(redeem_state_post_swap) = maybe_new_state_prio {
				Self::deposit_redemption_event(who, investment_id, Some(redeem_state_post_swap));
			} else {
				Self::deposit_redemption_event(who, investment_id, maybe_new_state);
			}
			Self::deposit_investment_event(who, investment_id, maybe_new_invest_state);

			if T::Investment::redemption(who, investment_id)? != redeeming_amount {
				// Finally, update redemption after all states have been updated
				T::Investment::update_redemption(who, investment_id, redeeming_amount)?;
			}

			Ok(())
		})
		.map_err(|e: DispatchError| e)?
	}

	/// Emits an event indicating the corresponding `InvestState` was either
	/// updated or cleared.
	///
	/// NOTE: Noop if the provided state is `None`.
	fn deposit_investment_event(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		maybe_state: Option<InvestState<T>>,
	) {
		match maybe_state {
			Some(state) if state == InvestState::NoState => {
				Self::deposit_event(Event::<T>::ForeignInvestmentCleared {
					investor: who.clone(),
					investment_id,
				})
			}
			Some(state) => Self::deposit_event(Event::<T>::ForeignInvestmentUpdated {
				investor: who.clone(),
				investment_id,
				state,
			}),
			_ => {}
		}
	}

	/// Emits an event indicating the corresponding `InvestState` was either
	/// updated or cleared.
	///
	/// NOTE: Noop if the provided state is `None`.
	fn deposit_redemption_event(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		maybe_state: Option<RedeemState<T::Balance, T::CurrencyId>>,
	) {
		match maybe_state {
			Some(state) if state == RedeemState::NoState => {
				Self::deposit_event(Event::<T>::ForeignRedemptionCleared {
					investor: who.clone(),
					investment_id,
				})
			}
			Some(state) => Self::deposit_event(Event::<T>::ForeignRedemptionUpdated {
				investor: who.clone(),
				investment_id,
				state,
			}),
			None => {}
		}
	}

	/// Terminates a redeem collection which required swapping into foreign
	/// currency.
	///
	/// Only acts upon redeem states which include `SwapIntoForeignDone`
	/// without `ActiveSwapIntoForeignCurrency`. Other states are ignored.
	/// Either updates the corresponding `RedemptionState` or drops it entirely.
	///
	/// Emits `notify_executed_collect_redeem`.
	///
	/// Returning...
	/// * `Some(RedeemState::NoState)` indicates a `ForeignRedemptionCleared`
	///   event can be deposited
	/// * `Some(state)` indicates a `ForeignRedemptionUpdated` event can be
	///   deposited
	/// * `None` indicates no state mutation occurred
	#[allow(clippy::type_complexity)]
	#[transactional]
	fn apply_collect_redeem_transition(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		state: RedeemState<T::Balance, T::CurrencyId>,
	) -> Result<Option<RedeemState<T::Balance, T::CurrencyId>>, DispatchError> {
		let CollectedAmount::<T::Balance> {
			amount_payment: amount_payment_tranche_tokens,
			..
		} = CollectedRedemption::<T>::get(who, investment_id);

		// Send notification and kill `CollectedRedemptionTrancheTokens` iff the state
		// includes `SwapIntoForeignDone` without `ActiveSwapIntoForeignCurrency`
		match state {
			RedeemState::SwapIntoForeignDone { done_swap, .. }
			| RedeemState::RedeemingAndSwapIntoForeignDone { done_swap, .. } => {
				Self::notify_executed_collect_redeem(
					who,
					investment_id,
					done_swap.currency_in,
					CollectedAmount {
						amount_collected: done_swap.amount,
						amount_payment: amount_payment_tranche_tokens,
					},
				)?;
				CollectedRedemption::<T>::remove(who, investment_id);
				Ok(())
			}
			_ => Ok(()),
		}
		.map_err(|e: DispatchError| e)?;

		// Update state iff the state includes `SwapIntoForeignDone` without
		// `ActiveSwapIntoForeignCurrency`
		match state {
			RedeemState::SwapIntoForeignDone { .. } => {
				RedemptionState::<T>::remove(who, investment_id);
				RedemptionPayoutCurrency::<T>::remove(who, investment_id);
				Ok(Some(RedeemState::NoState))
			}
			RedeemState::RedeemingAndSwapIntoForeignDone { redeem_amount, .. } => {
				let new_state = RedeemState::Redeeming { redeem_amount };
				RedemptionState::<T>::insert(who, investment_id, new_state);
				Ok(Some(new_state))
			}
			_ => Ok(None),
		}
	}

	/// Updates or kills a token swap order. If the final swap amount is zero,
	/// kills the swap order and all associated storage. Else, creates or
	/// updates an existing swap order.
	///
	/// If the provided reason does not match the latest one stored in
	/// `ForeignInvestmentInfo`, also resolves the _merge conflict_ resulting
	/// from updating and thus overwriting opposite swaps. See
	/// [Self::handle_concurrent_swap_orders] for details. If this results in
	/// either an altered invest state and/or an altered redeem state, the
	/// corresponding storage is updated and the new states returned. The latter
	/// is required for emitting events.
	///
	/// NOTE: Must not call any other swap order updating function.
	#[allow(clippy::type_complexity)]
	#[transactional]
	fn handle_swap_order(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		maybe_swap: Option<SwapOf<T>>,
		reason: TokenSwapReason,
	) -> Result<
		(
			Option<InvestState<T>>,
			Option<RedeemState<T::Balance, T::CurrencyId>>,
		),
		DispatchError,
	> {
		// check for concurrent conflicting swap orders
		if TokenSwapOrderIds::<T>::get(who, investment_id).is_some() {
			let (maybe_updated_swap, maybe_invest_state, maybe_redeem_state, swap_reason) =
				Self::handle_concurrent_swap_orders(who, investment_id)?;

			// Update or kill swap order with updated order having priority in case it was
			// overwritten
			if let Some(swap_order) = maybe_updated_swap {
				Self::place_swap_order(who, investment_id, swap_order, swap_reason)?;
			} else {
				Self::kill_swap_order(who, investment_id)?;
			}

			// Update invest state and kill if NoState
			InvestmentState::<T>::mutate_exists(who, investment_id, |current_invest_state| {
				match &maybe_invest_state {
					Some(state) if state != &InvestState::NoState => {
						*current_invest_state = Some(state.clone());
					}
					Some(state) if state == &InvestState::NoState => {
						*current_invest_state = None;
					}
					_ => (),
				}
			});

			// Need to check if `SwapReturnDone` is part of state without
			// `ActiveSwapIntoForeignCurrency` as this implies the successful termination of
			// a collect (with swap into foreign currency). If this is the case, the
			// returned redeem state needs to be updated or killed as well.
			let returning_redeem_state = Self::apply_collect_redeem_transition(
				who,
				investment_id,
				maybe_redeem_state.unwrap_or_default(),
			)?
			.map(Some)
			.unwrap_or(maybe_redeem_state)
			.map(|redeem_state| {
				RedemptionState::<T>::mutate_exists(who, investment_id, |current_redeem_state| {
					if redeem_state != RedeemState::NoState {
						*current_redeem_state = Some(redeem_state);
					} else {
						*current_redeem_state = None;
					}
				});
				redeem_state
			});

			Ok((maybe_invest_state, returning_redeem_state))
		}
		// Update to provided value, if not none
		else if let Some(swap_order) = maybe_swap {
			Self::place_swap_order(who, investment_id, swap_order, Some(reason))?;
			Ok((None, None))
		} else {
			Ok((None, None))
		}
	}

	/// Kills all storage associated with token swaps and cancels the
	/// potentially active swap order.
	#[transactional]
	fn kill_swap_order(who: &T::AccountId, investment_id: T::InvestmentId) -> DispatchResult {
		if let Some(swap_order_id) = TokenSwapOrderIds::<T>::take(who, investment_id) {
			if T::TokenSwaps::is_active(swap_order_id) {
				T::TokenSwaps::cancel_order(swap_order_id)?;
			}
			ForeignInvestmentInfo::<T>::remove(swap_order_id);
		}
		Ok(())
	}

	/// Sets up `TokenSwapOrderIds` and `ForeignInvestmentInfo` storages, if the
	/// order does not exist yet.
	///
	/// NOTE: Must only be called in `handle_swap_order`.
	#[transactional]
	fn place_swap_order(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swap: SwapOf<T>,
		reason: Option<TokenSwapReason>,
	) -> DispatchResult {
		if swap.amount.is_zero() {
			return Self::kill_swap_order(who, investment_id);
		}

		// Determine whether swap order direction changed which would require the order
		// to be cancelled and all associated storage to be killed
		let maybe_swap_order_id = TokenSwapOrderIds::<T>::get(who, investment_id);
		let cancel_swap_order = maybe_swap_order_id
			.map(|swap_order_id| {
				let cancel_swap_order = T::TokenSwaps::get_order_details(swap_order_id)
					.map(|swap_order| {
						swap_order.currency_in != swap.currency_in
							|| swap_order.currency_out != swap.currency_out
					})
					.unwrap_or(false);

				if cancel_swap_order {
					Self::kill_swap_order(who, investment_id)?;
				}

				Ok::<bool, DispatchError>(cancel_swap_order)
			})
			.transpose()?
			.unwrap_or(false);

		match maybe_swap_order_id {
			// Swap order is active and matches the swap direction
			Some(swap_order_id)
				if T::TokenSwaps::is_active(swap_order_id) && !cancel_swap_order =>
			{
				T::TokenSwaps::update_order(
					who.clone(),
					swap_order_id,
					swap.amount,
					// The max accepted sell rate is independent of the asset type for now
					T::DefaultTokenSellRatio::get(),
					// Convert default min fulfillment amount from native to incoming currency
					T::DecimalConverter::to_asset_balance(
						T::DefaultMinSwapFulfillmentAmount::get(),
						swap.currency_in,
					)?,
				)?;
				ForeignInvestmentInfo::<T>::insert(
					swap_order_id,
					ForeignInvestmentInfoOf::<T> {
						owner: who.clone(),
						id: investment_id,
						last_swap_reason: reason,
					},
				);
			}
			// Edge case: Only occurs as result of implicit collect when fulfilling a swap
			// order. At this point, swap is fulfilled but not propagated to the state yet as
			// collecting has to happen beforehand.
			Some(swap_order_id)
				if !T::TokenSwaps::is_active(swap_order_id) && !cancel_swap_order =>
			{
				Self::kill_swap_order(who, investment_id)?;
			}
			// Swap order either has not existed at all or was just cancelled
			_ => {
				let swap_order_id = T::TokenSwaps::place_order(
					who.clone(),
					swap.currency_in,
					swap.currency_out,
					swap.amount,
					// The max accepted sell rate is independent of the asset type for now
					T::DefaultTokenSellRatio::get(),
					// Convert default min fulfillment amount from native to incoming currency
					T::DecimalConverter::to_asset_balance(
						T::DefaultMinSwapFulfillmentAmount::get(),
						swap.currency_in,
					)?,
				)?;
				TokenSwapOrderIds::<T>::insert(who, investment_id, swap_order_id);
				ForeignInvestmentInfo::<T>::insert(
					swap_order_id,
					ForeignInvestmentInfoOf::<T> {
						owner: who.clone(),
						id: investment_id,
						last_swap_reason: reason,
					},
				);
			}
		};
		Ok(())
	}

	/// Determines the correct amount of a token swap based on the current
	/// `InvestState` and `RedeemState` corresponding to the `TokenSwapOrderId`.
	///
	/// Returns a tuple of the total swap order amount as well as potentially
	/// altered invest and redeem states. Any returning tuple element which is
	/// `None`, reflects that no change is required for this element. Else, it
	/// needs to be applied to the storage.
	///
	/// NOTE: Required since there exists at most one swap per `(AccountId,
	/// InvestmentId)` pair whereas investments and redemptions can both mutate
	/// orders. Assume, as a result of an `InvestState` transition, a token swap
	/// order into pool currency is initialized. Then, as a result of a
	/// `RedeemState` transition, a token swap order into foreign currency is
	/// needed. This handler resolves the _merge conflict_ in situations where
	/// the reason to create/update a swap order does not match the previous
	/// reason.
	///
	/// * Is noop, if the the current reason equals the previous one.
	/// * If both states are swapping into foreign currency, i.e. their invest
	///   and redeem states include `ActiveSwapIntoForeignCurrency`, the states
	///   stay the same. However the total order amount needs to be updated by
	///   summing up both swap order amounts.
	/// * If the `InvestState` includes swapping into pool currency, i.e.
	///   `ActiveSwapIntoPoolCurrency`, whereas the `RedeemState` is swapping
	///   into the opposite direction, i.e. `ActiveSwapIntoForeignCurrency`, we
	///   need to resolve the delta between both swap order amounts and update
	///   the states accordingly.
	#[allow(clippy::type_complexity)]
	fn handle_concurrent_swap_orders(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<
		(
			Option<Swap<T::Balance, T::CurrencyId>>,
			Option<InvestState<T>>,
			Option<RedeemState<T::Balance, T::CurrencyId>>,
			Option<TokenSwapReason>,
		),
		DispatchError,
	> {
		// Read states from storage and determine amounts in possible denominations
		let invest_state = InvestmentState::<T>::get(who, investment_id);
		let redeem_state = RedemptionState::<T>::get(who, investment_id);
		let active_invest_swap = invest_state.get_active_swap();
		let active_redeem_swap = redeem_state.get_active_swap();

		// Exit early if neither or only a single swap is active such that no merging is
		// necessary
		if active_invest_swap.is_none() && active_redeem_swap.is_none() {
			return Ok((None, None, None, None));
		} else if active_invest_swap.is_none() {
			return Ok((
				active_redeem_swap,
				None,
				Some(redeem_state),
				Some(TokenSwapReason::Redemption),
			));
		} else if active_redeem_swap.is_none() {
			return Ok((
				active_invest_swap,
				Some(invest_state),
				None,
				Some(TokenSwapReason::Investment),
			));
		}

		let invest_swap_amount_pool_deno =
			invest_state.get_active_swap_amount_pool_denominated()?;
		let invest_swap_amount_foreign_deno =
			invest_state.get_active_swap_amount_foreign_denominated()?;
		let (redeem_swap_amount_foreign_deno, redeem_swap_amount_pool_deno) = redeem_state
			.get_active_swap()
			.map(|swap| {
				// Redemptions can only swap into foreign
				let amount_pool_denominated = T::CurrencyConverter::stable_to_stable(
					swap.currency_out,
					swap.currency_in,
					swap.amount,
				)?;
				Ok::<(T::Balance, T::Balance), DispatchError>((
					swap.amount,
					amount_pool_denominated,
				))
			})
			.transpose()?
			.unwrap_or_default();
		let resolved_amount_pool_deno =
			invest_swap_amount_pool_deno.min(redeem_swap_amount_pool_deno);
		let swap_amount_opposite_direction_pool_deno = invest_swap_amount_pool_deno
			.max(redeem_swap_amount_pool_deno)
			.ensure_sub(resolved_amount_pool_deno)?;
		let swap_amount_opposite_direction_foreign_deno = invest_swap_amount_foreign_deno
			.max(redeem_swap_amount_foreign_deno)
			.ensure_sub(invest_swap_amount_foreign_deno.min(redeem_swap_amount_foreign_deno))?;

		let (maybe_token_swap, maybe_new_invest_state, maybe_new_redeem_state, swap_reason) =
			match (active_invest_swap, active_redeem_swap) {
				// same swap direction
				(Some(invest_swap), Some(redeem_swap))
					if invest_swap.currency_in == redeem_swap.currency_in =>
				{
					invest_swap.ensure_currencies_match(&redeem_swap, true)?;
					let token_swap = Swap {
						amount: invest_swap.amount.ensure_add(redeem_swap.amount)?,
						..invest_swap
					};
					Ok((
						Some(token_swap),
						None,
						None,
						Some(TokenSwapReason::InvestmentAndRedemption),
					))
				}
				// opposite swap direction
				(Some(invest_swap), Some(redeem_swap))
					if invest_swap.currency_in == redeem_swap.currency_out =>
				{
					invest_swap.ensure_currencies_match(&redeem_swap, false)?;
					let new_redeem_state = redeem_state.fulfill_active_swap_amount(
						redeem_swap_amount_foreign_deno.min(invest_swap_amount_foreign_deno),
					)?;

					let new_invest_state = match invest_state.clone() {
						InvestState::ActiveSwapIntoPoolCurrency { swap: pool_swap }
						| InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
							swap: pool_swap,
							..
						} => {
							let new_pool_swap = Swap {
								amount: pool_swap.amount.ensure_sub(resolved_amount_pool_deno)?,
								..pool_swap
							};
							let new_invest_amount = invest_state
								.get_investing_amount()
								.ensure_add(resolved_amount_pool_deno)?;

							if pool_swap.amount == resolved_amount_pool_deno {
								Ok(InvestState::InvestmentOngoing {
									invest_amount: new_invest_amount,
								})
							} else {
								Ok(
									InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
										invest_amount: new_invest_amount,
										swap: new_pool_swap,
									},
								)
							}
						}
						state => Ok(state),
					}
					.map_err(|e: DispatchError| e)?;

					if invest_swap_amount_foreign_deno > redeem_swap_amount_foreign_deno {
						let swap = Swap {
							amount: swap_amount_opposite_direction_pool_deno,
							..invest_swap
						};
						Ok((
							Some(swap),
							Some(new_invest_state),
							Some(new_redeem_state),
							Some(TokenSwapReason::Investment),
						))
					} else {
						let swap = Swap {
							amount: swap_amount_opposite_direction_foreign_deno,
							..redeem_swap
						};
						Ok((
							Some(swap),
							Some(new_invest_state),
							Some(new_redeem_state),
							Some(TokenSwapReason::Redemption),
						))
					}
				}
				_ => Err(DispatchError::Other(
					"Uncaught short circuit when merging concurrent swap orders",
				)),
			}
			.map_err(|e: DispatchError| e)?;

		let new_invest_state = match maybe_new_invest_state {
			Some(state) if state == invest_state => None,
			state => state,
		};
		let new_redeem_state = match maybe_new_redeem_state {
			Some(state) if state == redeem_state => None,
			state => state,
		};

		Ok((
			maybe_token_swap,
			new_invest_state,
			new_redeem_state,
			swap_reason,
		))
	}

	/// Increments the collected investment amount and transitions investment
	/// state as a result of collecting the investment.
	///
	/// NOTE: Does not transfer back the collected tranche tokens. This happens
	/// in `notify_executed_collect_invest`.
	#[transactional]
	pub(crate) fn denote_collected_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::Balance>,
	) -> DispatchResult {
		// Increment by previously stored amounts (via `CollectedInvestmentHook`)
		let nothing_collect = CollectedInvestment::<T>::mutate(who, investment_id, |c| {
			c.amount_collected
				.ensure_add_assign(collected.amount_collected)?;
			c.amount_payment
				.ensure_add_assign(collected.amount_payment)?;
			Ok::<bool, DispatchError>(c.amount_collected.is_zero() && c.amount_payment.is_zero())
		})?;

		// No need to transition if nothing was collected
		if nothing_collect {
			return Ok(());
		}

		// Update invest state to decrease the unprocessed investing amount
		let investing_amount = T::Investment::investment(who, investment_id)?;
		let pre_state = InvestmentState::<T>::get(who, investment_id);
		let post_state =
			pre_state.transition(InvestTransition::CollectInvestment(investing_amount))?;

		// Need to send notification before potentially killing the `InvestmentState` if
		// all was collected and no swap is remaining
		Self::notify_executed_collect_invest(who, investment_id)?;

		Self::apply_invest_state_transition(who, investment_id, post_state, true).map_err(|e| {
			log::debug!("InvestState transition error: {:?}", e);
			Error::<T>::from(InvestError::CollectTransition)
		})?;

		Ok(())
	}

	/// Increments the collected redemption amount and transitions redemption
	/// state as a result of collecting the redemption.
	///
	/// NOTE: Neither initiates a swap from the collected pool currency into
	/// foreign currency nor transfers back any currency to the investor. This
	/// happens in `transfer_collected_redemption`.
	pub(crate) fn denote_collected_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::Balance>,
	) -> DispatchResult {
		let foreign_payout_currency = RedemptionPayoutCurrency::<T>::get(who, investment_id)?;
		let pool_currency = T::PoolInspect::currency_for(investment_id.of_pool())
			.expect("Impossible to collect redemption for non existing pool at this point");

		// Increment by previously stored amounts (via `CollectedInvestmentHook`)
		let nothing_collect = CollectedRedemption::<T>::mutate(who, investment_id, |c| {
			c.amount_collected
				.ensure_add_assign(collected.amount_collected)?;
			c.amount_payment
				.ensure_add_assign(collected.amount_payment)?;
			Ok::<bool, DispatchError>(c.amount_collected.is_zero() && c.amount_payment.is_zero())
		})?;

		// No need to transition if nothing was collected
		if nothing_collect {
			return Ok(());
		}

		// Transition state to initiate swap from pool to foreign currency
		let pre_state = RedemptionState::<T>::get(who, investment_id);
		let amount_unprocessed_redemption = T::Investment::redemption(who, investment_id)?;
		// Amount needs to be denominated in foreign currency as it will be swapped into
		// foreign currency such that the swap order amount is in the incoming currency
		let amount_collected_foreign_denominated = T::CurrencyConverter::stable_to_stable(
			foreign_payout_currency,
			pool_currency,
			collected.amount_collected,
		)?;
		let post_state = pre_state
			.transition(RedeemTransition::CollectRedemption(
				amount_unprocessed_redemption,
				SwapOf::<T> {
					amount: amount_collected_foreign_denominated,
					currency_in: foreign_payout_currency,
					currency_out: pool_currency,
				},
			))
			.map_err(|e| {
				// Inner error holds finer granularity but should never occur
				log::debug!("RedeemState transition error: {:?}", e);
				Error::<T>::from(RedeemError::CollectTransition)
			})?;

		Pallet::<T>::apply_redeem_state_transition(who, investment_id, post_state)?;

		Ok(())
	}

	/// Sends `DecreasedForeignInvestOrderHook` notification such that any
	/// potential consumer could act upon that, e.g. Liquidity Pools for
	/// `ExecutedDecreaseInvestOrder`.
	#[transactional]
	pub(crate) fn notify_executed_decrease_invest(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_currency: T::CurrencyId,
		amount_decreased: T::Balance,
	) -> DispatchResult {
		let pool_currency = T::PoolInspect::currency_for(investment_id.of_pool())
			.expect("Pool must exist if decrease was executed; qed.");
		let amount_remaining_pool_denominated = T::Investment::investment(who, investment_id)?;
		let amount_remaining_foreign_denominated = T::CurrencyConverter::stable_to_stable(
			foreign_currency,
			pool_currency,
			amount_remaining_pool_denominated,
		)?;

		T::DecreasedForeignInvestOrderHook::notify_status_change(
			cfg_types::investments::ForeignInvestmentInfo::<T::AccountId, T::InvestmentId, ()> {
				owner: who.clone(),
				id: investment_id,
				// not relevant here
				last_swap_reason: None,
			},
			ExecutedForeignDecreaseInvest {
				amount_decreased,
				foreign_currency,
				amount_remaining: amount_remaining_foreign_denominated,
			},
		)
	}

	/// Consumes the `CollectedInvestment` amounts and
	/// `CollectedForeignInvestmentHook` notification such that any
	/// potential consumer could act upon that, e.g. Liquidity Pools for
	/// `ExecutedCollectInvest`.
	///
	/// NOTE: Converts the collected pool currency payment amount to foreign
	/// currency via the `CurrencyConverter` trait.
	pub(crate) fn notify_executed_collect_invest(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> DispatchResult {
		let foreign_payout_currency = InvestmentPaymentCurrency::<T>::get(who, investment_id)?;
		let pool_currency = T::PoolInspect::currency_for(investment_id.of_pool())
			.ok_or(Error::<T>::PoolNotFound)?;
		let collected = CollectedInvestment::<T>::take(who, investment_id);

		// Determine payout and remaining amounts in foreign currency instead of current
		// pool currency denomination
		let amount_currency_payout = T::CurrencyConverter::stable_to_stable(
			foreign_payout_currency,
			pool_currency,
			collected.amount_payment,
		)?;
		let remaining_amount_pool_denominated = T::Investment::investment(who, investment_id)?;
		let amount_remaining_invest_foreign_denominated = T::CurrencyConverter::stable_to_stable(
			foreign_payout_currency,
			pool_currency,
			remaining_amount_pool_denominated,
		)?;

		T::CollectedForeignInvestmentHook::notify_status_change(
			cfg_types::investments::ForeignInvestmentInfo::<T::AccountId, T::InvestmentId, ()> {
				owner: who.clone(),
				id: investment_id,
				// not relevant here
				last_swap_reason: None,
			},
			ExecutedForeignCollect {
				currency: foreign_payout_currency,
				amount_currency_payout,
				amount_tranche_tokens_payout: collected.amount_collected,
				amount_remaining: amount_remaining_invest_foreign_denominated,
			},
		)
	}

	/// Sends `CollectedForeignRedemptionHook` notification such that any
	/// potential consumer could act upon that, e.g. Liquidity Pools for
	/// `ExecutedCollectRedeem`.
	#[transactional]
	pub(crate) fn notify_executed_collect_redeem(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		currency: T::CurrencyId,
		collected: CollectedAmount<T::Balance>,
	) -> DispatchResult {
		T::CollectedForeignRedemptionHook::notify_status_change(
			cfg_types::investments::ForeignInvestmentInfo::<T::AccountId, T::InvestmentId, ()> {
				owner: who.clone(),
				id: investment_id,
				// not relevant here
				last_swap_reason: None,
			},
			ExecutedForeignCollect {
				currency,
				amount_currency_payout: collected.amount_collected,
				amount_tranche_tokens_payout: collected.amount_payment,
				amount_remaining: T::Investment::redemption(who, investment_id)?,
			},
		)
	}
}
