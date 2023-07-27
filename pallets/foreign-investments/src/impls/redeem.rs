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
	types::{InnerRedeemState, InvestTransition, RedeemState, RedeemTransition, Swap},
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
			Self::NotInvestedAnd(inner) => inner.get_active_swap(),
			Self::InvestedAnd(inner) => inner.get_active_swap(),
		}
	}

	// TODO: Mayb remove or add docs
	pub(crate) fn swap_inner_state(&self, inner: InnerRedeemState<Balance, Currency>) -> Self {
		match self {
			Self::InvestedAnd(_) => Self::InvestedAnd(inner),
			Self::NotInvestedAnd(_) => Self::NotInvestedAnd(inner),
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
			Self::InvestedAnd(inner) => {
				Ok(Self::InvestedAnd(inner.fulfill_active_swap_amount(amount)?))
			}
			Self::NotInvestedAnd(inner) => Ok(Self::NotInvestedAnd(
				inner.fulfill_active_swap_amount(amount)?,
			)),
			_ => Err(DispatchError::Other(
				"Cannot alter active swap amount for RedeemStates without swap",
			)),
		}
	}
}

impl<Balance, Currency> InnerRedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	/// Returns the potentially existing active swap into return currency:
	/// * If the inner state includes `ActiveSwapIntoReturnCurrency`, it returns
	///   the corresponding `Some(swap)`.
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
				"Cannot increase done_amount of InnerRedeemState if it does not include active swap",
			))
		}
	}
}

// Actual impl of transition
impl<Balance, Currency> RedeemState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
{
	fn handle_increase(
		&self,
		amount: Balance,
	) -> Result<RedeemState<Balance, Currency>, DispatchError> {
		todo!()
		// match self {}
	}

	fn handle_decrease(
		&self,
		amount: Balance,
	) -> Result<RedeemState<Balance, Currency>, DispatchError> {
		todo!()
	}

	fn handle_fulfilled_swap_order(
		&self,
		swap: Swap<Balance, Currency>,
	) -> Result<RedeemState<Balance, Currency>, DispatchError> {
		todo!()
	}
}
