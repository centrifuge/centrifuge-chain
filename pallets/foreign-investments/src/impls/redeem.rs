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
use frame_support::{dispatch::fmt::Debug, ensure};
use sp_runtime::{
	traits::{EnsureAdd, EnsureSub},
	ArithmeticError, DispatchError,
};

use crate::types::{
	InnerRedeemState,
	InnerRedeemState::{
		ActiveSwapIntoForeignCurrency, ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone,
		CollectableRedemption, CollectableRedemptionAndActiveSwapIntoForeignCurrency,
		CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone,
		CollectableRedemptionAndSwapIntoForeignDone, Redeeming,
		RedeemingAndActiveSwapIntoForeignCurrency,
		RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone,
		RedeemingAndCollectableRedemption,
		RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency,
		RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone,
		RedeemingAndCollectableRedemptionAndSwapIntoForeignDone, RedeemingAndSwapIntoForeignDone,
		SwapIntoForeignDone,
	},
	RedeemState, RedeemTransition,
};

impl<Balance, Currency> RedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + Copy + PartialEq + Debug,
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
			RedeemTransition::IncreaseRedeemOrder(amount) => Self::handle_increase(self, amount),
			RedeemTransition::DecreaseRedeemOrder(amount) => Self::handle_decrease(self, amount),
			RedeemTransition::FulfillSwapOrder(swap) => {
				Self::handle_fulfilled_swap_order(self, swap)
			}
			RedeemTransition::Collect(swap) => Self::handle_collect(self, swap),
			RedeemTransition::EpochExecution(amount_unprocessed) => {
				Self::handle_epoch_execution(self, amount_unprocessed)
			}
		}
	}

	/// Returns the potentially existing active swap into foreign currency of
	/// the inner state:
	/// * If the inner state includes `ActiveSwapIntoForeignCurrency`, it
	///   returns the corresponding `Some(swap)`.
	/// * Else, it returns `None`.
	pub(crate) fn get_active_swap(&self) -> Option<Swap<Balance, Currency>> {
		match self {
			Self::NoState => None,
			Self::Invested { .. } => None,
			Self::NotInvestedAnd { inner } | Self::InvestedAnd { inner, .. } => {
				inner.get_active_swap()
			}
		}
	}

	/// Returns the redeeming amount of the inner state, if existent. Else
	/// returns zero.
	pub(crate) fn get_redeeming_amount(&self) -> Balance {
		match self {
			Self::NoState | Self::Invested { .. } => Balance::zero(),
			Self::NotInvestedAnd { inner } | Self::InvestedAnd { inner, .. } => {
				inner.get_redeeming_amount()
			}
		}
	}

	/// Returns the potentially existing invest, i.e. the upper redemption
	/// bound.
	pub(crate) fn get_invested_amount(&self) -> Option<Balance> {
		match self {
			Self::Invested { invest_amount } | Self::InvestedAnd { invest_amount, .. } => {
				Some(*invest_amount)
			}
			_ => None,
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

	/// Reduce the amount of an active swap (into foreign currency) of the
	/// `InnerRedeemState` by the provided value:
	/// * If the provided value equals the swap amount, the state is
	///   transitioned into `*AndSwapIntoForeignDone`.
	/// * Else, it is transitioned into
	///   `*ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone`.
	///
	/// NOTE: Throws if any of the following holds true:
	/// * The outer `RedeemState` is not `InvestedAnd` or `NotInvested` as this
	///   implies there is no active swap.
	/// * The inner state is not an active swap, i.e. the state does not include
	///   `ActiveSwapIntoForeignCurrency`.
	/// * The reducible amount exceeds the active swap amount.
	pub(crate) fn fulfill_active_swap_amount(
		&self,
		amount: Balance,
	) -> Result<Self, DispatchError> {
		match self {
			Self::InvestedAnd { inner, .. } | Self::NotInvestedAnd { inner } => Ok(
				Self::swap_inner_state(self, inner.fulfill_active_swap_amount(amount)?),
			),
			_ => Err(DispatchError::Other(
				"Cannot alter active swap amount for RedeemStates without swap",
			)),
		}
	}

	/// Update or kill the unprocessed redemption amount of the inner state.
	/// * If the outer state does not include an inner state with `Redeeming`,
	///   there is nothing to transition, i.e. we return the current state
	/// * Else If the provided `unprocessed_amount` is zero, remove `Redeeming`
	///   from the inner state
	/// * Else set the `redeem_amount` to `unprocessed_amount`
	fn handle_epoch_execution(&self, amount_unprocessed: Balance) -> Result<Self, DispatchError> {
		match *self {
			RedeemState::NoState | RedeemState::Invested { .. } => Ok(*self),
			RedeemState::NotInvestedAnd { inner } => match inner {
				Redeeming { .. } if !amount_unprocessed.is_zero() => Ok(Self::Invested {
					invest_amount: amount_unprocessed,
				}),
				state => Ok(RedeemState::NotInvestedAnd {
					inner: state.set_existing_redeem_amount(amount_unprocessed)?,
				}),
			},
			RedeemState::InvestedAnd {
				inner,
				invest_amount,
			} => match inner {
				Redeeming { .. } if !amount_unprocessed.is_zero() => Ok(Self::Invested {
					invest_amount: amount_unprocessed,
				}),
				state => Ok(RedeemState::InvestedAnd {
					inner: state.set_existing_redeem_amount(amount_unprocessed)?,
					invest_amount,
				}),
			},
		}
	}
}

impl<Balance, Currency> InnerRedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + Copy + PartialEq + Debug,
{
	/// Returns the potentially existing active swap into foreign currency:
	/// * If the state includes `ActiveSwapIntoForeignCurrency`, it returns the
	///   corresponding `Some(swap)`.
	/// * Else, it returns `None`.
	fn get_active_swap(&self) -> Option<Swap<Balance, Currency>> {
		match *self {
			Self::ActiveSwapIntoForeignCurrency { swap } |
			Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, .. } |
			Self::RedeemingAndActiveSwapIntoForeignCurrency { swap, .. } |
			Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, .. } |
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { swap, .. } |
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, .. } |
			Self::CollectableRedemptionAndActiveSwapIntoForeignCurrency { swap, .. } |
			Self::CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, .. } => Some(swap),
			_ => None,
		}
	}

	/// Returns the redeeming amount if existent. Else returns zero.
	fn get_redeeming_amount(&self) -> Balance {
		match *self {
			Self::Redeeming { redeem_amount } |
			Self::RedeemingAndCollectableRedemption { redeem_amount, .. } |
			Self::RedeemingAndActiveSwapIntoForeignCurrency { redeem_amount, .. } |
			Self::RedeemingAndSwapIntoForeignDone { redeem_amount, .. } |
			Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount, .. } |
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { redeem_amount, .. } |
			Self::RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { redeem_amount, .. } |
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount, .. } => redeem_amount,
			_ => Balance::zero(),
		}
	}

	/// Reduce the amount of an active swap (into foreign currency) by the
	/// provided value:
	/// * Throws if there is no active swap, i.e. the state does not include
	///   `ActiveSwapIntoForeignCurrency` or if the reducible amount exceeds the
	///   swap amount
	/// * If the provided value equals the swap amount, the state is
	///   transitioned into `*AndSwapIntoForeignDone`.
	/// * Else, it is transitioned into
	///   `*ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone`.
	fn fulfill_active_swap_amount(&self, amount: Balance) -> Result<Self, DispatchError> {
		match self {
			Self::ActiveSwapIntoForeignCurrency { swap } => {
				if amount == swap.amount{
					Ok(Self::SwapIntoForeignDone { done_swap: *swap })
				} else {
					Ok(
						Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount: amount
						}
					)
				}
			},
			Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(
						Self::SwapIntoForeignDone {
							done_swap: Swap {
								amount: done_amount,
								..*swap
							}
						}
					)
				} else {
					Ok(
						Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount,
						}
					)
				}
			},
			Self::RedeemingAndActiveSwapIntoForeignCurrency { redeem_amount, swap } => {
				if amount == swap.amount {
					Ok(
						Self::RedeemingAndSwapIntoForeignDone {
							done_swap: Swap {
								amount,
								..*swap
							},
							redeem_amount: *redeem_amount,
						}
					)
				} else {
					Ok(
						Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
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
			Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(
						Self::RedeemingAndSwapIntoForeignDone {
							done_swap: Swap {
								amount: done_amount,
								..*swap
							},
							redeem_amount: *redeem_amount,
						}
					)
				} else {
					Ok(
						Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
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
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { redeem_amount, swap } => {
				if amount == swap.amount {
					Ok(
						Self::RedeemingAndCollectableRedemptionAndSwapIntoForeignDone {
							done_swap: *swap,
							redeem_amount: *redeem_amount,
						}
					)
				} else {
					Ok(
						Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
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
			Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(
						Self::RedeemingAndCollectableRedemptionAndSwapIntoForeignDone {
							done_swap: Swap {
								amount: done_amount,
								..*swap
							},
							redeem_amount: *redeem_amount,
						}
					)
				} else {
					Ok(
						Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
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
			Self::CollectableRedemptionAndActiveSwapIntoForeignCurrency { swap } => {
				if amount == swap.amount {
					Ok(
						Self::CollectableRedemptionAndSwapIntoForeignDone {
							done_swap: *swap,
						}
					)
				} else {
					Ok(
						Self::CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount: amount,
						}
						)
				}
			},
			Self::CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;

				if amount == swap.amount {
					Ok(
						Self::CollectableRedemptionAndSwapIntoForeignDone {
							done_swap: Swap {
								amount: done_amount,
								..*swap
							},
						}
					)
				} else {
					Ok(
						Self::CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
							swap: Swap {
								amount: swap.amount.ensure_sub(amount)?,
								..*swap
							},
							done_amount,
						}
					)
				}
			},
			_ => Err(DispatchError::Other(
				"Invalid inner redeem state when fulfilling active swap amount",
			))
		}
	}

	/// Removes `Redeeming` from the state.
	fn remove_redeem_amount(&self) -> Result<Self, DispatchError> {
		match *self {
			Redeeming { .. } => Err(DispatchError::Other("Outer RedeemState must be transitioned to Self::Invested")),
			RedeemingAndCollectableRedemption { .. } => Ok(CollectableRedemption),
			RedeemingAndActiveSwapIntoForeignCurrency { swap, .. } => Ok(ActiveSwapIntoForeignCurrency { swap }),
			RedeemingAndSwapIntoForeignDone { done_swap, .. } => Ok(SwapIntoForeignDone { done_swap }),
			RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount, .. } => Ok(ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount }),
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { swap, .. } => Ok(CollectableRedemptionAndActiveSwapIntoForeignCurrency { swap }),
			RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { done_swap, .. } => Ok(CollectableRedemptionAndSwapIntoForeignDone { done_swap }),
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount, .. } => Ok(CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount }),
			// Throw for states without `Redeeming`
			_ => Err(DispatchError::Other("Cannot remove redeeming amount of inner redeem state which does not include `Redeeming`")),
		}
	}

	/// Either adds a non existing redeeming amount to the state or overwrites
	/// it.
	/// * If the value is not zero and the state involves `Redeeming`: Sets the
	///   amount.
	/// * Else if the value is not zero and the state does not involve
	///   `Redeeming`: Adds `Redeeming` to the state with the corresponding
	///   amount.
	/// * If the value is zero and the state includes `Redeeming`: Removes
	///   `Redeeming` from the state.
	/// * Else throws.
	fn add_or_overwrite_redeem_amount(&self, amount: Balance) -> Result<Self, DispatchError> {
		if amount.is_zero() {
			return Self::remove_redeem_amount(self);
		}
		match *self {
			Redeeming { .. } => Ok(Redeeming { redeem_amount: amount }),
			RedeemingAndCollectableRedemption { .. } => Ok(RedeemingAndCollectableRedemption { redeem_amount: amount }),
			RedeemingAndActiveSwapIntoForeignCurrency { swap, .. } => Ok(RedeemingAndActiveSwapIntoForeignCurrency { redeem_amount: amount, swap }),
			RedeemingAndSwapIntoForeignDone { done_swap, .. } => Ok(RedeemingAndSwapIntoForeignDone { redeem_amount: amount, done_swap }),
			RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount, .. } => Ok(RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount: amount, swap, done_amount }),
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { swap, .. } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { redeem_amount: amount, swap }),
			RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { done_swap, .. } => Ok(RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { redeem_amount: amount, done_swap }),
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount, .. } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount: amount, swap, done_amount }),
			CollectableRedemption => Ok(RedeemingAndCollectableRedemption { redeem_amount: amount }),
			ActiveSwapIntoForeignCurrency { swap } => Ok(RedeemingAndActiveSwapIntoForeignCurrency { swap, redeem_amount: amount }),
			ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => Ok(RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount, redeem_amount: amount }),
			SwapIntoForeignDone { done_swap } => Ok(RedeemingAndSwapIntoForeignDone { done_swap, redeem_amount: amount }),
			CollectableRedemptionAndActiveSwapIntoForeignCurrency { swap } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { swap, redeem_amount: amount }),
			CollectableRedemptionAndSwapIntoForeignDone { done_swap } => Ok(RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { done_swap, redeem_amount: amount }),
			CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount, redeem_amount: amount }),
		}
	}

	/// Sets the redeeming amount of the state to the given amount.
	///
	/// Throws if the the state does not include `Redeeming`.
	fn set_existing_redeem_amount(&self, amount: Balance) -> Result<Self, DispatchError> {
		if amount.is_zero() {
			return Self::remove_redeem_amount(self);
		}
		match *self {
			Redeeming { .. } => Ok(Redeeming { redeem_amount: amount }),
			RedeemingAndCollectableRedemption { .. } => Ok(RedeemingAndCollectableRedemption { redeem_amount: amount }),
			RedeemingAndActiveSwapIntoForeignCurrency { swap, .. } => Ok(RedeemingAndActiveSwapIntoForeignCurrency { redeem_amount: amount, swap }),
			RedeemingAndSwapIntoForeignDone { done_swap, .. } => Ok(RedeemingAndSwapIntoForeignDone { redeem_amount: amount, done_swap }),
			RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount, .. } => Ok(RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount: amount, swap, done_amount }),
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { swap, .. } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { redeem_amount: amount, swap }),
			RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { done_swap, .. } => Ok(RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { redeem_amount: amount, done_swap }),
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount, .. } => Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount: amount, swap, done_amount }),
			_ => Err(DispatchError::Other("Cannot set existing redeem amount of inner redeem state which does not include `Redeeming`")),
		}
	}

	/// Transition all inner states which include
	/// `ActiveSwapIntoForeignCurrency`. The transitioned state either includes
	/// `*SwapIntoForeignDone` or
	/// `*ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone`.
	///
	/// Also supports non-foreign swaps, i.e. those with matching in and out
	/// currency.
	///
	/// Throws if the fulfilled swap direction is not into foreign currency or
	/// if the amount exceeds the states active swap amount.
	///
	/// NOTE: We can ignore all states which do not include
	/// `ActiveSwapIntoForeignCurrency`.
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

		// Edge case: if currency_in matches currency_out, we can immediately fulfill
		// the swap
		match *self {
			ActiveSwapIntoForeignCurrency { swap } => {
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount: amount })
				} else {
					Ok(SwapIntoForeignDone { done_swap: swap })
				}
			},
			ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount })
				} else {
					Ok(SwapIntoForeignDone { done_swap: Swap { amount: done_amount, ..swap } })
				}
			},
			RedeemingAndActiveSwapIntoForeignCurrency { redeem_amount, swap } => {
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount: amount, redeem_amount })
				} else {
					Ok(RedeemingAndSwapIntoForeignDone { done_swap: swap, redeem_amount })
				}
			},
			RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount, redeem_amount })
				} else {
					Ok(RedeemingAndSwapIntoForeignDone { done_swap: Swap { amount: done_amount, ..swap }, redeem_amount })
				}
			},
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { redeem_amount, swap } => {
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount: amount, redeem_amount })
				} else {
					Ok(RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { done_swap: swap, redeem_amount })
				}
			},
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount, swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount, redeem_amount })
				} else {
					Ok(RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { done_swap: Swap { amount: done_amount, ..swap }, redeem_amount })
				}
			},
			CollectableRedemptionAndActiveSwapIntoForeignCurrency { swap } => {
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount: amount })
				} else {
					Ok(CollectableRedemptionAndSwapIntoForeignDone { done_swap: swap })
				}
			},
			CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				let done_amount = done_amount.ensure_add(amount)?;
				if amount < swap.amount && swap.currency_in != swap.currency_out {
					Ok(CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap: Swap { amount: swap.amount - amount, ..swap }, done_amount })
				} else {
					Ok(CollectableRedemptionAndSwapIntoForeignDone { done_swap: Swap { amount: done_amount, ..swap } })
				}
			},
			_ => Err(DispatchError::Other("Invalid inner redeem state when transitioning fulfilled swap order")),
		}
	}

	/// Apply the transition of the state after collecting a redemption:
	/// * If the collected amount (in pool currency) is positive, this indicates
	///   that we need to initiate the swap into foreign currency
	/// * If the collected amount is zero, this indicates that the collection is
	///   considered to be done.
	///
	/// Throws if
	/// * The current state includes an active/done swap and in and out
	///   currencies do not match the provided ones; or
	/// * The collected amount is zero but the state does not include a foreign
	///   `ActiveSwapIntoForeignCurrency` or `SwapIntoForeignDone`
	fn transition_collect(
		&self,
		collected_swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		ensure!(
			self.get_active_swap()
				.map(|swap| (swap.currency_in, swap.currency_out)
					== (collected_swap.currency_in, collected_swap.currency_out))
				.unwrap_or(true),
			DispatchError::Other("Invalid swap currencies when transitioning collect redemption")
		);

		if collected_swap.currency_in == collected_swap.currency_out {
			return Self::transition_collect_non_foreign(self, collected_swap);
		}

		// A collectable redemption is considered to be _done_ iff the amount of pool
		// currency returned after calling `collect_redeem` is zero
		match *self {
			CollectableRedemption => {
				if collected_swap.amount.is_zero() {
					Err(DispatchError::Other("Cannot clear CollectableRedemption if the collected amount is zero and state does not include swap"))
				} else {
					Ok(Self::CollectableRedemptionAndActiveSwapIntoForeignCurrency {
						swap: collected_swap,
					})
				}
			},
			RedeemingAndCollectableRedemption { redeem_amount } => {
				if collected_swap.amount.is_zero() {
					Err(DispatchError::Other("Cannot clear CollectableRedemption if the collected amount is zero and state does not include swap"))
				} else {
					Ok(Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency {
						redeem_amount,
						swap: collected_swap,
					})
				}
			},
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency { redeem_amount, swap } => {
				if collected_swap.amount.is_zero() {
					Ok(Self::RedeemingAndActiveSwapIntoForeignCurrency {
						redeem_amount,
						swap
					})
				}
				else {
					Ok(Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrency {
						redeem_amount,
						swap: Swap {
							amount: swap.amount.ensure_add(collected_swap.amount)?,
							..collected_swap
						}
					})
				}
			},
			RedeemingAndCollectableRedemptionAndSwapIntoForeignDone { redeem_amount, done_swap } => {
				if collected_swap.amount.is_zero() {
					Ok(Self::RedeemingAndSwapIntoForeignDone {
						redeem_amount,
						done_swap,
					})
				} else {
					Ok(Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						redeem_amount,
						swap: Swap {
							amount: collected_swap.amount,
							..collected_swap
						},
						done_amount: done_swap.amount
					})
				}
			},
			RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { redeem_amount, swap, done_amount } => {
				if collected_swap.amount.is_zero() {
					Ok(Self::RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						redeem_amount,
						swap,
						done_amount
					})
				} else {
					Ok(Self::RedeemingAndCollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						redeem_amount,
						swap: Swap {
							amount: swap.amount.ensure_add(collected_swap.amount)?,
							..collected_swap
						},
						done_amount
					})
				}
			},
			CollectableRedemptionAndActiveSwapIntoForeignCurrency { swap } => {
				if collected_swap.amount.is_zero() {
					Ok(Self::ActiveSwapIntoForeignCurrency {
						swap,
					})
				} else {
					Ok(Self::CollectableRedemptionAndActiveSwapIntoForeignCurrency {
						swap: Swap {
							amount: swap.amount.ensure_add(collected_swap.amount)?,
							..collected_swap
						},
					})
				}
			},
			CollectableRedemptionAndSwapIntoForeignDone { done_swap } => {
				if collected_swap.amount.is_zero() {
					Ok(Self::SwapIntoForeignDone {
						done_swap,
					})
				} else {
					Ok(Self::CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap: Swap {
							amount: collected_swap.amount,
							..collected_swap
						},
						done_amount: done_swap.amount,
					})
				}
			},
			CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				if collected_swap.amount.is_zero() {
					Ok(Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap,
						done_amount
					})
				} else {
					Ok(Self::CollectableRedemptionAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
						swap: Swap {
							amount: swap.amount.ensure_add(collected_swap.amount)?,
							..collected_swap
						},
						done_amount
					})
				}
			},
			state => Ok(state)
		}
	}

	/// Apply the transition of the state after collecting a redemption in
	/// non-foreign currencies.
	///  * Ignores any states without `CollectableRedemption`.
	///  * Throws for all states with `CollectableRedemption` and
	///    `ActiveSwapIntoForeignCurrency` as there can't be an active swap for
	///    non-foreign currencies, these should immediately fulfilled.
	///  * Else replaces `CollectableRedemption` with `SwapIntoForeignDone` if
	///    it did not exist already. If it did, increment the done swap amount
	///    by the collected one.
	fn transition_collect_non_foreign(
		&self,
		collected_swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match *self {
			CollectableRedemption => {
				if collected_swap.amount.is_zero() {
					Err(DispatchError::Other("Cannot clear CollectableRedemption if the collected amount is zero and state does not include done swap"))
				} else {
					Ok(Self::CollectableRedemptionAndSwapIntoForeignDone {
						done_swap: collected_swap,
					})
				}
			}
			RedeemingAndCollectableRedemption { redeem_amount } => {
				if collected_swap.amount.is_zero() {
					Err(DispatchError::Other("Cannot clear CollectableRedemption if the collected amount is zero and state does not include done swap"))
				} else {
					Ok(Self::RedeemingAndSwapIntoForeignDone {
						redeem_amount,
						done_swap: collected_swap,
					})
				}
			}

			RedeemingAndCollectableRedemptionAndSwapIntoForeignDone {
				redeem_amount,
				done_swap,
			} => Ok(RedeemingAndSwapIntoForeignDone {
				redeem_amount,
				done_swap: Swap {
					amount: done_swap.amount.ensure_add(collected_swap.amount)?,
					..collected_swap
				},
			}),
			CollectableRedemptionAndSwapIntoForeignDone { done_swap } => Ok(SwapIntoForeignDone {
				done_swap: Swap {
					amount: done_swap.amount.ensure_add(collected_swap.amount)?,
					..collected_swap
				},
			}),
			_ => Err(DispatchError::Other(
				"Invalid pre state when transitioning collect for same currencies",
			)),
		}
	}
}

// Actual impl of transition
impl<Balance, Currency> RedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + Copy + PartialEq + Debug,
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
						inner: inner.add_or_overwrite_redeem_amount(amount)?,
					})
				} else {
					Ok(Self::InvestedAnd {
						invest_amount: invest_amount.ensure_sub(amount)?,
						inner: inner.add_or_overwrite_redeem_amount(amount)?,
					})
				}
			}
			_ => Err(DispatchError::Other(
				"Invalid redeem state when transitioning an increase",
			)),
		}
	}

	/// Handle `decrease` transitions depicted by `msg::decrease` edges in the
	/// redeem state diagram:
	/// * If the current inner state includes an unprocessed redemption, i.e. is
	///   `InnerRedeemState::Redeeming`, decreases the redeeming amount up to
	///   its max. Throws if the decrement amount exceeds the previously
	///   increased redemption amount.
	/// * Else throws for incorrect pre state.
	fn handle_decrease(&self, amount: Balance) -> Result<Self, DispatchError> {
		let error_not_redeeming = Err(DispatchError::Other(
			"Invalid redeem state when transitioning a decrease",
		));

		match self.get_redeeming_amount() {
			amount if amount.is_zero() => error_not_redeeming,
			// Can only decrease up to current redeeming amount
			redeem_amount if redeem_amount <= amount => {
				Err(DispatchError::Arithmetic(ArithmeticError::Underflow))
			}
			// Entire redeeming amount becomes invested amount, i.e. remove `Redeeming` from inner
			// state
			redeem_amount if redeem_amount == amount => match self {
				Self::NoState | Self::Invested { .. } => error_not_redeeming,
				Self::NotInvestedAnd { inner } => match inner {
					Redeeming { .. } => Ok(Self::Invested {
						invest_amount: amount,
					}),
					_ => Ok(Self::InvestedAnd {
						invest_amount: amount,
						inner: inner.add_or_overwrite_redeem_amount(Balance::zero())?,
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
							inner: inner.add_or_overwrite_redeem_amount(Balance::zero())?,
						}),
					}
				}
			},
			// Partial redeeming amount becomes invested amount, i.e. keep `Redeeming` in inner
			// state
			old_redeem_amount => {
				let redeem_amount = old_redeem_amount.ensure_sub(amount)?;

				match self {
					Self::NoState | Self::Invested { .. } => error_not_redeeming,
					Self::NotInvestedAnd { inner } => Ok(Self::InvestedAnd {
						invest_amount: amount,
						inner: inner.add_or_overwrite_redeem_amount(redeem_amount)?,
					}),
					Self::InvestedAnd {
						invest_amount,
						inner,
					} => {
						let invest_amount = invest_amount.ensure_add(amount)?;
						Ok(Self::InvestedAnd {
							invest_amount,
							inner: inner.add_or_overwrite_redeem_amount(redeem_amount)?,
						})
					}
				}
			}
		}
	}

	/// Update the inner state if it includes `ActiveSwapIntoForeignCurrency`.
	fn handle_fulfilled_swap_order(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match self {
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
			_ => Err(DispatchError::Other(
				"Invalid redeem state when transitioning a fulfilled order",
			)),
		}
	}

	/// Remove `CollectableRedemption` from all inner states which include it.
	/// Either swap it with `ActiveSwapIntoForeignCurrency` if the inner state
	/// did not include an active swap or simply drop it.
	///
	/// Throws if the state does not allow for collection or the the inner state
	/// includes an active/done swap with mismatching currencies to the provided
	/// ones.
	fn handle_collect(&self, swap: Swap<Balance, Currency>) -> Result<Self, DispatchError> {
		match self {
			RedeemState::NoState | RedeemState::Invested { .. } => Err(DispatchError::Other(
				"Invalid redeem state when transitioning collect",
			)),
			RedeemState::NotInvestedAnd { inner } | RedeemState::InvestedAnd { inner, .. } => Ok(
				Self::swap_inner_state(self, inner.transition_collect(swap)?),
			),
		}
	}
}
