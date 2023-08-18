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

use cfg_types::investments::Swap;
use sp_runtime::{
	traits::{EnsureAdd, EnsureSub},
	ArithmeticError, DispatchError,
};

use crate::types::{InvestState, InvestTransition};

impl<Balance, Currency> InvestState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	/// Solely apply state machine to transition one `InvestState` into another
	/// based on the transition, see https://centrifuge.hackmd.io/IPtRlOrOSrOF9MHjEY48BA?view#State-diagram.
	///
	/// NOTE: MUST call `apply_invest_state_transition` on the post state to
	/// actually mutate storage.
	pub fn transition(
		&self,
		transition: InvestTransition<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match transition {
			InvestTransition::IncreaseInvestOrder(swap) => Self::handle_increase(&self, swap),
			InvestTransition::DecreaseInvestOrder(swap) => Self::handle_decrease(&self, swap),
			InvestTransition::FulfillSwapOrder(swap) => {
				Self::handle_fulfilled_swap_order(&self, swap)
			}
			InvestTransition::EpochExecution(amount_unprocessed) => {
				Self::handle_epoch_execution(&self, amount_unprocessed)
			}
		}
	}

	/// Returns the potentially existing active swap into either pool or return
	/// currency:
	/// * If the state includes `ActiveSwapInto{Pool, Return}Currency`, it
	///   returns `Some(swap)`.
	/// * Else, it returns `None`.
	pub(crate) fn get_active_swap(&self) -> Option<Swap<Balance, Currency>> {
		match *self {
			InvestState::NoState => None,
			InvestState::InvestmentOngoing { .. } => None,
			InvestState::ActiveSwapIntoPoolCurrency { swap } => Some(swap),
			InvestState::ActiveSwapIntoReturnCurrency { swap } => Some(swap),
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, .. } => Some(swap),
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap, .. } => Some(swap),
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap, .. } => Some(swap),
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => Some(swap),
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap, .. } => {
				Some(swap)
			},
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap, .. } => {
				Some(swap)
			},
			InvestState::SwapIntoReturnDone { .. } => None,
			InvestState::SwapIntoReturnDoneAndInvestmentOngoing { .. } => None,
		}
	}

	/// Returns the `invest_amount` if existent, else zero.
	pub(crate) fn get_investing_amount(&self) -> Balance {
		match *self {
			InvestState::InvestmentOngoing { invest_amount}  |
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { invest_amount, .. } |
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { invest_amount, .. } |
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { invest_amount, .. } |
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { invest_amount, .. } |
			InvestState::SwapIntoReturnDoneAndInvestmentOngoing { invest_amount, .. } => invest_amount,
			_ => Balance::zero()
		}
	}
}

// Actual impl of transition
impl<Balance, Currency> InvestState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	/// Handle `increase` transitions depicted by `msg::increase` edges in the
	/// invest state diagram:
	/// * If there is no swap into return currency, the pool currency swap
	///   amount is increased.
	/// * Else, resolves opposite swap directions by immediately fulfilling the
	///   side with lower amounts; or both if the swap amounts are equal.
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
	/// * Say before my pre invest state has `return_done = 1000` and
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
	/// `*SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency*` as we
	/// consume the done amount and transition in the post transition phase.
	/// To be safe and to not make any unhandled assumptions, we throw
	/// `DispatchError::Other` for these states though we need to make sure
	/// this can never occur!
	fn handle_increase(&self, swap: Swap<Balance, Currency>) -> Result<Self, DispatchError> {
		if swap.currency_in == swap.currency_out {
			return Self::handle_increase_non_foreign(&self, swap);
		}

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
				swap.ensure_currencies_match(pool_swap, true)?;
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
				swap.ensure_currencies_match(return_swap, false)?;
				let invest_amount = swap.amount.min(return_swap.amount);
				let done_amount = swap.amount.min(return_swap.amount);

				// pool swap amount is immediately invested and done amount increased equally
				if swap.amount < return_swap.amount {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe since swap.amount < return_swap.amount
								amount: return_swap.amount - swap.amount,
								..*return_swap
							},
							done_amount,
							invest_amount,
						},
					)
				}
				// swap amount is immediately invested and done amount increased equally
				else if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						done_swap: *return_swap,
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
			} => {
				swap.ensure_currencies_match(pool_swap, true)?;

				Ok(Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
					swap: Swap {
						amount: swap.amount.ensure_add(pool_swap.amount)?,
						..swap
					},
					invest_amount: *invest_amount,
				})
			}
			// Reduce return swap amount by the increasing amount and increase investing amount as
			// well adding return_done amount by the minimum of active swap amounts
			Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
				swap: return_swap,
				invest_amount,
			} => {
				swap.ensure_currencies_match(return_swap, false)?;
				let invest_amount =
					invest_amount.ensure_add(swap.amount.min(return_swap.amount))?;
				let done_amount = swap.amount.min(return_swap.amount);

				// pool swap amount is immediately invested and done amount increased equally
				if swap.amount < return_swap.amount {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe since swap.amount < return_swap.amount
								amount: return_swap.amount - swap.amount,
								..*return_swap
							},
							done_amount,
							invest_amount,
						},
					)
				}
				// swap amount is immediately invested and done amount increased equally
				else if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						done_swap: *return_swap,
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
				swap.ensure_currencies_match(return_swap, false)?;
				let invest_amount = swap.amount.min(return_swap.amount);
				let done_amount = invest_amount.ensure_add(*done_amount)?;

				// pool swap amount is immediately invested and done amount increased equally
				if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						done_swap: Swap {
							amount: done_amount,
							..*return_swap
						},
						invest_amount,
					})
				}
				// swap amount is immediately invested and done amount increased equally
				else if swap.amount < return_swap.amount {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe since swap.amount < return_swap.amount
								amount: return_swap.amount - swap.amount,
								..*return_swap
							},
							done_amount,
							invest_amount,
						},
					)
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
				swap.ensure_currencies_match(return_swap, false)?;
				let invest_amount =
					invest_amount.ensure_add(swap.amount.min(return_swap.amount))?;
				let done_amount = swap
					.amount
					.min(return_swap.amount)
					.ensure_add(*done_amount)?;

				// pool swap amount is immediately invested and done amount increased equally
				if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						done_swap: Swap {
							amount: done_amount,
							..*return_swap
						},
						invest_amount,
					})
				}
				// swap amount is immediately invested and done amount increased equally
				else if swap.amount < return_swap.amount {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe since swap.amount < return_swap.amount
								amount: return_swap.amount - swap.amount,
								..*return_swap
							},
							done_amount,
							invest_amount,
						},
					)
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
			_ => Err(DispatchError::Other(
				"Invalid invest state, should automatically be transitioned into \
				 ActiveSwapIntoPoolCurrencyAndInvestmentOngoing",
			)),
		}
	}

	/// Handle `decrease` transitions depicted by `msg::decrease` edges in the
	/// state diagram:
	/// * If there is no swap into pool currency, the return currency swap
	///   amount is increased up to the ongoing investment amount which is not
	///   yet processed.
	/// * Else, resolves opposite swap directions by immediately fulfilling the
	///   side with lower amounts; or both if the swap amounts are equal.
	///
	/// Throws if the decreasing amount exceeds the amount which is
	/// currently swapping into pool currency and/or investing as we cannot
	/// decrease more than was invested. We must ensure, this can never happen
	/// at this stage!
	///
	/// NOTE: We can ignore handling all states which include
	/// `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency` as we
	/// consume the done amount and transition in the post transition phase.
	/// Moreover, we can ignore handling all states which do not include
	/// `ActiveSwapIntoPoolCurrency` or `InvestmentOngoing` as we cannot reduce
	/// further then.
	/// To be safe and to not make any unhandled assumptions, we throw
	/// `DispatchError::Other` for these states though we need to make sure
	/// this can never occur!
	fn handle_decrease(&self, swap: Swap<Balance, Currency>) -> Result<Self, DispatchError> {
		if swap.currency_in == swap.currency_out {
			return Self::handle_decrease_non_foreign(&self, swap);
		}

		match &self {
			// Cannot reduce if there is neither an ongoing investment nor an active swap into pool currency
			InvestState::NoState
			| InvestState::ActiveSwapIntoReturnCurrency { .. }
			| InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { .. } => {
				Err(DispatchError::Other("Invalid invest state when transitioning a decrease"))
			},
			// Increment return swap amount up to ongoing investment
			InvestState::InvestmentOngoing { invest_amount } => {
				if swap.amount < *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
						swap,
						invest_amount: *invest_amount - swap.amount,
					})
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
				swap.ensure_currencies_match(pool_swap, false)?;

				if swap.amount == pool_swap.amount {
					Ok(Self::SwapIntoReturnDone { done_swap: swap })
				} else if swap.amount < pool_swap.amount {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone {
						swap: Swap {
							// safe because swap.amount < pool_swap.amount
							amount: pool_swap.amount - swap.amount,
							..*pool_swap
						},
						done_amount: swap.amount,
					})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			// Increment `return_done` up to pool swap amount and increment return swap amount up to ongoing investment
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
				swap: pool_swap,
				invest_amount,
			} => {
				swap.ensure_currencies_match(pool_swap, false)?;
				let done_amount = swap.amount.min(pool_swap.amount);
				let invest_amount = invest_amount.ensure_sub(done_amount)?;
				let max_decrease_amount = pool_swap.amount.ensure_add(invest_amount)?;

				if swap.amount < pool_swap.amount {
					Ok(
						Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe because done_amount is min
								amount: pool_swap.amount - done_amount,
								..*pool_swap
							},
							done_amount,
							invest_amount,
						},
					)
				} else if swap.amount == pool_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						done_swap: swap,
						invest_amount,
					})
				} else if swap.amount < max_decrease_amount {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe because done_amount is min
								amount: swap.amount - done_amount,
								..swap
							},
							done_amount,
							invest_amount,
						},
					)
				} else if swap.amount == max_decrease_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
						swap: Swap {
							// safe because done_amount is min
							amount: swap.amount - done_amount,
							..swap
						},
						done_amount,
					})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			// Increment return swap up to ongoing investment
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
				swap: return_swap,
				invest_amount,
			} => {
				swap.ensure_currencies_match(return_swap, true)?;
				let amount = return_swap.amount.ensure_add(swap.amount)?;

				if swap.amount < *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
						swap: Swap { amount, ..swap },
						// safe because invest_amount > swap_amount
						invest_amount: *invest_amount - swap.amount,
					})
				} else if swap.amount == *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrency {
						swap: Swap { amount, ..swap },
					})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				swap: return_swap,
				done_amount,
				invest_amount,
			} => {
				swap.ensure_currencies_match(return_swap, true)?;
				let amount = return_swap.amount.ensure_add(swap.amount)?;

				if swap.amount < *invest_amount {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap { amount, ..swap },
							done_amount: *done_amount,
							// safe because swap.amount < invest_amount
							invest_amount: *invest_amount - swap.amount,
						},
					)
				} else if swap.amount == *invest_amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
						swap: Swap { amount, ..swap },
						done_amount: *done_amount,
					})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
				}
			},
			_ => Err(DispatchError::Other(
				"Invalid invest state, should automatically be transitioned into \
				 ActiveSwapIntoPoolCurrencyAndInvestmentOngoing",
			)),
		}
	}

	/// Handle partial/full token swap order transitions  depicted by
	/// `order_partial` and `order_full` edges in the state diagram.
	///
	/// Please note, that we ensure that there can always be at most one swap,
	/// either into pool currency (`ActiveSwapIntoPoolCurrency`) or into return
	/// currency (`ActiveSwapIntoReturnCurrency`). Thus, if the previous state
	/// (`&self`) is into pool, we know the incoming transition is made from
	/// return into pool currency and vice versa if the previous state is
	/// swapping into return currency.
	///
	/// This transition should always increase the active ongoing
	/// investment.
	///
	/// NOTE: We can ignore handling all states which include
	/// `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency` as we
	/// consume the done amount and transition in the post transition phase.
	/// Moreover, we can ignore handling all states which do not include
	/// `ActiveSwapInto{Pool, Return}Currency` as else there cannot be an active
	/// token swap for investments.
	/// To be safe and to not make any unhandled assumptions, we throw
	/// `DispatchError::Other` for these states though we need to make sure
	/// this can never occur!

	// FIXME(@review): This handler assumes partial fulfillments and 1-to-1
	// conversion of amounts, i.e., 100 `return_currency` equals 100
	// `pool_currency`. If we use the CurrencyConverter, the amounts could be off as
	// the `CurrencyConverter` is decoupled from the `TokenSwaps` trait.
	fn handle_fulfilled_swap_order(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match &self {
			InvestState::NoState | InvestState::InvestmentOngoing { .. } => Err(DispatchError::Other(
				"Invalid invest state when transitioning a fulfilled order",
			)),

			// Increment ongoing investment by swapped amount
			InvestState::ActiveSwapIntoPoolCurrency { swap: pool_swap } => {
				swap.ensure_currencies_match(pool_swap, true)?;

				if swap.amount == pool_swap.amount {
					Ok(Self::InvestmentOngoing {
						invest_amount: swap.amount,
					})
				} else if swap.amount < pool_swap.amount {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
						swap: Swap {
							// safe because pool_swap.amount > swap.amount
							amount: pool_swap.amount - swap.amount,
							..swap
						},
						invest_amount: swap.amount,
					})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
				}
			},
			// Increment done_return by swapped amount
			InvestState::ActiveSwapIntoReturnCurrency { swap: return_swap } => {
				swap.ensure_currencies_match(return_swap, true)?;

				if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDone { done_swap: swap })
				} else if swap.amount < return_swap.amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
						swap: Swap {
							// safe because return_swap.amount > swap.amount
							amount: return_swap.amount - swap.amount,
							..swap
						},
						done_amount: swap.amount,
					})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
				}
			},
			// Increment ongoing investment by swapped amount
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
				swap: pool_swap,
				invest_amount,
			} => {
				swap.ensure_currencies_match(pool_swap, true)?;
				let invest_amount = invest_amount.ensure_add(swap.amount)?;

				if swap.amount == pool_swap.amount {
					Ok(Self::InvestmentOngoing { invest_amount })
				} else if swap.amount < pool_swap.amount {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
						swap: Swap {
							// safe because pool_swap.amount > swap.amount
							amount: pool_swap.amount - swap.amount,
							..swap
						},
						invest_amount,
					})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
				}
			},
			// Increment done_return by swapped amount, leave invest amount untouched
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
				swap: return_swap,
				invest_amount,
			} => {
				swap.ensure_currencies_match(return_swap, true)?;

				if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						done_swap: swap,
						invest_amount: *invest_amount,
					})
				} else if swap.amount < return_swap.amount {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe because return_swap.amount > swap.amount
								amount: return_swap.amount - swap.amount,
								..swap
							},
							done_amount: swap.amount,
							invest_amount: *invest_amount,
						},
					)
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
				}
			},
			// Increment done_return by swapped amount
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
				swap: return_swap,
				done_amount,
			} => {
				swap.ensure_currencies_match(return_swap, true)?;
				let done_amount = done_amount.ensure_add(swap.amount)?;

				if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDone {
						done_swap: Swap {
							amount: done_amount,
							..swap
						},
					})
				} else if swap.amount < return_swap.amount {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
						swap: Swap {
							// safe because return_swap.amount > swap.amount
							amount: return_swap.amount - swap.amount,
							..swap
						},
						done_amount,
					})
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
				}
			},
			// Increment done_return by swapped amount, leave invest amount untouched
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				swap: return_swap,
				done_amount,
				invest_amount,
			} => {
				swap.ensure_currencies_match(return_swap, true)?;
				let done_amount = done_amount.ensure_add(swap.amount)?;

				if swap.amount == return_swap.amount {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						done_swap: Swap {
							amount: done_amount,
							..swap
						},
						invest_amount: *invest_amount,
					})
				} else if swap.amount < return_swap.amount {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap: Swap {
								// safe because return_swap.amount > swap.amount
								amount: return_swap.amount - swap.amount,
								..swap
							},
							done_amount,
							invest_amount: *invest_amount,
						},
					)
				}
				// should never occur but let's be safe here
				else {
					Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
				}
			},
			_ => Err(DispatchError::Other(
				"Invalid invest state, should automatically be transitioned into state without AndSwapIntoReturnDone",
			)),
		}
	}

	/// Handle increase transitions for the same incoming and outgoing
	/// currencies.
	///
	/// NOTE: We can ignore handling all states which include
	/// `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency` as we
	/// consume the done amount and transition in the post transition phase.
	/// Moreover, we can ignore any state which involves an active swap, i.e.
	/// `ActiveSwapInto{Pool, Return}Currency`, as these must not exist if the
	/// in and out currency is the same.
	/// To be safe and to not make any unhandled assumptions, we throw
	/// `DispatchError::Other` for these states though we need to make sure
	/// this can never occur!
	fn handle_increase_non_foreign(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match &self {
			Self::NoState => Ok(Self::InvestmentOngoing {
				invest_amount: swap.amount,
			}),
			Self::InvestmentOngoing { invest_amount } => Ok(Self::InvestmentOngoing {
				invest_amount: invest_amount.ensure_add(swap.amount)?,
			}),
			Self::ActiveSwapIntoPoolCurrency { .. }
			| Self::ActiveSwapIntoReturnCurrency { .. }
			| Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { .. }
			| Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { .. }
			| Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { .. }
			| Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				..
			} => Err(DispatchError::Other(
				"Invalid invest state when transitioning an increased swap order with the same in- \
				 and outgoing currency",
			)),
			_ => Err(DispatchError::Other(
				"Invalid invest state, should automatically be transitioned into state without \
				 AndSwapIntoReturnDone",
			)),
		}
	}

	/// Handle decrease transitions for the same incoming and outgoing
	/// currencies.
	///
	/// NOTE: We can ignore handling all states which include
	/// `SwapIntoReturnDone` without `ActiveSwapIntoReturnCurrency` as we
	/// consume the done amount and transition in the post transition phase.
	/// Moreover, we can ignore any state which involves an active swap, i.e.
	/// `ActiveSwapInto{Pool, Return}Currency`, as these must not exist if the
	/// in and out currency is the same.
	/// To be safe and to not make any unhandled assumptions, we throw
	/// `DispatchError::Other` for these states though we need to make sure
	/// this can never occur!
	fn handle_decrease_non_foreign(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		if let Self::InvestmentOngoing { invest_amount } = &self {
			if swap.amount < *invest_amount {
				Ok(InvestState::SwapIntoReturnDoneAndInvestmentOngoing {
					done_swap: swap,
					invest_amount: invest_amount.ensure_sub(swap.amount)?,
				})
			} else {
				Ok(Self::SwapIntoReturnDone { done_swap: swap })
			}
		}
		// should never occur but let's be safe here
		else {
			Err(DispatchError::Other(
				"Invalid invest state when transitioning a decreased swap order with the same in- \
				 and outgoing currency",
			))
		}
	}

	/// Update or kill the unprocessed investment amount.
	/// * If the state does not include `InvestmentOngoing` and the unprocessed
	///   amount is not zero, there is nothing to transition, return the current
	///   state. If the unprocessed amount is zero, state is corrupted.
	/// * Else If the provided `unprocessed_amount` is zero, remove
	///   `InvestmentOngoing` from the state
	/// * Else set the `invest_amount` to `unprocessed_amount`
	fn handle_epoch_execution(&self, unprocessed_amount: Balance) -> Result<Self, DispatchError> {
		match *self {
			Self::InvestmentOngoing { .. } => {
				if unprocessed_amount.is_zero() {
					Ok(Self::NoState)
				} else {
					Ok(Self::InvestmentOngoing {
						invest_amount: unprocessed_amount,
					})
				}
			}
			Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, .. } => {
				if unprocessed_amount.is_zero() {
					Ok(Self::ActiveSwapIntoPoolCurrency { swap })
				} else {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
						swap,
						invest_amount: unprocessed_amount,
					})
				}
			}
			Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap, .. } => {
				if unprocessed_amount.is_zero() {
					Ok(Self::ActiveSwapIntoReturnCurrency { swap })
				} else {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
						swap,
						invest_amount: unprocessed_amount,
					})
				}
			}
			Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				swap,
				done_amount,
				..
			} => {
				if unprocessed_amount.is_zero() {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap, done_amount })
				} else {
					Ok(
						Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
							swap,
							done_amount,
							invest_amount: unprocessed_amount,
						},
					)
				}
			}
			Self::SwapIntoReturnDoneAndInvestmentOngoing { done_swap, .. } => {
				if unprocessed_amount.is_zero() {
					Ok(Self::SwapIntoReturnDone { done_swap })
				} else {
					Ok(Self::SwapIntoReturnDoneAndInvestmentOngoing {
						done_swap,
						invest_amount: unprocessed_amount,
					})
				}
			}
			Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				swap,
				done_amount,
				..
			} => {
				if unprocessed_amount.is_zero() {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
						swap,
						done_amount,
					})
				} else {
					Ok(Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
						swap,
						done_amount,
						invest_amount: unprocessed_amount,
					})
				}
			}
			state => {
				if unprocessed_amount.is_zero() {
					Ok(state)
				} else {
					Err(DispatchError::Other(
						"Invalid invest state when transitioning epoch execution",
					))
				}
			}
		}
	}
}
