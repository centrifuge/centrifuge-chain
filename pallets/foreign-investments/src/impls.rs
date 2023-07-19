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
	traits::{EnsureAdd, EnsureSub, Zero},
	ArithmeticError, DispatchError, DispatchResult,
};

use crate::{
	pallet,
	types::{InvestState, InvestTransition, Swap},
	Config, Error, ForeignInvestmentInfo, ForeignInvestmentInfoOf, InvestmentState, Pallet, SwapOf,
	TokenSwapOrderIds,
};

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
		let pre_state = InvestmentState::<T>::get(info.owner, info.id).unwrap_or_default();

		match status.currency_in {
			pool_currency if T::Investment::accepted_payment_currency(info.id, pool_currency) => {
				pre_state
					.transition(InvestTransition::SwapIntoPool(status))
					.map(|_| ())
			}
			return_currency
				if T::Investment::accepted_payout_currency(info.id, return_currency) =>
			{
				pre_state
					.transition(InvestTransition::SwapIntoReturn(status))
					.map(|_| ())
			}
			_ => Err(Error::<T>::InvalidInvestmentCurrency.into()),
		}
	}
}

impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
	type Amount = T::Balance;
	type CurrencyId = T::CurrencyId;
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;
	type SwapNotification = Pallet<T>;

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

		// TODO: Add check for same currencies, i.e. no swap required
		if amount > pre_amount {
			let post_state = pre_state.transition(InvestTransition::IncreaseInvestOrder(Swap {
				currency_in: pool_currency,
				currency_out: return_currency,
				amount,
			}))?;
			Ok(())
		} else if amount < pre_amount {
			let post_state = pre_state.transition(InvestTransition::DecreaseInvestOrder(Swap {
				currency_in: return_currency,
				currency_out: pool_currency,
				amount,
			}))?;
			Ok(())
		} else {
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Updates chain storage after applying state transition of `InvestState`
	/// and execute `ExecutedDecreaseHook`.
	#[transactional]
	fn apply_state_transition(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		state: InvestState<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		match state.clone() {
			InvestState::NoState=> {
				Self::kill_swap_order(who, investment_id)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;

				// Exit early to prevent setting InvestmentState
				InvestmentState::<T>::remove(who, investment_id);
				return Ok(());
			},
			InvestState::InvestmentOngoing { invest_amount } => {
				Self::kill_swap_order(who, investment_id)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;
			},
			InvestState::ActiveSwapIntoPoolCurrency { swap } |
			InvestState::ActiveSwapIntoReturnCurrency { swap } |
			// We don't care about `done_amount` until swap into return is fulfilled
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => {
				Self::place_swap_order(who, investment_id, swap)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount } |
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap, invest_amount } |
			// We don't care about `done_amount` until swap into return is fulfilled
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap,invest_amount, .. } => {
				Self::place_swap_order(who, investment_id, swap)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap, done_amount } => {
				Self::place_swap_order(who, investment_id, swap.clone())?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;

				Self::send_executed_decrease_hook(who, investment_id, done_amount)?;

				// Exit early to prevent setting InvestmentState
				let new_state = InvestState::ActiveSwapIntoPoolCurrency { swap };
				InvestmentState::<T>::insert(who, investment_id, new_state);
				return Ok(());
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap, done_amount, invest_amount } => {
				Self::place_swap_order(who, investment_id, swap.clone())?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;

				Self::send_executed_decrease_hook(who, investment_id, done_amount)?;

				// Exit early to prevent setting InvestmentState
				let new_state = InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount };
				InvestmentState::<T>::insert(who, investment_id, new_state);
				return Ok(());
			},
			InvestState::SwapIntoReturnDone { swap } => {
				Self::kill_swap_order(who, investment_id)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;

				Self::send_executed_decrease_hook(who, investment_id, swap.amount)?;

				// Exit early to prevent setting InvestmentState
				InvestmentState::<T>::remove(who, investment_id);
				return Ok(());
			},
			InvestState::SwapIntoReturnDoneAndInvestmentOngoing { swap, invest_amount } => {
				Self::kill_swap_order(who, investment_id)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;

				Self::send_executed_decrease_hook(who, investment_id, swap.amount)?;

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

	/// Kills all storage associated with token swaps and cancels the
	/// potentially active swap order.
	fn kill_swap_order(who: &T::AccountId, investment_id: T::InvestmentId) -> DispatchResult {
		if let Some(swap_order_id) = TokenSwapOrderIds::<T>::take(who, investment_id) {
			T::TokenSwaps::cancel_order(swap_order_id);
			ForeignInvestmentInfo::<T>::remove(swap_order_id);
		}
		Ok(())
	}

	/// Initializes or updates an existing swap order.
	///
	/// Sets up `TokenSwapOrderIds` and `ForeignInvestmentInfo` storages, if the
	/// order does not exist yet.
	fn place_swap_order(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swap: SwapOf<T>,
	) -> DispatchResult {
		if let Some(swap_order_id) = TokenSwapOrderIds::<T>::get(who, investment_id) {
			T::TokenSwaps::update_order(
				who.clone(),
				swap_order_id,
				swap.amount,
				T::DefaultTokenSwapSellPriceLimit::get(),
				T::DefaultTokenMinFulfillmentAmount::get(),
			)
		} else {
			// TODO: How to handle potential failure?
			let order_id = T::TokenSwaps::place_order(
				who.clone(),
				swap.currency_out,
				swap.currency_in,
				swap.amount,
				T::DefaultTokenSwapSellPriceLimit::get(),
				T::DefaultTokenMinFulfillmentAmount::get(),
			)?;
			TokenSwapOrderIds::<T>::insert(who, investment_id, order_id);
			ForeignInvestmentInfo::<T>::insert(
				order_id,
				ForeignInvestmentInfoOf::<T> {
					owner: who.clone(),
					id: investment_id,
				},
			);
			Ok(())
		}
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

impl<Balance, Currency> InvestState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	/// Solely apply state machine to transition one `InvestState` into another
	/// based on the transition, see https://centrifuge.hackmd.io/IPtRlOrOSrOF9MHjEY48BA?view#State-diagram.
	///
	/// NOTE: Does not mutate storage which is done by `apply_state_transition`.
	pub fn transition(
		&self,
		transition: InvestTransition<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match transition {
			InvestTransition::IncreaseInvestOrder(swap) => Self::handle_increase(&self, swap),
			InvestTransition::DecreaseInvestOrder(swap) => Self::handle_decrease(&self, swap),
			InvestTransition::SwapIntoPool(swap) => {
				Self::handle_fulfilled_swap_into_pool(&self, swap)
			}
			InvestTransition::SwapIntoReturn(swap) => {
				Self::handle_fulfilled_swap_into_return(&self, swap)
			}
		}
	}
}

// Actual impl of transition
impl<Balance, Currency> InvestState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	// TODO: Add to spec
	/// Handle `increase` transitions depicted by `msg::increase` edges in the
	/// state diagram. Behaves similar to a ledger when considering
	/// `SwapIntoReturnDone` and `InvestmentOngoing` as the
	///
	/// When we increase an investment, we normally have to swap it into pool
	/// currency (`ActiveSwapIntoPoolCurrency`) before it can be invested
	/// (`ActiveInvestmentOngoing`). However, if the current state includes
	/// swapping back into pool currency (`ActiveSwapIntoReturnCurrency`) as the
	/// result of a previous decrement, then we can minimize the amount which
	/// needs to be swapped such that we always have **at most a single active
	/// swap** which is the maximum of `pool_swap.amount` and
	/// `return_swap.amount`. When we do this, we always need to bump the
	/// investment amount as well as the `SwapIntoReturnDone` amount as a result
	/// of immediately fulfilling the pool swap order up to the possible amount.
	///
	/// Example:
	/// * Say before my pre state has `return_done = 1000` and
	/// `return_swap.amount = 500`. Now we look at three scenarios in which we
	/// increase below, exactly at and above the `return_swap.amount`:
	/// * a) If we increase by 500, we can reduce the `return_swap.amount`
	///   fully, which we denote by adding the 500 to the `return_done` amount.
	///   Moreover, we can immediately invest the 500. The resulting state is
	///   `(done_amount = 1500, investing = 500)`.
	/// * b) If we increase by 400, we can reduce the `return_swap.amount` only
	///   by 400 and increase both the `investing` as well as `return_done`
	///   amount by that. The resulting state is
	/// `(done_amount = 1400, return_swap.amount = 100, investing = 400)`.
	/// * c) If we increase by 600, we can reduce the `return_swap.amount` fully
	///   and need to add a swap into pool currency for 100. Moreover both the
	///   `investing` as well as `return_done` amount can only be increased by
	///   500. The resulting state is
	/// `(done_amount = 1500, pool_swap.amount = 100, investing = 500)`.
	///
	/// NOTE: We can ignore handling all states which include
	/// `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency` as we
	/// consume the done amount and transition in the post transition phase.
	fn handle_increase(&self, swap: Swap<Balance, Currency>) -> Result<Self, DispatchError> {
		match &self {
			Self::NoState => Ok(Self::ActiveSwapIntoPoolCurrency { swap }),
			// Add pool swap
			Self::InvestmentOngoing { invest_amount } => {
				Ok(Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
					swap,
					invest_amount: *invest_amount,
				})
			}
			// Bump pool swap
			Self::ActiveSwapIntoPoolCurrency { swap: pool_swap } => {
				Ok(Self::ActiveSwapIntoPoolCurrency {
					swap: Swap {
						amount: swap.amount.ensure_add(pool_swap.amount)?,
						..swap
					},
				})
			}
			// Reduce return swap amount by the increasing amount and increase investing amount as
			// well adding return_done amount by the minimum of active swap amounts
			Self::ActiveSwapIntoReturnCurrency { swap: return_swap } => {
				let invest_amount = swap.amount.min(return_swap.amount);
				let done_amount = swap.amount.min(return_swap.amount);

				// pool swap amount is immediately invested and done amount increased equally
				if swap.amount < return_swap.amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
						swap: Swap {
							// safe since swap.amount < return_swap.amount
							amount: return_swap.amount - swap.amount,
							..*return_swap
						},
						done_amount,
						invest_amount,
					})
				}
				// swap amount is immediately invested and done amount increased equally
				else if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						swap: *return_swap,
						invest_amount,
					})
				}
				// return swap amount is immediately invested and done amount increased equally
				else {
					Ok(
						Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe since swap.amount > return_swap.amount
								amount: swap.amount - return_swap.amount,
								..swap
							},
							done_amount,
							invest_amount,
						},
					)
				}
			}
			// Bump pool swap
			Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
				swap: pool_swap,
				invest_amount,
			} => Ok(Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
				swap: Swap {
					amount: swap.amount.ensure_add(pool_swap.amount)?,
					..swap
				},
				invest_amount: *invest_amount,
			}),
			// Reduce return swap amount by the increasing amount and increase investing amount as
			// well adding return_done amount by the minimum of active swap amounts
			Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
				swap: return_swap,
				invest_amount,
			} => {
				let invest_amount =
					invest_amount.ensure_add(swap.amount.min(return_swap.amount))?;
				let done_amount = swap.amount.min(return_swap.amount);

				// pool swap amount is immediately invested and done amount increased equally
				if swap.amount < return_swap.amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
						swap: Swap {
							// safe since swap.amount < return_swap.amount
							amount: return_swap.amount - swap.amount,
							..*return_swap
						},
						done_amount,
						invest_amount,
					})
				}
				// swap amount is immediately invested and done amount increased equally
				else if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						swap: *return_swap,
						invest_amount,
					})
				}
				// return swap amount is immediately invested and done amount increased equally
				else {
					Ok(
						Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe since swap.amount > return_swap.amount
								amount: swap.amount - return_swap.amount,
								..swap
							},
							done_amount,
							invest_amount,
						},
					)
				}
			}
			// Reduce amount of return by the increasing amount and increase investing as well as
			// return_done amount by the minimum of active swap amounts
			Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
				swap: return_swap,
				done_amount,
			} => {
				let invest_amount = swap.amount.min(return_swap.amount);
				let done_amount = invest_amount.ensure_add(*done_amount)?;

				// pool swap amount is immediately invested and done amount increased equally
				if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						swap: Swap {
							amount: done_amount,
							..*return_swap
						},
						invest_amount,
					})
				}
				// swap amount is immediately invested and done amount increased equally
				else if swap.amount < return_swap.amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: Swap {
			// safe since swap.amount < return_swap.amount
			amount: return_swap.amount - swap.amount,
			..*return_swap
		}, done_amount, invest_amount })
				}
				// return swap amount is immediately invested and done amount increased equally
				else {
					Ok(
						Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe since swap.amount > return_swap.amount
								amount: swap.amount - return_swap.amount,
								..swap
							},
							done_amount,
							invest_amount,
						},
					)
				}
			}
			// Reduce amount of return swap by increasing amount and increase investing as well as
			// return_done amount by minimum of swap amounts
			Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				swap: return_swap,
				done_amount,
				invest_amount,
			} => {
				let invest_amount =
					invest_amount.ensure_add(swap.amount.min(return_swap.amount))?;
				let done_amount = swap
					.amount
					.min(return_swap.amount)
					.ensure_add(*done_amount)?;

				// pool swap amount is immediately invested and done amount increased equally
				if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						swap: Swap {
							amount: done_amount,
							..*return_swap
						},
						invest_amount,
					})
				}
				// swap amount is immediately invested and done amount increased equally
				else if swap.amount < return_swap.amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: Swap {
			// safe since swap.amount < return_swap.amount
			amount: return_swap.amount - swap.amount,
			..*return_swap
		}, done_amount, invest_amount })
				}
				// return swap amount is immediately invested and done amount increased equally
				else {
					Ok(
						Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe since swap.amount > return_swap.amount
								amount: swap.amount - return_swap.amount,
								..swap
							},
							done_amount,
							invest_amount,
						},
					)
				}
			}
			// Should be updated to `ActiveSwapIntoPoolCurrencyAndInvestmentOngoing` in post
			// transition trigger and thus never exist as pre transition state
			Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { .. } => {
				Err(DispatchError::Corruption)
			}
			// Should be updated to `ActiveSwapIntoPoolCurrencyAndInvestmentOngoing` in post
			// transition trigger and thus never exist as pre transition state
			Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				..
			} => Err(DispatchError::Corruption),
			// Should be cleared entirely in post transition trigger and thus never exist as pre
			// transition state
			Self::SwapIntoReturnDone { .. } => Err(DispatchError::Corruption),
			// Should be updated to `InvestmentOngoing` in post transition trigger and thus never
			// exist as pre transition state
			Self::SwapIntoReturnDoneAndInvestmentOngoing { .. } => Err(DispatchError::Corruption),
		}
	}

	/// Handle `decrease` transitions depicted by `msg::decrease` edges in the
	/// state diagram.
	///
	/// NOTE: Throws the decreasing amount can never exceed the the amount which
	/// is swapping into pool currency and/or investing.
	fn handle_decrease(&self, swap: Swap<Balance, Currency>) -> Result<Self, DispatchError> {
		match &self {
			// Cannot reduce if there is neither an ongoing investment nor an active swap into pool currency
			InvestState::NoState => Err(DispatchError::Corruption),
			// Increment return swap amount up to ongoing investment
			InvestState::InvestmentOngoing { invest_amount } => {
				if swap.amount < *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap, invest_amount: *invest_amount - swap.amount })
				} else if swap.amount == *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrency { swap })
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			// Increment return done amount up to amount of the active pool swap
			InvestState::ActiveSwapIntoPoolCurrency { swap: pool_swap } => {
				if swap.amount == pool_swap.amount {
					Ok(Self::SwapIntoReturnDone { swap })
				} else if swap.amount < pool_swap.amount {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap: Swap {
						// safe because swap.amount < pool_swap.amount
						amount: pool_swap.amount - swap.amount,
						..*pool_swap
					}, done_amount: swap.amount})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			// Cannot reduce if there is neither an ongoing investment nor an active swap into pool currency
			InvestState::ActiveSwapIntoReturnCurrency { swap } => Err(DispatchError::Corruption),
			// Increment `return_done` up to pool swap amount and increment return swap amount up to ongoing investment
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap: pool_swap, invest_amount } => {
				let done_amount = swap.amount.min(pool_swap.amount);
				let invest_amount = invest_amount.ensure_sub(done_amount)?;
				let max_decrease_amount = pool_swap.amount.ensure_add(invest_amount)?;

				if swap.amount < pool_swap.amount {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: Swap {
						// safe because done_amount is min
						amount: pool_swap.amount - done_amount,
						..*pool_swap
					}, done_amount, invest_amount })
				}  else if swap.amount == pool_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing { swap, invest_amount })
				} else if swap.amount < max_decrease_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: Swap {
						// safe because done_amount is min
						amount: swap.amount - done_amount,
						..swap
					}, done_amount, invest_amount })
				} else if swap.amount == max_decrease_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap {
						// safe because done_amount is min
						amount: swap.amount - done_amount,
						..swap
					}, done_amount })
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			// Increment return swap up to ongoing investment
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap: return_swap, invest_amount } => {
				let amount = return_swap.amount.ensure_add(swap.amount)?;

				if swap.amount < *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap: Swap {
						amount,
						..swap
					},
					// safe because invest_amount > swap_amount
					invest_amount: *invest_amount - swap.amount })
				} else if swap.amount == *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrency { swap: Swap {
						amount,
						..swap
					} })
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			// Increment return_done amount up to pool swap amount
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap: pool_swap, done_amount } => {
				let done_amount = done_amount.ensure_add(swap.amount.min(pool_swap.amount))?;

				if swap.amount < pool_swap.amount {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap: Swap {
						amount: pool_swap.amount - done_amount,
						..*pool_swap
					}, done_amount })
 				} else if swap.amount == pool_swap.amount {
					Ok(Self::SwapIntoReturnDone { swap: Swap {
						amount: done_amount,
						..swap
					} })
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			// Cannot reduce if there is neither an ongoing investment nor an active swap into pool currency
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount } =>  Err(DispatchError::Corruption),
			// Increment `return_done` up to pool swap amount and increment return swap amount up to ongoing investment
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: pool_swap, done_amount, invest_amount } => {
				let return_amount = swap.amount.min(pool_swap.amount);
				let done_amount = done_amount.ensure_add(return_amount)?;
				let invest_amount = invest_amount.ensure_sub(return_amount)?;
				let max_decrease_amount = pool_swap.amount.ensure_add(invest_amount)?;

				if swap.amount < pool_swap.amount {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: Swap {
						// safe because return_amount is min
						amount: pool_swap.amount - return_amount,
						..*pool_swap
					}, done_amount, invest_amount })
				}  else if swap.amount == pool_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing { swap, invest_amount })
				} else if swap.amount < max_decrease_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: Swap {
						// safe because return_amount is min
						amount: swap.amount - return_amount,
						..swap
					}, done_amount, invest_amount })
				} else if swap.amount == max_decrease_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap {
						// safe because return_amount is min
						amount: swap.amount - return_amount,
						..swap
					}, done_amount })
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: return_swap, done_amount, invest_amount } => {
				let amount = return_swap.amount.ensure_add(swap.amount)?;

				if swap.amount < *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap: Swap { amount, ..swap}, done_amount: *done_amount,
						// safe because swap.amount < invest_amount
						invest_amount: *invest_amount - swap.amount })
				} else if swap.amount == *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount, ..swap}, done_amount: *done_amount })
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			// Cannot reduce if there is neither an ongoing investment nor an active swap into pool currency
			InvestState::SwapIntoReturnDone { swap } => Err(DispatchError::Corruption),
			InvestState::SwapIntoReturnDoneAndInvestmentOngoing { swap: Swap { amount: done_amount, .. }, invest_amount } => {
				if swap.amount < *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap, done_amount: *done_amount,
						// safe because swap.amount < invest_amount
						invest_amount: *invest_amount - swap.amount })
				} else if swap.amount == *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount: *done_amount })
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
	}
	}

	/// Handle partial/full token swap order transitions into pool currency
	/// depicted by `order_partial` edges in the state diagram where the swap
	/// currency matches the pool one.
	///
	/// NOTE: These should always increase the active ongoing investment.
	fn handle_fulfilled_swap_into_pool(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		todo!("Do state transition here")
	}

	/// Handle partial/full token swap order transitions into return currency
	/// depicted by `order_partial` edges in the state diagram with the swap
	/// currency matches the return one.
	///
	/// NOTE: Assumes the corresponding investment has been decreased
	/// beforehand.
	fn handle_fulfilled_swap_into_return(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		todo!("Do state transition here")
	}

	// TODO(@review): Do we need to handle this case at all or assume to always have
	// required swaps through foreign investments?
	// fn handle_increase_non_foreign(&self, swap: Swap<Balance, Currency>) ->
	// Result<Self, DispatchError> { 	match &self {
	// 		Self::NoState => {
	// 				Ok(Self::InvestmentOngoing {
	// 					invest_amount: swap.amount,
	// 				})
	// 		}
	// 		Self::InvestmentOngoing { invest_amount } => {
	// 				Ok(Self::InvestmentOngoing {
	// 					invest_amount: invest_amount.ensure_add(swap.amount)?,
	// 				})
	// 		}
	// 		Self::ActiveSwapIntoPoolCurrency { ..} => todo!(),
	// 		Self::ActiveSwapIntoReturnCurrency { ..} => todo!(),
	// 		Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
	// 			swap,
	// 			invest_amount,
	// 		} => todo!(),
	// 		Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
	// 			swap,
	// 			invest_amount,
	// 		} => todo!(),
	// 		Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap, done_amount }
	// => todo!(), 		Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap,
	// done_amount } => { 			todo!()
	// 		}
	// 		Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
	// 			swap,
	// 			done_amount,
	// 			invest_amount,
	// 		} => todo!(),
	// 		Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
	// 			swap,
	// 			done_amount,
	// 			invest_amount,
	// 		} => todo!(),
	// 		Self::SwapIntoReturnDone(done_swap) => {
	// 			if swap.currency_in == swap.currency_out {
	// 				if swap.amount < done_swap.amount {
	// 					Ok(InvestState::SwapIntoReturnDoneAndInvestmentOngoing {
	// 						swap: Swap {
	// 							currency_in: swap.currency_out,
	// 							currency_out: swap.currency_in,
	// 							amount: done_swap.amount.ensure_sub(swap.amount)?,
	// 						},
	// 						invest_amount: swap.amount,
	// 					})
	// 				} else {
	// 					Ok(Self::InvestmentOngoing {
	// 						invest_amount: swap.amount,
	// 					})
	// 				}
	// 			} else {
	// 				if swap.amount < done_swap.amount {
	// 					let done_amount = done_swap.amount.ensure_sub(swap.amount)?;
	// 					Ok(Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone {
	// 						swap,
	// 						done_amount,
	// 					})
	// 				} else {
	// 					Ok(Self::ActiveSwapIntoPoolCurrency { swap })
	// 				}
	// 			}
	// 		}
	// 		Self::SwapIntoReturnDoneAndInvestmentOngoing {
	// 			swap,
	// 			invest_amount,
	// 		} => todo!(),
	// }
}

// TODO: How to merge token swaps and investment trait? Create new trait
// ForeignInvestment? > Check diagrams

// impl<T: Config> Investment<T::AccountId> for Pallet<T> {
// 	type Amount = T::Balance;
// 	type CurrencyId = T::CurrencyId;
// 	type Error = DispatchError;
// 	type InvestmentId = T::InvestmentId;

// 	fn update_investment(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 		amount: Self::Amount,
// 	) -> Result<(), Self::Error> {
// 		let pre_amount = Self::investment(who, investment_id.clone())?;
// 		let pre_state = InvestmentState::<T>::get(who,
// investment_id.clone()).unwrap_or_default();

// 		if amount > pre_amount {
// 			// TODO: Can payment currency be derived?
// 			let swap_currency =
// 				<Self as Accountant>::info(investment_id).map(|info|
// info.payment_currency()); 			let post_state: Option<InvestState<<T as
// Config>::Balance, <T as Config>::CurrencyId>> = 				pre_state.
// transition(InvestTransition::IncreaseInvestOrder(amount))?; 			Ok(())
// 		} else if amount < pre_amount {
// 			let post_state: Option<InvestState<<T as Config>::Balance, <T as
// Config>::CurrencyId>> = 				pre_state.
// transition(InvestTransition::DecreaseInvestOrder(amount))?; 			Ok(())
// 		} else {
// 			Ok(())
// 		}
// 	}

// 	fn accepted_payment_currency(
// 		investment_id: Self::InvestmentId,
// 		currency: Self::CurrencyId,
// 	) -> bool {
// 		T::Investment::accepted_payment_currency(investment_id, currency)
// 	}

// 	fn investment(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 	) -> Result<Self::Amount, Self::Error> {
// 		todo!()
// 	}

// 	fn update_redemption(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 		amount: Self::Amount,
// 	) -> Result<(), Self::Error> {
// 		todo!()
// 	}

// 	fn accepted_payout_currency(
// 		investment_id: Self::InvestmentId,
// 		currency: Self::CurrencyId,
// 	) -> bool {
// 		T::Investment::accepted_payout_currency(investment_id, currency)
// 	}

// 	fn redemption(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 	) -> Result<Self::Amount, Self::Error> {
// 		todo!()
// 	}
// }
