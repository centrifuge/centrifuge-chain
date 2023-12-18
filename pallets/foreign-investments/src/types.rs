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

use cfg_traits::IdentityCurrencyConversion;
use cfg_types::investments::Swap;
use frame_support::{dispatch::fmt::Debug, RuntimeDebugNoBound};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::{EnsureAdd, EnsureSub, Zero};

/// Reflects the reason for the last token swap update such that it can be
/// updated accordingly if the last and current reason mismatch.
#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
pub enum TokenSwapReason {
	Investment,
	Redemption,
	InvestmentAndRedemption,
}

/// Restriction of `pallet_foreign_investments::Config` trait to support
/// currency conversion in the `InvestState`.
pub trait InvestStateConfig {
	type Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug + Zero;
	type CurrencyId: Clone + Copy + PartialEq + Debug;
	type CurrencyConverter: IdentityCurrencyConversion<
		Balance = Self::Balance,
		Currency = Self::CurrencyId,
		Error = sp_runtime::DispatchError,
	>;
}

/// Reflects all states a foreign investment can have until it is processed as
/// an investment via `<T as Config>::Investment`. This includes swapping it
/// into a pool currency or back, if the investment is decreased before it is
/// fully processed.
#[derive(
	PartialOrd, Ord, PartialEq, Eq, RuntimeDebugNoBound, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub enum InvestState<T: InvestStateConfig> {
	/// Default state for initialization which will never be actively put into
	/// chain state, i.e. if this state is the result of applying transition(s),
	/// then the corresponding `InvestmentState` will be cleared.
	NoState,
	/// The investment is waiting to be processed.
	InvestmentOngoing { invest_amount: T::Balance },
	/// The investment is currently swapped into the required pool currency.
	ActiveSwapIntoPoolCurrency {
		swap: Swap<T::Balance, T::CurrencyId>,
	},
	/// The unprocessed investment was fully decreased and is currently swapped
	/// back into the corresponding foreign currency.
	ActiveSwapIntoForeignCurrency {
		swap: Swap<T::Balance, T::CurrencyId>,
	},
	/// The investment is not fully swapped into pool currency and thus split
	/// into two parts:
	/// * One part is still being swapped.
	/// * The remainder is already waiting to be processed as investment.
	ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
		swap: Swap<T::Balance, T::CurrencyId>,
		invest_amount: T::Balance,
	},
	/// The investment is split into two parts:
	/// * One part is waiting to be processed as investment.
	/// * The remainder is swapped back into the foreign currency as a result of
	///   decreasing the invested amount before being processed.
	ActiveSwapIntoForeignCurrencyAndInvestmentOngoing {
		swap: Swap<T::Balance, T::CurrencyId>,
		invest_amount: T::Balance,
	},
	/// The investment is split into two parts:
	/// * The one part is swapping into pool currency.
	/// * The remainder was swapped back into the foreign currency as a result
	///   of decreasing the invested amount before being processed.
	///
	/// NOTE: This state is transitioned into `ActiveSwapIntoPoolCurrency`
	/// in the post-processing `apply_invest_state_transition` as the done part
	/// invokes `ExecutedDecreaseInvestOrder` dispatch.
	ActiveSwapIntoPoolCurrencyAndSwapIntoForeignDone {
		swap: Swap<T::Balance, T::CurrencyId>,
		done_amount: T::Balance,
	},
	/// The investment is swapped back into the foreign currency and was already
	/// partially fulfilled.
	ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
		swap: Swap<T::Balance, T::CurrencyId>,
		done_amount: T::Balance,
	},
	/// The investment is split into three parts:
	/// * One part is currently swapping into the pool currency.
	/// * The second is already waiting to be processed as investment.
	/// * The remainder was swapped back into the foreign currency as a result
	///   of decreasing the invested amount before being processed.
	///
	/// NOTE: This state is transitioned into
	/// `ActiveSwapIntoPoolCurrencyAndInvestmentOngoing` in the post-processing
	/// `apply_invest_state_transition` as the done part invokes
	/// `ExecutedDecreaseInvestOrder` dispatch.
	ActiveSwapIntoPoolCurrencyAndSwapIntoForeignDoneAndInvestmentOngoing {
		swap: Swap<T::Balance, T::CurrencyId>,
		done_amount: T::Balance,
		invest_amount: T::Balance,
	},
	/// The investment is split into three parts:
	/// * One is waiting to be processed as investment.
	/// * The second is swapped back into the foreign currency as a result of
	///   decreasing the invested amount before being processed.
	/// * The remainder was already swapped back into the foreign currency.
	///
	/// NOTE: This state must not be transitioned by applying the trigger for
	/// the done part but wait until the active swap is fulfilled.
	ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDoneAndInvestmentOngoing {
		swap: Swap<T::Balance, T::CurrencyId>,
		done_amount: T::Balance,
		invest_amount: T::Balance,
	},
	/// The unprocessed investment was swapped back into foreign currency.
	///
	/// NOTE: This state is killed in the post-processing
	/// `apply_invest_state_transition` as it invokes
	/// `ExecutedDecreaseInvestOrder` dispatch.
	SwapIntoForeignDone {
		done_swap: Swap<T::Balance, T::CurrencyId>,
	},
	/// The investment is split into two parts:
	/// * One part is waiting to be processed as an investment
	/// * The swapped back into the foreign currency as a result of decreasing
	///   the invested amount before being processed.
	///
	/// NOTE: This state is transitioned into `InvestmentOngoing`  in the
	/// post-processing `apply_invest_state_transition` as the done part invokes
	/// `ExecutedDecreaseInvestOrder` dispatch.
	SwapIntoForeignDoneAndInvestmentOngoing {
		done_swap: Swap<T::Balance, T::CurrencyId>,
		invest_amount: T::Balance,
	},
}
// NOTE: Needed because `T` of `InvestState<T>` cannot be restricted to impl
// Default
impl<T: InvestStateConfig> Default for InvestState<T> {
	fn default() -> Self {
		Self::NoState
	}
}

// NOTE: Needed because `T` of `InvestState<T>` cannot be restricted to impl
// Copy
impl<T: InvestStateConfig> Clone for InvestState<T>
where
	T::Balance: Clone,
	T::CurrencyId: Clone,
	Swap<T::Balance, T::CurrencyId>: Clone,
{
	fn clone(&self) -> Self {
		match self {
			Self::NoState => Self::NoState,
			Self::InvestmentOngoing { invest_amount } => Self::InvestmentOngoing {
				invest_amount: *invest_amount,
			},
			Self::ActiveSwapIntoPoolCurrency { swap } => {
				Self::ActiveSwapIntoPoolCurrency { swap: *swap }
			}
			Self::ActiveSwapIntoForeignCurrency { swap } => {
				Self::ActiveSwapIntoForeignCurrency { swap: *swap }
			}
			Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
				swap,
				invest_amount,
			} => Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
				swap: *swap,
				invest_amount: *invest_amount,
			},
			Self::ActiveSwapIntoForeignCurrencyAndInvestmentOngoing {
				swap,
				invest_amount,
			} => Self::ActiveSwapIntoForeignCurrencyAndInvestmentOngoing {
				swap: *swap,
				invest_amount: *invest_amount,
			},
			Self::ActiveSwapIntoPoolCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				Self::ActiveSwapIntoPoolCurrencyAndSwapIntoForeignDone {
					swap: *swap,
					done_amount: *done_amount,
				}
			}
			Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone { swap, done_amount } => {
				Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
					swap: *swap,
					done_amount: *done_amount,
				}
			}
			Self::ActiveSwapIntoPoolCurrencyAndSwapIntoForeignDoneAndInvestmentOngoing {
				swap,
				done_amount,
				invest_amount,
			} => Self::ActiveSwapIntoPoolCurrencyAndSwapIntoForeignDoneAndInvestmentOngoing {
				swap: *swap,
				done_amount: *done_amount,
				invest_amount: *invest_amount,
			},
			Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDoneAndInvestmentOngoing {
				swap,
				done_amount,
				invest_amount,
			} => Self::ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDoneAndInvestmentOngoing {
				swap: *swap,
				done_amount: *done_amount,
				invest_amount: *invest_amount,
			},
			Self::SwapIntoForeignDone { done_swap } => Self::SwapIntoForeignDone {
				done_swap: *done_swap,
			},
			Self::SwapIntoForeignDoneAndInvestmentOngoing {
				done_swap,
				invest_amount,
			} => Self::SwapIntoForeignDoneAndInvestmentOngoing {
				done_swap: *done_swap,
				invest_amount: *invest_amount,
			},
		}
	}
}

/// Reflects all state transitions of an `InvestmentState` which can be
/// externally triggered, i.e. by (partially) fulfilling a token swap order or
/// updating an unprocessed investment.
#[derive(Clone, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum InvestTransition<
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + Copy + PartialEq + Debug,
> {
	/// Assumes `swap.amount` to be denominated in pool currency and
	/// `swap.currency_in` to be pool currency as we increase here.
	IncreaseInvestOrder(Swap<Balance, Currency>),
	/// Assumes `swap.amount` to be denominated in foreign currency and
	/// `swap.currency_in` to be foreign currency as we increase here.
	DecreaseInvestOrder(Swap<Balance, Currency>),
	/// Implicitly derives `swap.currency_in` and `swap.currency_out` from
	/// previous state:
	/// * If the previous state includes `ActiveSwapIntoPoolCurrency`,
	///   `currency_in` is the pool currency.
	/// * If the previous state includes `ActiveSwapIntoForeignCurrency`,
	///   `currency_in` is the foreign currency.
	FulfillSwapOrder(Swap<Balance, Currency>),
	CollectInvestment(Balance),
}

/// Reflects all states a foreign redemption can have until transferred to the
/// corresponding source domain.
///
/// This includes swapping it into a pool currency or back, if the investment is
/// decreased before it is fully processed.
#[derive(
	Clone,
	Copy,
	Default,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum RedeemState<
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + Copy + PartialEq + Debug,
> {
	#[default]
	/// Default state for initialization which will never be actively put into
	/// chain state, i.e. if this state is the result of applying transition(s),
	/// then the corresponding `RedemptionState` will be cleared.
	NoState,
	/// The redemption is pending until it is processed during epoch execution.
	Redeeming { redeem_amount: Balance },
	/// The redemption was fully processed and collected and is currently
	/// swapping into the foreign currency.
	ActiveSwapIntoForeignCurrency { swap: Swap<Balance, Currency> },
	/// The redemption was fully processed, collected and partially swapped into
	/// the foreign currency. It is split into two parts:
	/// * One part is swapping back into the foreign currency.
	/// * The remainder was already swapped back.
	ActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
		swap: Swap<Balance, Currency>,
		done_amount: Balance,
	},
	/// The redemption was fully processed, collected and swapped into the
	/// foreign currency.
	///
	/// NOTE: This state is automatically killed in the post-processing
	/// `apply_collect_redeem_transition` as it prepares the dispatch of
	/// `ExecutedCollectRedeem` message which needs to be triggered manually.
	SwapIntoForeignDone { done_swap: Swap<Balance, Currency> },
	/// The redemption is split into two parts:
	/// * One part is waiting to be processed as redemption.
	/// * The remainder is swapping back into the foreign currency as a result
	///   of processing and collecting beforehand.
	RedeemingAndActiveSwapIntoForeignCurrency {
		redeem_amount: Balance,
		swap: Swap<Balance, Currency>,
	},
	/// The redemption is split into two parts:
	/// * One part is waiting to be processed as redemption.
	/// * The remainder is swapping back into the foreign currency as a result
	///   of processing and collecting beforehand.
	///
	/// NOTE: This state is automatically transitioned into `Redeeming` in the
	/// post-processing `apply_collect_redeem_transition` as the done part
	/// prepares the dispatch of `ExecutedCollectRedeem` message which needs to
	/// be triggered manually.
	RedeemingAndSwapIntoForeignDone {
		redeem_amount: Balance,
		done_swap: Swap<Balance, Currency>,
	},
	/// The redemption is split into three parts:
	/// * One part is waiting to be processed as redemption.
	/// * The second is swapping back into the foreign currency as a result of
	///   processing and collecting beforehand.
	/// * The remainder was already swapped back.
	RedeemingAndActiveSwapIntoForeignCurrencyAndSwapIntoForeignDone {
		redeem_amount: Balance,
		swap: Swap<Balance, Currency>,
		done_amount: Balance,
	},
}

/// Reflects all state transitions of a `RedeemState` which can be
/// externally triggered, i.e. by (partially) fulfilling a token swap order or
/// updating an unprocessed redemption.
#[derive(Clone, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum RedeemTransition<
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + Copy + PartialEq + Debug,
> {
	IncreaseRedeemOrder(Balance),
	DecreaseRedeemOrder(Balance),
	FulfillSwapOrder(Swap<Balance, Currency>),
	CollectRedemption(Balance, Swap<Balance, Currency>),
}
