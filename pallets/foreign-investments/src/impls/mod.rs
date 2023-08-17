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

use cfg_traits::{
	investments::{ForeignInvestment, Investment, InvestmentCollector},
	StatusNotificationHook, TokenSwaps,
};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollectInvest, ExecutedForeignCollectRedeem,
	ExecutedForeignDecrease, Swap,
};
use frame_support::{traits::Get, transactional};
use sp_runtime::{
	traits::{EnsureAdd, EnsureAddAssign, Zero},
	DispatchError, DispatchResult,
};

use crate::{
	types::{
		InnerRedeemState, InvestState, InvestTransition, RedeemState, RedeemTransition,
		TokenSwapReason,
	},
	CollectedRedemptionTrancheTokens, Config, Error, Event, ForeignInvestmentInfo,
	ForeignInvestmentInfoOf, InvestmentState, Pallet, RedemptionState, SwapOf, TokenSwapOrderIds,
	TokenSwapReasons,
};

mod invest;
mod redeem;

// Handles the second stage of updating investments. Whichever (potentially
// async) code path of the first stage concludes it (partially) should call
// `Swap::Config::SwapNotificationHandler::notify_status_update(swap_order_id,
// swapped_amount)`.
impl<T: Config> StatusNotificationHook for Pallet<T> {
	type Error = DispatchError;
	type Id = T::TokenSwapOrderId;
	type Status = SwapOf<T>;

	fn notify_status_change(
		id: T::TokenSwapOrderId,
		status: SwapOf<T>,
	) -> Result<(), DispatchError> {
		let info = ForeignInvestmentInfo::<T>::get(id).ok_or(Error::<T>::InvestmentInfoNotFound)?;
		let reason = TokenSwapReasons::<T>::get(id).ok_or(Error::<T>::TokenSwapReasonNotFound)?;

		match reason {
			TokenSwapReason::Investment => {
				let pre_state = InvestmentState::<T>::get(&info.owner, info.id).unwrap_or_default();
				let post_state =
					pre_state.transition(InvestTransition::FulfillSwapOrder(status))?;
				Pallet::<T>::apply_invest_state_transition(&info.owner, info.id, post_state)
			}
			TokenSwapReason::Redemption => {
				let pre_state = RedemptionState::<T>::get(&info.owner, info.id).unwrap_or_default();
				let post_state =
					pre_state.transition(RedeemTransition::FulfillSwapOrder(status))?;
				Pallet::<T>::apply_redeem_state_transition(&info.owner, info.id, post_state)
			}
		}
	}
}

impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
	type Amount = T::Balance;
	type CollectInvestResult = ExecutedForeignCollectInvest<T::Balance>;
	type CurrencyId = T::CurrencyId;
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;

	#[transactional]
	fn increase_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
		return_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
	) -> Result<(), DispatchError> {
		let pre_state = InvestmentState::<T>::get(who, investment_id.clone()).unwrap_or_default();
		let post_state = pre_state.transition(InvestTransition::IncreaseInvestOrder(Swap {
			currency_in: pool_currency,
			currency_out: return_currency,
			amount,
		}))?;

		Pallet::<T>::apply_invest_state_transition(who, investment_id, post_state)?;

		Ok(())
	}

	#[transactional]
	fn decrease_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
		return_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
	) -> Result<(), DispatchError> {
		let pre_state = InvestmentState::<T>::get(who, investment_id.clone()).unwrap_or_default();
		let post_state = pre_state.transition(InvestTransition::DecreaseInvestOrder(Swap {
			currency_in: pool_currency,
			currency_out: return_currency,
			amount,
		}))?;

		Pallet::<T>::apply_invest_state_transition(who, investment_id, post_state)?;

		Ok(())
	}

	#[transactional]
	fn increase_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
	) -> Result<(), DispatchError> {
		let pre_state = RedemptionState::<T>::get(who, investment_id.clone()).unwrap_or_default();
		let post_state = pre_state.transition(RedeemTransition::IncreaseRedeemOrder(amount))?;

		Pallet::<T>::apply_redeem_state_transition(who, investment_id, post_state)?;

		Ok(())
	}

	#[transactional]
	fn decrease_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		// TODO: Check if we can drop the below line
		let pre_amount = T::Investment::redemption(who, investment_id.clone())?;
		let pre_state = RedemptionState::<T>::get(who, investment_id.clone()).unwrap_or_default();
		let post_state = pre_state.transition(RedeemTransition::DecreaseRedeemOrder(amount))?;

		Pallet::<T>::apply_redeem_state_transition(who, investment_id, post_state)?;

		Ok(amount)
	}

	#[transactional]
	fn collect_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<ExecutedForeignCollectInvest<T::Balance>, DispatchError> {
		// No need to transition or update state as collection of tranche tokens is
		// independent of the current `InvestState`
		let CollectedAmount::<T::Balance> {
			amount_collected,
			amount_payment,
		} = T::Investment::collect_investment(who.clone(), investment_id)?;

		Ok(ExecutedForeignCollectInvest {
			// TODO: Translate from `pool_currency` to `return_currency`
			amount_currency_payout: amount_payment,
			amount_tranche_tokens_payout: amount_collected,
		})
	}

	#[transactional]
	fn collect_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		return_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
	) -> Result<(), DispatchError> {
		let collected = T::Investment::collect_redemption(who.clone(), investment_id)?;
		CollectedRedemptionTrancheTokens::<T>::try_mutate(who, investment_id, |amount| {
			amount.ensure_add_assign(collected.amount_payment)?;

			Ok::<(), DispatchError>(())
		})?;

		// Transition state to initiate swap from pool to return currency
		let pre_state = RedemptionState::<T>::get(who, investment_id.clone()).unwrap_or_default();
		let post_state = pre_state.transition(RedeemTransition::Collect(SwapOf::<T> {
			amount: collected.amount_collected,
			currency_in: return_currency,
			currency_out: pool_currency,
		}))?;

		Pallet::<T>::apply_redeem_state_transition(who, investment_id, post_state)?;

		Ok(())
	}

	fn investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		// TODO: Needs to be translated from `pool_currency` to `return_currency`
		T::Investment::investment(who, investment_id)
	}

	fn redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		// TODO: Needs to be translated from `pool_currency` to `return_currency`
		T::Investment::redemption(who, investment_id)
	}

	fn accepted_payment_currency(investment_id: T::InvestmentId, currency: T::CurrencyId) -> bool {
		// TODO(future): If this returns false, we should add a mechanism which checks
		// whether `currency` can be swapped into an accepted payment currency.
		//
		// This requires
		//   * Querying all accepted payment currencies of an investment
		//   * Checking whether there are orders from `currency` into an accepted
		//     payment currency
		T::Investment::accepted_payment_currency(investment_id, currency)
	}

	fn accepted_payout_currency(investment_id: T::InvestmentId, currency: T::CurrencyId) -> bool {
		// TODO(future): If this returns false, we should add a mechanism which checks
		// whether any of the accepted `payout` currencies can be swapped into
		// `currency`.
		//
		// This requires
		//   * Querying all accepted payout currencies of an investment
		//   * Checking whether there are orders from an accepted payout currency into
		//     `currency`
		T::Investment::accepted_payout_currency(investment_id, currency)
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
	/// 1. If the `InvestState` includes `SwapIntoReturnDone` without
	/// `ActiveSwapIntoReturnCurrency`: Send executed decrease hook & transition
	/// state into its form without `SwapIntoReturnDone`. If the state is just
	/// `SwapIntoReturnDone`, kill it.
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
	/// `RedemptionState` again. If the corresponding new `InnerRedeemState`
	/// includes `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency`,
	/// remove the `SwapIntoReturnDone` part or kill it. Additionally, emit
	/// `ForeignRedemptionUpdate` or `ForeignRedemptionCleared`.
	///
	/// 6. Update the investment. This also includes setting it to zero. We
	/// assume the impl of `<T as Config>::Investment` handles this case.
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
		state: InvestState<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		// Do first round of updates and forward state, swap as well as invest amount
		match state.clone() {
			InvestState::NoState => {
				InvestmentState::<T>::remove(who, investment_id);

				Ok((InvestState::NoState, None, Zero::zero()))
			},
			InvestState::InvestmentOngoing { invest_amount } => {
				InvestmentState::<T>::insert(who, investment_id, state.clone());

				Ok((state, None, invest_amount))
			},
			InvestState::ActiveSwapIntoPoolCurrency { swap } |
			InvestState::ActiveSwapIntoReturnCurrency { swap } |
			// We don't care about `done_amount` until swap into return is fulfilled
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => {
				InvestmentState::<T>::insert(who, investment_id, state.clone());
				Ok((state, Some(swap), Zero::zero()))
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount } |
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap, invest_amount } |
			// We don't care about `done_amount` until swap into return is fulfilled
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap,invest_amount, .. } => {
				InvestmentState::<T>::insert(who, investment_id, state.clone());
				Ok((state, Some(swap), invest_amount))
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap, done_amount } => {
				Self::notify_executed_decrease_invest(who, investment_id, done_amount, swap.currency_out)?;

				let new_state = InvestState::ActiveSwapIntoPoolCurrency { swap };
				InvestmentState::<T>::insert(who, investment_id, new_state.clone());

				Ok((new_state, Some(swap), Zero::zero()))
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap, done_amount, invest_amount } => {
				Self::notify_executed_decrease_invest(who, investment_id, done_amount, swap.currency_out)?;

				let new_state = InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount };
				InvestmentState::<T>::insert(who, investment_id, new_state.clone());

				Ok((new_state, Some(swap), invest_amount))
			},
			InvestState::SwapIntoReturnDone { done_swap } => {
				Self::notify_executed_decrease_invest(who, investment_id, done_swap.amount, done_swap.currency_in)?;

				InvestmentState::<T>::remove(who, investment_id);

				Ok((InvestState::NoState, None, Zero::zero()))
			},
			InvestState::SwapIntoReturnDoneAndInvestmentOngoing { done_swap, invest_amount } => {
				Self::notify_executed_decrease_invest(who, investment_id, done_swap.amount, done_swap.currency_in)?;

				let new_state = InvestState::InvestmentOngoing { invest_amount };
				InvestmentState::<T>::insert(who, investment_id, new_state.clone());

				Ok((new_state, None, invest_amount))
			},
		}
		.map(|(invest_state, maybe_swap, invest_amount)| {
			let (maybe_invest_state_prio, maybe_new_redeem_state) = Self::handle_swap_order(who, investment_id, maybe_swap,  TokenSwapReason::Investment)?;

			// Dispatch transition event, post swap state has priority if it exists as it was the last transition
			if let Some(invest_state_prio) = maybe_invest_state_prio {
				Self::deposit_investment_event(who, investment_id, Some(invest_state_prio));
			} else {
				Self::deposit_investment_event(who, investment_id, Some(invest_state));
			}
			Self::deposit_redemption_event(who, investment_id, maybe_new_redeem_state);

			// Finally, update investment after all states have been updated
			T::Investment::update_investment(who, investment_id, invest_amount)?;

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
	/// 1. If the `RedeemState` includes `SwapIntoReturnDone` without
	/// `ActiveSwapIntoReturnCurrency`, remove the `SwapIntoReturnDone` part or
	/// kill it.
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
	/// `RedemptionState` again. If the corresponding new `InnerRedeemState`
	/// includes `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency`,
	/// remove the `SwapIntoReturnDone` part or kill it. Additionally, emit
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
	/// * Must be called after transitionin g any `RedeemState` via
	/// `transition` to update the chain storage.
	/// * When updating token swap orders, only `handle_swap_order` should
	/// be called.
	#[transactional]
	pub(crate) fn apply_redeem_state_transition(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		state: RedeemState<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let invest_amount = state.get_invest_amount().unwrap_or_default();

		// Do first round of updates and forward state as well as swap
		match state.clone() {
			RedeemState::NoState => {
				RedemptionState::<T>::remove(who, investment_id);
				Ok((Some(RedeemState::NoState), None))
			}
			RedeemState::Invested { .. } => {
				RedemptionState::<T>::insert(who, investment_id, state);
				Ok((Some(state), None))
			}
			RedeemState::InvestedAnd { inner, .. } | RedeemState::NotInvestedAnd { inner } => {
				match inner {
					InnerRedeemState::Redeeming { .. } |
					InnerRedeemState::RedeemingAndCollectableRedemption { .. } |
					InnerRedeemState::CollectableRedemption => {
						RedemptionState::<T>::insert(who, investment_id, state);
						Ok((Some(state), None))
					},
					InnerRedeemState::RedeemingAndActiveSwapIntoReturnCurrency { swap, .. } |
					InnerRedeemState::RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } |
					InnerRedeemState::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { swap, .. } |
					InnerRedeemState::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } |
					InnerRedeemState::ActiveSwapIntoReturnCurrency { swap, .. } |
					InnerRedeemState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } |
					InnerRedeemState::CollectableRedemptionAndActiveSwapIntoReturnCurrency { swap, .. } |
					InnerRedeemState::CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => {
						RedemptionState::<T>::insert(who, investment_id, state);
						Ok((Some(state), Some(swap)))
					},
					// Only states left include `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency` such that we can notify collect
					inner => {
						let maybe_new_state = Self::apply_collect_redeem_transition(who, investment_id, state, inner)?;
						Ok((maybe_new_state, None))
					}
				}
			}
		}
		.map(|(maybe_new_state, maybe_swap)| {
			let (maybe_new_invest_state, maybe_new_state_prio) = Self::handle_swap_order(
				who,
				investment_id,
				maybe_swap,
				TokenSwapReason::Redemption,
			)?;

			// Dispatch transition event, post swap state has priority if it exists as it was the last transition
			if let Some(redeem_state_post_swap) = maybe_new_state_prio {
				Self::deposit_redemption_event(who, investment_id, Some(redeem_state_post_swap));
			} else {
				Self::deposit_redemption_event(who, investment_id, maybe_new_state);
			}
			Self::deposit_investment_event(who, investment_id, maybe_new_invest_state);

			// Finally, update redemption after all states have been updated
			T::Investment::update_redemption(who, investment_id, invest_amount)?;

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
		maybe_state: Option<InvestState<T::Balance, T::CurrencyId>>,
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

	/// Terminates a redeem collection which required swapping into return
	/// currency.
	///
	/// Only acts upon inner redeem states which include `SwapIntoReturnDone`
	/// without `ActiveSwapIntoReturnCurrency`. Other inner states are ignored.
	/// Either updates the corresponding `RedemptionState` or drops it entirely.
	///
	/// Emits `notify_executed_collect_redeem`.
	///
	/// Returning...
	/// * `Some(RedeemState::NoState)` indicates a `ForeignRedemptionCleared`
	///   event can be deposited
	/// * `Some(state)` indicates a `ForeignRedemptionUpdated` event can be
	///   deposited
	/// * `None` indicates no state mutation occurred	///
	#[transactional]
	fn apply_collect_redeem_transition(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		state: RedeemState<T::Balance, T::CurrencyId>,
		inner_redeem_state: InnerRedeemState<T::Balance, T::CurrencyId>,
	) -> Result<Option<RedeemState<T::Balance, T::CurrencyId>>, DispatchError> {
		let amount_payment_tranche_tokens =
			CollectedRedemptionTrancheTokens::<T>::get(who, investment_id);

		// Send notification and kill `CollectedRedemptionTrancheTokens` iff the state
		// includes `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency`
		match inner_redeem_state {
			InnerRedeemState::SwapIntoReturnDone { done_swap, .. }
			| InnerRedeemState::RedeemingAndSwapIntoReturnDone { done_swap, .. }
			| InnerRedeemState::RedeemingAndCollectableRedemptionAndSwapIntoReturnDone {
				done_swap,
				..
			}
			| InnerRedeemState::CollectableRedemptionAndSwapIntoReturnDone { done_swap, .. } => {
				Self::notify_executed_collect_redeem(
					who,
					investment_id,
					done_swap.currency_in,
					CollectedAmount {
						amount_collected: done_swap.amount,
						amount_payment: amount_payment_tranche_tokens,
					},
				)?;
				CollectedRedemptionTrancheTokens::<T>::remove(who, investment_id);
				Ok(())
			}
			_ => Ok(()),
		}
		.map_err(|e: DispatchError| e)?;

		// Update state iff the state includes `SwapIntoReturnDone` without
		// `ActiveSwapIntoReturnCurrency`
		match inner_redeem_state {
			InnerRedeemState::SwapIntoReturnDone { .. } => {
				RedemptionState::<T>::remove(who, investment_id);
				Ok(Some(RedeemState::NoState))
			}
			InnerRedeemState::RedeemingAndSwapIntoReturnDone { redeem_amount, .. } => {
				let new_state =
					state.swap_inner_state(InnerRedeemState::Redeeming { redeem_amount });
				RedemptionState::<T>::insert(who, investment_id, new_state);
				Ok(Some(new_state))
			}
			InnerRedeemState::RedeemingAndCollectableRedemptionAndSwapIntoReturnDone {
				redeem_amount,
				..
			} => {
				let new_state =
					state.swap_inner_state(InnerRedeemState::RedeemingAndCollectableRedemption {
						redeem_amount,
					});
				RedemptionState::<T>::insert(who, investment_id, new_state);
				Ok(Some(new_state))
			}
			InnerRedeemState::CollectableRedemptionAndSwapIntoReturnDone { .. } => {
				let new_state = state.swap_inner_state(InnerRedeemState::CollectableRedemption);
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
	/// `TokenSwapReasons`, also resolves the _merge conflict_ resulting from
	/// updating and thus overwriting opposite swaps. See
	/// [Self::handle_concurrent_swap_orders] for details. If this results in
	/// either an altered invest state and/or an altered redeem state, the
	/// corresponding storage is updated and the new states returned. The latter
	/// is required for emitting events.
	///
	/// NOTE: Must not call any other swap order updating function.
	#[transactional]
	fn handle_swap_order(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		maybe_swap: Option<SwapOf<T>>,
		reason: TokenSwapReason,
	) -> Result<
		(
			Option<InvestState<T::Balance, T::CurrencyId>>,
			Option<RedeemState<T::Balance, T::CurrencyId>>,
		),
		DispatchError,
	> {
		// check for concurrent conflicting swap orders
		if let Some(swap_order_id) = TokenSwapOrderIds::<T>::get(who, investment_id) {
			let (maybe_updated_swap, maybe_invest_state, maybe_redeem_state) =
				Self::handle_concurrent_swap_orders(who, investment_id, swap_order_id, reason)?;

			// Update or kill swap order with updated order having priority in case it was
			// overwritten
			if let Some(swap_order) = maybe_updated_swap {
				Self::place_swap_order(who, investment_id, swap_order, reason)?;
			} else if let Some(swap_order) = maybe_swap {
				Self::place_swap_order(who, investment_id, swap_order, reason)?;
			} else {
				Self::kill_swap_order(who, investment_id)?;
			}

			// Update invest and redeem states if necessary
			InvestmentState::<T>::mutate(who, investment_id, |current_invest_state| {
				// Should never occur but let's be safe
				if let Some(InvestState::NoState) = maybe_invest_state {
					*current_invest_state = None;
				} else if maybe_invest_state != *current_invest_state {
					*current_invest_state = maybe_invest_state.clone();
				}
			});

			// Need to check if `SwapReturnDone` is part of inner state without
			// `ActiveSwapIntoReturnCurrency` as this implies the successful termination of
			// a collect (with swap into return currency). If this is the case, the returned
			// redeem state needs to be updated as well.
			match maybe_redeem_state {
				Some(RedeemState::InvestedAnd { inner, .. })
				| Some(RedeemState::NotInvestedAnd { inner }) => {
					let redeem_state = maybe_redeem_state.unwrap_or_default();
					let maybe_collected_redeem_state = Self::apply_collect_redeem_transition(
						who,
						investment_id,
						redeem_state,
						inner,
					)?;
					Ok((maybe_invest_state, maybe_collected_redeem_state))
				}
				Some(_) => {
					RedemptionState::<T>::mutate(who, investment_id, |current_redeem_state| {
						// Update and emit event on mismatch
						if maybe_redeem_state != *current_redeem_state {
							*current_redeem_state = maybe_redeem_state;
						}
					});
					Ok((maybe_invest_state, maybe_redeem_state))
				}
				None => Ok((maybe_invest_state, maybe_redeem_state)),
			}
			.map_err(|e: DispatchError| e)
		}
		// Update to provided value, if not none
		else if let Some(swap_order) = maybe_swap {
			Self::place_swap_order(who, investment_id, swap_order, reason)?;
			Ok((None, None))
		} else {
			Ok((None, None))
		}
	}

	/// Kills all storage associated with token swaps and cancels the
	/// potentially active swap order.
	///
	/// NOTE: Must only be called in `handle_swap_order`.
	fn kill_swap_order(who: &T::AccountId, investment_id: T::InvestmentId) -> DispatchResult {
		if let Some(swap_order_id) = TokenSwapOrderIds::<T>::take(who, investment_id) {
			T::TokenSwaps::cancel_order(swap_order_id)?;
			ForeignInvestmentInfo::<T>::remove(swap_order_id);
			TokenSwapReasons::<T>::remove(swap_order_id);
		}
		Ok(())
	}

	/// Sets up `TokenSwapOrderIds` and `ForeignInvestmentInfo` storages, if the
	/// order does not exist yet. Moreover, updates `TokenSwapReasons` pointer
	/// to the provided value.
	///
	/// NOTE: Must only be called in `handle_swap_order`.
	#[transactional]
	fn place_swap_order(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swap: SwapOf<T>,
		reason: TokenSwapReason,
	) -> DispatchResult {
		// exit early
		if swap.amount.is_zero() {
			return Self::kill_swap_order(who, investment_id);
		}
		match TokenSwapOrderIds::<T>::get(who, investment_id) {
			Some(swap_order_id) if T::TokenSwaps::is_active(swap_order_id) => {
				T::TokenSwaps::update_order(
					who.clone(),
					swap_order_id,
					swap.amount,
					T::DefaultTokenSwapSellPriceLimit::get(),
					T::DefaultTokenMinFulfillmentAmount::get(),
				)?;
				TokenSwapReasons::<T>::insert(swap_order_id, reason);
			}
			_ => {
				// TODO: How to handle potential failure?
				let swap_order_id = T::TokenSwaps::place_order(
					who.clone(),
					swap.currency_out,
					swap.currency_in,
					swap.amount,
					T::DefaultTokenSwapSellPriceLimit::get(),
					T::DefaultTokenMinFulfillmentAmount::get(),
				)?;
				TokenSwapOrderIds::<T>::insert(who, investment_id, swap_order_id);
				ForeignInvestmentInfo::<T>::insert(
					swap_order_id,
					ForeignInvestmentInfoOf::<T> {
						owner: who.clone(),
						id: investment_id,
					},
				);
				TokenSwapReasons::<T>::insert(swap_order_id, reason);
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
	/// `RedeemState` transition, a token swap order into return currency is
	/// needed. This handler resolves the _merge conflict_ in situations where
	/// the reason to create/update a swap order does not match the previous
	/// reason.
	///
	/// * Is noop, if the the current reason equals the previous one.
	/// * If both states are swapping into return currency, i.e. their invest
	///   and redeem states include `ActiveSwapIntoReturnCurrency`, the states
	///   stay the same. However the total order amount needs to be updated by
	///   summing up both swap order amounts.
	/// * If the `InvestState` includes swapping into pool currency, i.e.
	///   `ActiveSwapIntoPoolCurrency`, whereas the `RedeemState` is swapping
	///   into the opposite direction, i.e. `ActiveSwapIntoReturnCurrency`, we
	///   need to resolve the delta between both swap order amounts and update
	///   the states accordingly.
	fn handle_concurrent_swap_orders(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swap_order_id: T::TokenSwapOrderId,
		reason: TokenSwapReason,
	) -> Result<
		(
			Option<Swap<T::Balance, T::CurrencyId>>,
			Option<InvestState<T::Balance, T::CurrencyId>>,
			Option<RedeemState<T::Balance, T::CurrencyId>>,
		),
		DispatchError,
	> {
		let last_reason =
			TokenSwapReasons::<T>::get(swap_order_id).ok_or(Error::<T>::TokenSwapReasonNotFound)?;

		// Exit early if both reasons match, i.e. we would not override any opposite
		// swap order
		if last_reason == reason {
			return Ok((None, None, None));
		}

		// Read states from storage and determine amounts
		let invest_state = InvestmentState::<T>::get(who, investment_id).unwrap_or_default();
		let redeem_state = RedemptionState::<T>::get(who, investment_id).unwrap_or_default();
		let invest_swap_amount = invest_state
			.get_active_swap()
			.map(|s| s.amount)
			.unwrap_or_default();
		let redeem_swap_amount = redeem_state
			.get_active_swap()
			.map(|s| s.amount)
			.unwrap_or_default();
		let resolved_amount = invest_swap_amount.min(redeem_swap_amount);
		// safe because max >= min, equals zero if both amounts equal
		let swap_amount_opposite_direction =
			invest_swap_amount.max(redeem_swap_amount) - resolved_amount;

		// Determine new invest state
		let new_invest_state = match invest_state.clone() {
			// As redeem swap can only be into return currency, we need to delta on the opposite
			// swap directions
			InvestState::ActiveSwapIntoPoolCurrency { swap } => {
				if invest_swap_amount > redeem_swap_amount {
					Ok(Some(
						InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
							swap: Swap {
								amount: swap_amount_opposite_direction,
								..swap
							},
							invest_amount: resolved_amount,
						},
					))
				} else {
					Ok(Some(InvestState::InvestmentOngoing {
						invest_amount: resolved_amount,
					}))
				}
			}
			// Same as above except for the base investment amount which is incremented
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
				swap: invest_swap,
				invest_amount,
			} => {
				if invest_swap_amount > redeem_swap_amount {
					Ok(Some(
						InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
							swap: Swap {
								amount: swap_amount_opposite_direction,
								..invest_swap
							},
							invest_amount: invest_amount.ensure_add(resolved_amount)?,
						},
					))
				} else {
					Ok(Some(InvestState::InvestmentOngoing {
						invest_amount: invest_amount.ensure_add(resolved_amount)?,
					}))
				}
			}
			// We must not alter the invest state if there is no active pool currency swap
			_ => Ok(None),
		}
		.map_err(|e: DispatchError| e)?;

		// Determine final swap amount and new redeem state
		let (final_swap_amount, new_redeem_state) = match invest_state.clone() {
			// Opposite swaps cancel out at least one (or if equal amounts) both swaps
			InvestState::ActiveSwapIntoPoolCurrency { .. }
			| InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { .. } => {
				let new_state = redeem_state
					.fulfill_active_swap_amount(
						redeem_swap_amount.min(swap_amount_opposite_direction),
					)
					.unwrap_or(redeem_state);

				Ok((swap_amount_opposite_direction, new_state))
			}
			// All leftover combinations either do not involve any active swaps or both
			// swaps have the same direction, i.e. into return currency. Thus, we can
			// leave states untouched and just add up the potential swap amount.
			_ => Ok((
				invest_swap_amount.ensure_add(redeem_swap_amount)?,
				redeem_state,
			)),
		}
		.map(|(token_swap_amount, maybe_new_redeem_state)| {
			// If old state match new state, no need to return it as this could cause a
			// follow-up transition trigger
			if redeem_state == maybe_new_redeem_state {
				(token_swap_amount, None)
			} else {
				(token_swap_amount, Some(maybe_new_redeem_state))
			}
		})
		.map_err(|e: DispatchError| e)?;

		// Determine token swap from amount
		let token_swap = if invest_swap_amount > redeem_swap_amount {
			invest_state.get_active_swap().map(|invest_swap| Swap {
				amount: final_swap_amount,
				..invest_swap
			})
		}
		// handle redeem_swap_amount >= invest_swap_amount as well as all cases, in which neither
		// states include an active swap
		else {
			redeem_state.get_active_swap().map(|redeem_swap| Swap {
				amount: final_swap_amount,
				..redeem_swap
			})
		};

		Ok((token_swap, new_invest_state, new_redeem_state))
	}

	/// Sends `ExecutedDecreaseInvestHook` notification such that any potential
	/// consumer could act upon that, e.g. Liquidity Pools for
	/// `ExecutedDecreaseInvestOrder`.
	#[transactional]
	fn notify_executed_decrease_invest(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount_decreased: T::Balance,
		return_currency: T::CurrencyId,
	) -> DispatchResult {
		T::ExecutedDecreaseInvestHook::notify_status_change(
			ForeignInvestmentInfoOf::<T> {
				owner: who.clone(),
				id: investment_id,
			},
			ExecutedForeignDecrease {
				amount_decreased,
				return_currency,
			},
		)
	}

	/// Sends `ExecutedCollectRedeemHook` notification such that any potential
	/// consumer could act upon that, e.g. Liquidity Pools for
	/// `ExecutedCollectRedeemOrder`.
	#[transactional]
	fn notify_executed_collect_redeem(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		currency: T::CurrencyId,
		collected: CollectedAmount<T::Balance>,
	) -> DispatchResult {
		T::ExecutedCollectRedeemHook::notify_status_change(
			ForeignInvestmentInfoOf::<T> {
				owner: who.clone(),
				id: investment_id,
			},
			ExecutedForeignCollectRedeem {
				currency,
				amount_currency_payout: collected.amount_collected,
				amount_tranche_tokens_payout: collected.amount_payment,
			},
		)
	}
}
