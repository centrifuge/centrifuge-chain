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
use frame_support::{ensure, traits::Get, transactional};
use sp_runtime::{
	traits::{EnsureAdd, EnsureSub, Zero},
	ArithmeticError, DispatchError, DispatchResult,
};

use crate::{
	types::{
		InnerRedeemState,
		InnerRedeemState::{
			ActiveSwapIntoReturnCurrency, ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone,
			CollectableRedemption, CollectableRedemptionAndActiveSwapIntoReturnCurrency,
			CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone,
			CollectableRedemptionAndSwapIntoReturnDone, Redeeming,
			RedeemingAndActiveSwapIntoReturnCurrency,
			RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone,
			RedeemingAndCollectableRedemption,
			RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency,
			RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone,
			RedeemingAndCollectableRedemptionAndSwapIntoReturnDone, RedeemingAndSwapIntoReturnDone,
			SwapIntoReturnDone,
		},
		InvestTransition, RedeemState, RedeemTransition, Swap,
	},
	Config, Error, ForeignInvestmentInfo, ForeignInvestmentInfoOf, InvestmentState, Pallet, SwapOf,
	TokenSwapOrderIds,
};

impl<Balance, Currency> RedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	/// Solely apply state machine to transition one `RedeemState` into another
	/// based on the transition, see https://centrifuge.hackmd.io/IPtRlOrOSrOF9MHjEY48BA?view#Redemption-States
	///
	/// NOTE: MUST call `apply_redeem_state_transition` on the post state to
	/// actually mutate storage.
	pub fn transition(
		&self,
		transition: RedeemTransition<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match transition {
			RedeemTransition::IncreaseRedeemOrder(amount) => Self::handle_increase(&self, amount),
			RedeemTransition::DecreaseRedeemOrder(amount) => Self::handle_decrease(&self, amount),
			RedeemTransition::FulfillSwapOrder(swap) => {
				Self::handle_fulfilled_swap_order(&self, swap)
			}
		}
	}

	/// Returns the potentially existing active swap into return currency of the
	/// inner state:
	/// * If the inner state includes `ActiveSwapIntoReturnCurrency`, it returns
	///   the corresponding `Some(swap)`.
	/// * Else, it returns `None`.
	pub(crate) fn get_active_swap(&self) -> Option<Swap<Balance, Currency>> {
		match self {
			Self::NoState => None,
			Self::Invested { invest_amount } => None,
			Self::NotInvestedAnd { inner } | Self::InvestedAnd { inner, .. } => {
				inner.get_active_swap()
			}
		}
	}

	/// Returns the potentially existing redeeming amount of the inner state:
	/// * If the inner state includes `Redeeming`, it returns the corresponding
	///   `Some(amount)`.
	/// * Else, it returns `None`.
	pub(crate) fn get_redeeming_amount(&self) -> Option<Balance> {
		match self {
			Self::NoState => None,
			Self::Invested { invest_amount } => None,
			Self::NotInvestedAnd { inner } | Self::InvestedAnd { inner, .. } => {
				inner.get_redeeming_amount()
			}
		}
	}

	///  Exchanges the inner state of `RedeemState::InvestedAnd` as well as
	/// `RedeemState::NotInvestedAnd` with the provided one similar to a memory
	/// swap.
	pub(crate) fn swap_inner_state(&self, inner: InnerRedeemState<Balance, Currency>) -> Self {
		match *self {
			Self::InvestedAnd { invest_amount, .. } => Self::InvestedAnd {
				invest_amount,
				inner,
			},
			Self::NotInvestedAnd { .. } => Self::NotInvestedAnd { inner },
			_ => *self,
		}
	}

	/// Reduce the amount of an active swap (into return currency) of the
	/// `InnerRedeemState` by the provided value:
	/// * If the provided value equals the swap amount, the state is
	///   transitioned into `*AndSwapIntoReturnDone`.
	/// * Else, it is transitioned into
	///   `*ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone`.
	///
	/// NOTE: Throws if any of the following holds true:
	/// * The outer `RedeemState` is not `InvestedAnd` or `NotInvested` as this
	///   implies there is no active swap.
	/// * The inner state is not an active swap, i.e. the state does not include
	///   `ActiveSwapIntoReturnCurrency`.
	/// * The reducible amount exceeds the active swap amount.
	pub(crate) fn fulfill_active_swap_amount(
		&self,
		amount: Balance,
	) -> Result<Self, DispatchError> {
		match self {
			Self::InvestedAnd { inner, .. } | Self::NotInvestedAnd { inner } => Ok(
				Self::swap_inner_state(&self, inner.fulfill_active_swap_amount(amount)?),
			),
			_ => Err(DispatchError::Other(
				"Cannot alter active swap amount for RedeemStates without swap",
			)),
		}
	}

	// pub(crate) increase_invested(&self, amount: Balance) -> Result<Self,
	// DispatchError> { 	match self {
	// 		Self::InvestedAnd { invest_amount, inner } => {

	// 		}
	// 	}
	// }
}

impl<Balance, Currency> InnerRedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	/// Returns the potentially existing active swap into return currency:
	/// * If the state includes `ActiveSwapIntoReturnCurrency`, it returns the
	///   corresponding `Some(swap)`.
	/// * Else, it returns `None`.
	fn get_active_swap(&self) -> Option<Swap<Balance, Currency>> {
		match *self {
			Self::Redeeming { .. } => None,
			Self::CollectableRedemption { .. } => None,
			Self::RedeemingAndCollectableRedemption { .. } => None,
			Self::ActiveSwapIntoReturnCurrency { swap } => Some(swap),
			Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => Some(swap),
			Self::SwapIntoReturnDone { .. } => None,
			Self::RedeemingAndActiveSwapIntoReturnCurrency { swap, .. } => Some(swap),
			Self::RedeemingAndSwapIntoReturnDone { .. } => None,
			Self::RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => Some(swap),
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { swap, .. } => Some(swap),
			Self::RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { .. } => None,
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => Some(swap),
			Self::CollectableRedemptionAndActiveSwapIntoReturnCurrency { swap, .. } => Some(swap),
			Self::CollectableRedemptionAndSwapIntoReturnDone { .. } => None,
			Self::CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => Some(swap),
		}
	}

	/// Returns the potentially existing redeeming amount:
	/// * If the state includes `Redeeming`, it returns the corresponding
	///   `Some(amount)`.
	/// * Else, it returns `None`.
	fn get_redeeming_amount(&self) -> Option<Balance> {
		match *self {
			Self::Redeeming { redeem_amount } => Some(redeem_amount),
			Self::CollectableRedemption { .. } => None,
			Self::RedeemingAndCollectableRedemption { redeem_amount, .. } => Some(redeem_amount),
			Self::ActiveSwapIntoReturnCurrency { .. } => None,
			Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { .. } => None,
			Self::SwapIntoReturnDone { .. } => None,
			Self::RedeemingAndActiveSwapIntoReturnCurrency { redeem_amount, .. } => Some(redeem_amount),
			Self::RedeemingAndSwapIntoReturnDone { redeem_amount, .. } => Some(redeem_amount),
			Self::RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount, .. } => Some(redeem_amount),
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { redeem_amount, .. } => Some(redeem_amount),
			Self::RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { redeem_amount, .. } => Some(redeem_amount),
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount, .. } => Some(redeem_amount),
			Self::CollectableRedemptionAndActiveSwapIntoReturnCurrency { .. } => None,
			Self::CollectableRedemptionAndSwapIntoReturnDone { .. } => None,
			Self::CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { .. } => None,
		}
	}

	/// Reduce the amount of an active swap (into return currency) by the
	/// provided value:
	/// * Throws if there is no active swap, i.e. the state does not include
	///   `ActiveSwapIntoReturnCurrency` or if the reducible amount exceeds the
	///   swap amount
	/// * If the provided value equals the swap amount, the state is
	///   transitioned into `*AndSwapIntoReturnDone`.
	/// * Else, it is transitioned into
	///   `*ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone`.
	fn fulfill_active_swap_amount(&self, amount: Balance) -> Result<Self, DispatchError> {
		match self {
			Self::ActiveSwapIntoReturnCurrency { swap } => {
				if amount == swap.amount{
					Ok(Self::SwapIntoReturnDone { done_swap: *swap })
				} else {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount: amount
						}
					)
				}
			},
			Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(
						Self::SwapIntoReturnDone {
							done_swap: Swap {
								amount: done_amount,
								..*swap
							}
						}
					)
				} else {
					Ok(
						Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount,
						}
					)
				}
			},
			Self::RedeemingAndActiveSwapIntoReturnCurrency { redeem_amount, swap } => {
				if amount == swap.amount {
					Ok(
						Self::RedeemingAndSwapIntoReturnDone {
							done_swap: Swap {
								amount,
								..*swap
							},
							redeem_amount: *redeem_amount,
						}
					)
				} else {
					Ok(
						Self::RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount: amount,
							redeem_amount: *redeem_amount,
						}
					)
				}
			},
			Self::RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(
						Self::RedeemingAndSwapIntoReturnDone {
							done_swap: Swap {
								amount: done_amount,
								..*swap
							},
							redeem_amount: *redeem_amount,
						}
					)
				} else {
					Ok(
						Self::RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount,
							redeem_amount: *redeem_amount,
						}
					)
				}
			},
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { redeem_amount, collect_amount, swap } => {
				if amount == swap.amount {
					Ok(
						Self::RedeemingAndCollectableRedemptionAndSwapIntoReturnDone {
							done_swap: *swap,
							redeem_amount: *redeem_amount,
							collect_amount: *collect_amount,
						}
					)
				} else {
					Ok(
						Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount: amount,
							redeem_amount: *redeem_amount,
							collect_amount: *collect_amount,
						}
					)
				}
			},
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount, collect_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(
						Self::RedeemingAndCollectableRedemptionAndSwapIntoReturnDone {
							done_swap: Swap {
								amount: done_amount,
								..*swap
							},
							redeem_amount: *redeem_amount,
							collect_amount: *collect_amount,
						}
					)
				} else {
					Ok(
						Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount,
							redeem_amount: *redeem_amount,
							collect_amount: *collect_amount,
						}
					)
				}
			},
			Self::CollectableRedemptionAndActiveSwapIntoReturnCurrency { collect_amount, swap } => {
				if amount == swap.amount {
					Ok(
						Self::CollectableRedemptionAndSwapIntoReturnDone {
							done_swap: *swap,
							collect_amount: *collect_amount,
						}
					)
				} else {
					Ok(
						Self::CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount: amount,
							collect_amount: *collect_amount,
						}
						)
				}
			},
			Self::CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { collect_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(
						Self::CollectableRedemptionAndSwapIntoReturnDone {
							done_swap: Swap {
								amount: done_amount,
								..*swap
							},
							collect_amount: *collect_amount,
						}
					)
				} else {
					Ok(
						Self::CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount,
							collect_amount: *collect_amount,
						}
					)
				}
			},
			_ => Err(DispatchError::Other(
				"Invalid inner redeem state when fulfilling active swap amount",
			))
		}
	}

	/// Sets the redeeming amount to the provided value:
	/// * If the value is not zero and the state involves `Redeeming`: Sets the
	///   amount.
	/// * Else if the value is not zero and the state does not involve
	///   `Redeeming`: Adds `Redeeming` to the state with the corresponding
	///   amount.
	/// * If the value is zero and the state includes `Redeeming`: Removes
	///   `Redeeming` from the state.
	/// * Else throws.
	///
	/// NOTE: If setting the amount to a non-zero value, assumes this function
	/// is **only called on inner states of `RedeemState::InvestedAnd`** as the
	/// redeeming amounts can at most equal the processed investment amount.
	/// Since the inner state has no knowledge about the investment amount, this
	/// check must be done beforehand.
	fn set_redeem_amount(&self, amount: Balance) -> Result<Self, DispatchError> {
		// Remove `Redeeming` from state
		if amount.is_zero() {
			match *self {
				Redeeming { .. } => Err(DispatchError::Other("Outer RedeemState must be transitioned to Self::Invested")),
				RedeemingAndCollectableRedemption { collect_amount, .. } => Ok(CollectableRedemption { collect_amount }),
				RedeemingAndActiveSwapIntoReturnCurrency { swap, .. } => Ok(ActiveSwapIntoReturnCurrency { swap }),
				RedeemingAndSwapIntoReturnDone { done_swap, .. } => Ok(SwapIntoReturnDone { done_swap }),
				RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount, .. } => Ok(ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount }),
				RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { collect_amount, swap, .. } => Ok(CollectableRedemptionAndActiveSwapIntoReturnCurrency { collect_amount, swap }),
				RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { collect_amount, done_swap, .. } => Ok(CollectableRedemptionAndSwapIntoReturnDone { collect_amount, done_swap }),
				RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { collect_amount, swap, done_amount, .. } => Ok(CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { collect_amount, swap, done_amount }),
				// Throw for states without `Redeeming`
				inner => Err(DispatchError::Other("Cannot remove redeeming amount of inner redeem state which does not include `Redeeming`")),
			}
		}
		// Set `redeeming` to non-zero value. Add `Redeeming` if not part of state yet.
		else {
			match *self {
				Redeeming { redeem_amount } => Ok(Redeeming { redeem_amount: amount }),
				RedeemingAndCollectableRedemption { redeem_amount, collect_amount } => Ok(RedeemingAndCollectableRedemption { redeem_amount: amount, collect_amount }),
				RedeemingAndActiveSwapIntoReturnCurrency { redeem_amount, swap } => Ok(RedeemingAndActiveSwapIntoReturnCurrency { redeem_amount: amount, swap }),
				RedeemingAndSwapIntoReturnDone { redeem_amount, done_swap } => Ok(RedeemingAndSwapIntoReturnDone { redeem_amount: amount, done_swap }),
				RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount, swap, done_amount } => Ok(RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount: amount, swap, done_amount }),
				RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { redeem_amount, collect_amount, swap } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { redeem_amount: amount, collect_amount, swap }),
				RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { redeem_amount, collect_amount, done_swap } => Ok(RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { redeem_amount: amount, collect_amount, done_swap }),
				RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount, collect_amount, swap, done_amount } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount: amount, collect_amount, swap, done_amount }),
				CollectableRedemption { collect_amount } => Ok(RedeemingAndCollectableRedemption { collect_amount, redeem_amount: amount }),
				ActiveSwapIntoReturnCurrency { swap } => Ok(RedeemingAndActiveSwapIntoReturnCurrency { swap, redeem_amount: amount }),
				ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount } => Ok(RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount, redeem_amount: amount }),
				SwapIntoReturnDone { done_swap } => Ok(RedeemingAndSwapIntoReturnDone { done_swap, redeem_amount: amount }),
				CollectableRedemptionAndActiveSwapIntoReturnCurrency { collect_amount, swap } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { collect_amount, swap, redeem_amount: amount }),
				CollectableRedemptionAndSwapIntoReturnDone { collect_amount, done_swap } => Ok(RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { collect_amount, done_swap, redeem_amount: amount }),
				CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { collect_amount, swap, done_amount } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { collect_amount, swap, done_amount, redeem_amount: amount }),
			}
		}
	}

	/// Transition all inner states which include
	/// `ActiveSwapIntoReturnCurrency` either into `*SwapIntoReturnDone` or
	/// `*ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone`.
	///
	/// Throws if the fulfilled swap direction is not into return currency or if
	/// the amount exceeds the states active swap amount.
	///
	/// NOTE: We can ignore all states which do not include
	/// `ActiveSwapIntoReturnCurrency`.
	fn transition_fulfilled_swap_order(
		&self,
		fulfilled_swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		ensure!(
			self.get_active_swap()
				.map(|swap| {
					swap.amount >= fulfilled_swap.amount
						&& swap.currency_in == fulfilled_swap.currency_in
						&& swap.currency_out == fulfilled_swap.currency_out
				})
				.unwrap_or(true),
			DispatchError::Other(
				"Invalid inner redeem state when transitioning fulfilled swap order"
			)
		);

		let Swap { amount, .. } = fulfilled_swap;

		match *self {
			Redeeming { .. } |
			CollectableRedemption { .. } |
			RedeemingAndCollectableRedemption { .. } |
			SwapIntoReturnDone { .. } |
			RedeemingAndSwapIntoReturnDone { .. } |
			RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { .. } |
			CollectableRedemptionAndSwapIntoReturnDone { .. } => Err(DispatchError::Other("Invalid inner redeem state when transitioning fulfilled swap order")),
			ActiveSwapIntoReturnCurrency { swap } => {
				if amount < swap.amount {
					Ok(ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount: amount })
				} else {
					Ok(SwapIntoReturnDone { done_swap: swap })
				}
			},
			ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount {
					Ok(ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount })
				} else {
					Ok(SwapIntoReturnDone { done_swap: Swap { amount: done_amount, ..swap } })
				}
			},
			RedeemingAndActiveSwapIntoReturnCurrency { redeem_amount, swap } => {
				if amount < swap.amount {
					Ok(RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount: amount, redeem_amount })
				} else {
					Ok(RedeemingAndSwapIntoReturnDone { done_swap: swap, redeem_amount })
				}
			},
			RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount {
					Ok(RedeemingAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount, redeem_amount })
				} else {
					Ok(RedeemingAndSwapIntoReturnDone { done_swap: Swap { amount: done_amount, ..swap }, redeem_amount })
				}
			},
			RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrency { redeem_amount, collect_amount, swap } => {
				if amount < swap.amount {
					Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount: amount, redeem_amount, collect_amount })
				} else {
					Ok(RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { done_swap: swap, redeem_amount, collect_amount })
				}
			},
			RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { redeem_amount, collect_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount {
					Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount, redeem_amount, collect_amount })
				} else {
					Ok(RedeemingAndCollectableRedemptionAndSwapIntoReturnDone { done_swap: Swap { amount: done_amount, ..swap }, redeem_amount, collect_amount })
				}
			},
			CollectableRedemptionAndActiveSwapIntoReturnCurrency { collect_amount, swap } => {
				if amount < swap.amount {
					Ok(CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount: amount, collect_amount })
				} else {
					Ok(CollectableRedemptionAndSwapIntoReturnDone { done_swap: swap, collect_amount })
				}
			},
			CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { collect_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount {
					Ok(CollectableRedemptionAndActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount, collect_amount })
				} else {
					Ok(CollectableRedemptionAndSwapIntoReturnDone { done_swap: Swap { amount: done_amount, ..swap }, collect_amount })
				}
			},
		}
	}
}

// Actual impl of transition
impl<Balance, Currency> RedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	/// Handle `increase` transitions depicted by `msg::increase` edges in the
	/// redeem state diagram:
	/// * If the current state includes a processed investment, i.e. is either
	///   `RedeemState::Invested` or `RedeemState::InvestedAnd(..)`, decreases
	///   the invest amount and increases the redeeming amount. Throws if the
	///   investment amount is exceeded as this reflects the max redeeming
	///   amount.
	/// * Else throws for incorrect pre state.
	fn handle_increase(&self, amount: Balance) -> Result<Self, DispatchError> {
		match self {
			Self::NoState | Self::NotInvestedAnd { .. } => Err(DispatchError::Other(
				"Invalid redeem state when transitioning an increase",
			)),
			Self::Invested { invest_amount } => {
				if &amount == invest_amount {
					Ok(Self::NotInvestedAnd {
						inner: Redeeming {
							redeem_amount: amount,
						},
					})
				} else {
					Ok(Self::InvestedAnd {
						invest_amount: invest_amount.ensure_sub(amount)?,
						inner: Redeeming {
							redeem_amount: amount,
						},
					})
				}
			}
			Self::InvestedAnd {
				invest_amount,
				inner,
			} => {
				if &amount == invest_amount {
					Ok(Self::NotInvestedAnd {
						inner: inner.set_redeem_amount(amount)?,
					})
				} else {
					Ok(Self::InvestedAnd {
						invest_amount: invest_amount.ensure_sub(amount)?,
						inner: inner.set_redeem_amount(amount)?,
					})
				}
			}
		}
	}

	fn handle_decrease(&self, amount: Balance) -> Result<Self, DispatchError> {
		let error_not_redeeming = Err(DispatchError::Other(
			"Invalid redeem state when transitioning a decrease",
		));

		match self.get_redeeming_amount() {
			None => error_not_redeeming,
			// Can only decrease up to current redeeming amount
			Some(redeem_amount) if redeem_amount <= amount => {
				Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
			}
			// Entire redeeming amount becomes invested amount, i.e. remove `Redeeming` from inner
			// state
			Some(redeem_amount) if redeem_amount == amount => match self {
				Self::NoState | Self::Invested { .. } => error_not_redeeming,
				Self::Invested { invest_amount } => error_not_redeeming,
				Self::NotInvestedAnd { inner } => match inner {
					Redeeming { .. } => Ok(Self::Invested {
						invest_amount: amount,
					}),
					_ => Ok(Self::InvestedAnd {
						invest_amount: amount,
						inner: inner.set_redeem_amount(Balance::zero())?,
					}),
				},
				Self::InvestedAnd {
					invest_amount,
					inner,
				} => {
					let invest_amount = invest_amount.ensure_add(amount)?;
					match inner {
						Redeeming { .. } => Ok(Self::Invested { invest_amount }),
						_ => Ok(Self::InvestedAnd {
							invest_amount,
							inner: inner.set_redeem_amount(Balance::zero())?,
						}),
					}
				}
			},
			// Partial redeeming amount becomes invested amount, i.e. keep `Redeeming` in inner
			// state
			Some(old_redeem_amount) => {
				let redeem_amount = old_redeem_amount.ensure_sub(amount)?;

				match self {
					Self::NoState | Self::Invested { .. } => error_not_redeeming,
					Self::Invested { invest_amount } => error_not_redeeming,
					Self::NotInvestedAnd { inner } => Ok(Self::InvestedAnd {
						invest_amount: amount,
						inner: inner.set_redeem_amount(redeem_amount)?,
					}),
					Self::InvestedAnd {
						invest_amount,
						inner,
					} => {
						let invest_amount = invest_amount.ensure_add(amount)?;
						Ok(Self::InvestedAnd {
							invest_amount,
							inner: inner.set_redeem_amount(redeem_amount)?,
						})
					}
				}
			}
		}
	}

	fn handle_fulfilled_swap_order(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match self {
			Self::NoState | Self::Invested { .. } => Err(DispatchError::Other(
				"Invalid invest state when transitioning a fulfilled order",
			)),
			Self::NotInvestedAnd { inner } => Ok(Self::NotInvestedAnd {
				inner: inner.transition_fulfilled_swap_order(swap)?,
			}),
			Self::InvestedAnd {
				invest_amount,
				inner,
			} => Ok(Self::InvestedAnd {
				invest_amount: *invest_amount,
				inner: inner.transition_fulfilled_swap_order(swap)?,
			}),
		}
	}
}
