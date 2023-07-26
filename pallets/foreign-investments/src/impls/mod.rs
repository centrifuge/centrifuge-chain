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

use cfg_traits::{ForeignInvestment, Investment, StatusNotificationHook, TokenSwaps};
use cfg_types::investments::{ExecutedDecrease, InvestmentInfo};
use frame_support::{traits::Get, transactional};
use sp_runtime::{
	traits::{EnsureAdd, Zero},
	DispatchError, DispatchResult,
};

use crate::{
	types::{
		InnerRedeemState, InvestState, InvestTransition, RedeemState, RedeemTransition, Swap,
		TokenSwapReason,
	},
	Config, Error, ForeignInvestmentInfo, ForeignInvestmentInfoOf, InvestmentState, Pallet,
	RedemptionState, SwapOf, TokenSwapOrderIds, TokenSwapReasons,
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
				let post_state = pre_state
					.transition(InvestTransition::FulfillSwapOrder(status))
					.map_err(|e| {
						log::debug!(
							"Encountered unexpected pre state {:?} when transitioning into {:?} \
							 after (partially) fulfilling a swap",
							pre_state,
							status
						);
						e
					})?;
				Pallet::<T>::apply_state_transition(&info.owner, info.id, post_state.clone()).map(
					|e| {
						log::debug!(
							"Encountered unexpected error when applying state transition to state \
							 {:?}",
							post_state
						);
						e
					},
				)
			}
			TokenSwapReason::Redemption => {
				let pre_state = RedemptionState::<T>::get(&info.owner, info.id).unwrap_or_default();
				let post_state = pre_state
					.transition(RedeemTransition::FulfillSwapOrder(status))
					.map_err(|e| {
						log::debug!(
							"Encountered unexpected pre state {:?} when transitioning into {:?} \
							 after (partially) fulfilling a swap",
							pre_state,
							status
						);
						e
					})?;
				todo!()
			}
		}
	}
}

impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
	type Amount = T::Balance;
	type CurrencyId = T::CurrencyId;
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;

	// Consumers such as Connectors should call this function instead of
	// `Investment::update_invest_order` as this implementation accounts for
	// (potentially) splitting the update into two stages. The second stage is
	// resolved by `StatusNotificationHook::notify_status_change`.
	fn update_foreign_invest_order(
		who: &T::AccountId,
		return_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
	) -> Result<(), DispatchError> {
		let pre_amount = T::Investment::investment(who, investment_id.clone())?;
		let pre_state = InvestmentState::<T>::get(who, investment_id.clone()).unwrap_or_default();

		let post_state = if amount > pre_amount {
			pre_state.transition(InvestTransition::IncreaseInvestOrder(Swap {
				currency_in: pool_currency,
				currency_out: return_currency,
				// safe because amount > pre_amount
				amount: amount - pre_amount,
			}))
		} else if amount < pre_amount {
			pre_state.transition(InvestTransition::DecreaseInvestOrder(Swap {
				currency_in: return_currency,
				currency_out: pool_currency,
				// safe because amount < pre_amount
				amount: pre_amount - amount,
			}))
		} else {
			Ok(pre_state)
		}?;

		Pallet::<T>::apply_state_transition(who, investment_id, post_state)?;

		Ok(())
	}

	fn update_foreign_redemption(
		who: &T::AccountId,
		// return_currency: T::CurrencyId,
		// pool_currency: T::CurrencyId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
	) -> Result<(), DispatchError> {
		let pre_amount = T::Investment::redemption(who, investment_id.clone())?;
		let pre_state = RedemptionState::<T>::get(who, investment_id.clone()).unwrap_or_default();

		let post_state = if amount > pre_amount {
			// safe because amount > pre_amount
			pre_state.transition(RedeemTransition::IncreaseRedeemOrder(amount - pre_amount))
		} else if amount < pre_amount {
			// safe because amount < pre_amount
			pre_state.transition(RedeemTransition::DecreaseRedeemOrder(pre_amount - amount))
		} else {
			Ok(pre_state)
		}?;

		// Pallet::<T>::apply_state_transition(who, investment_id, post_state)?;

		Ok(())
	}

	fn collect_foreign_investment(
		who: &T::AccountId,
		return_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
		investment_id: T::InvestmentId,
	) -> Result<(), DispatchError> {
		todo!()
	}

	fn collect_foreign_redemption(
		who: &T::AccountId,
		return_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
		investment_id: T::InvestmentId,
	) -> Result<(), DispatchError> {
		todo!()
	}
}

impl<T: Config> Pallet<T> {
	/// Must be called after transitioning any `InvestState` via `transition` to
	/// update the chain storage and execute various trait config hooks, e.g.
	/// `ExecutedDecreaseHook`.
	///
	/// NOTE: When updating token swap orders, only `handle_swap_order` should
	/// be called!
	#[transactional]
	// TODO: Add/adjust apply_state_transition for redeem orders
	fn apply_state_transition(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		state: InvestState<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		match state.clone() {
			InvestState::NoState=> {
				Self::handle_swap_order(who, investment_id, None, TokenSwapReason::Investment)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;

				// Exit early to prevent setting InvestmentState
				InvestmentState::<T>::remove(who, investment_id);
				return Ok(());
			},
			InvestState::InvestmentOngoing { invest_amount } => {
				Self::handle_swap_order(who, investment_id, None, TokenSwapReason::Investment)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;
			},
			InvestState::ActiveSwapIntoPoolCurrency { swap } |
			InvestState::ActiveSwapIntoReturnCurrency { swap } |
			// We don't care about `done_amount` until swap into return is fulfilled
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => {
				Self::handle_swap_order(who, investment_id, Some(swap), TokenSwapReason::Investment)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount } |
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap, invest_amount } |
			// We don't care about `done_amount` until swap into return is fulfilled
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap,invest_amount, .. } => {
				Self::handle_swap_order(who, investment_id, Some(swap), TokenSwapReason::Investment)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap, done_amount } => {
				Self::handle_swap_order(who, investment_id, Some(swap.clone()), TokenSwapReason::Investment)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;

				Self::send_executed_decrease_hook(who, investment_id, done_amount)?;

				// Exit early to prevent setting InvestmentState
				let new_state = InvestState::ActiveSwapIntoPoolCurrency { swap };
				InvestmentState::<T>::insert(who, investment_id, new_state);
				return Ok(());
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap, done_amount, invest_amount } => {
				Self::handle_swap_order(who, investment_id, Some(swap.clone()), TokenSwapReason::Investment)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;

				Self::send_executed_decrease_hook(who, investment_id, done_amount)?;

				// Exit early to prevent setting InvestmentState
				let new_state = InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount };
				InvestmentState::<T>::insert(who, investment_id, new_state);
				return Ok(());
			},
			InvestState::SwapIntoReturnDone { done_swap } => {
				Self::handle_swap_order(who, investment_id, None, TokenSwapReason::Investment)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;

				Self::send_executed_decrease_hook(who, investment_id, done_swap.amount)?;

				// Exit early to prevent setting InvestmentState
				InvestmentState::<T>::remove(who, investment_id);
				return Ok(());
			},
			InvestState::SwapIntoReturnDoneAndInvestmentOngoing { done_swap, invest_amount } => {
				Self::handle_swap_order(who, investment_id, None, TokenSwapReason::Investment)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;

				Self::send_executed_decrease_hook(who, investment_id, done_swap.amount)?;

				// Exit early to prevent setting InvestmentState
				let new_state = InvestState::InvestmentOngoing { invest_amount };
				InvestmentState::<T>::insert(who, investment_id, new_state);
				return Ok(());
			},
		};

		InvestmentState::<T>::insert(who, investment_id, state);
		// TODO: Emit event?

		Ok(())
	}

	/// Updates or kills a token swap order. If the final swap amount is zero,
	/// kills the swap order and all associated storage. Else, creates or
	/// updates an existing swap order.
	///
	/// If the provided reason does not match the latest one stored in
	/// `TokenSwapReasons`, also resolves the _merge conflict_ resulting from
	/// updating and thus overwriting opposite swaps. See
	/// [Self::handle_concurrent_swap_orders] for details.
	///
	/// Returns potentially altered invest and redeem states which are not
	/// updated in storage yet!
	///
	/// NOTE: Should be the only swap order updating function which should be
	/// called.
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

			// update or kill swap order with updated order having priority in case it was
			// overwritten
			if let Some(swap_order) = maybe_updated_swap {
				Self::place_swap_order(who, investment_id, swap_order, reason)?;
			} else if let Some(swap_order) = maybe_swap {
				Self::place_swap_order(who, investment_id, swap_order, reason)?;
			} else {
				Self::kill_swap_order(who, investment_id)?;
			}

			Ok((maybe_invest_state, maybe_redeem_state))
		}
		// update to provided value, if not none
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
			T::TokenSwaps::cancel_order(swap_order_id);
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
		if let Some(swap_order_id) = TokenSwapOrderIds::<T>::get(who, investment_id) {
			T::TokenSwaps::update_order(
				who.clone(),
				swap_order_id,
				swap.amount,
				T::DefaultTokenSwapSellPriceLimit::get(),
				T::DefaultTokenMinFulfillmentAmount::get(),
			)?;
			TokenSwapReasons::<T>::insert(swap_order_id, reason);

			Ok(())
		} else {
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

			Ok(())
		}
	}

	/// Determines the correct amount for a token swap based on the current
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
			state => Ok(None),
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

	/// Sends `ExecutedDecreaseHook` notification such that any potential
	/// consumer could act upon that, e.g. Connectors for
	/// `ExecutedDecrease{Invest, Redeem}Order`.
	fn send_executed_decrease_hook(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount_payout: T::Balance,
	) -> DispatchResult {
		// TODO(@mustermeiszer): Does this return the entire desired amount or do we
		// need to tap into collecting?
		let amount_remaining = T::Investment::investment(who, investment_id)?;

		// TODO(@mustermeiszer): Do we add the active swap amount?
		T::ExecutedDecreaseHook::notify_status_change(
			ForeignInvestmentInfoOf::<T> {
				owner: who.clone(),
				id: investment_id,
			},
			ExecutedDecrease {
				amount_payout,
				amount_remaining,
			},
		)
	}
}
