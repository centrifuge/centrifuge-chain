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

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::{EnsureAdd, EnsureSub};

// TODO: Might want to use this trimmed down version of InvestmentInfo for the
// ForeignInvestmentInfo storage in case we don't need to store the payment
// currency
// #[derive(
// 	Clone, Default, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode,
// TypeInfo, MaxEncodedLen, )]
// pub struct ForeignInvestment<AccountId, InvestmentId> {
// 	pub investor: Balance,
// 	pub investment_id: InvestmentId,
// }

#[derive(
	Clone, Default, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
pub struct Swap<Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord, Currency: Clone + PartialEq> {
	pub currency_in: Currency,
	pub currency_out: Currency,
	pub amount: Balance,
}

/// Reflects all states a foreign investment can be in until it is processed.
/// This includes swapping it into a pool currency or back, if the investment is
/// decreased before it is fully processed.
#[derive(
	Clone, Default, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
pub enum InvestState<
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
> {
	#[default]
	NoState,
	/// The investment is waiting to be processed.
	InvestmentOngoing { invest_amount: Balance },
	/// The investment is currently swapped into the required pool currency.
	ActiveSwapIntoPoolCurrency { swap: Swap<Balance, Currency> },
	/// The unprocessed investment was fully decreased and is currently swapped
	/// back into the corresponding return currency.
	ActiveSwapIntoReturnCurrency { swap: Swap<Balance, Currency> },
	/// The investment is not fully swapped into pool currency and thus split
	/// into two:
	///     * One part is still being swapped.
	///     * The other part is already waiting to be processed as investment.
	ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
		swap: Swap<Balance, Currency>,
		invest_amount: Balance,
	},
	/// The investment is split into two:
	///     * One part is waiting to be processed as investment.
	///     * The other part is swapped back into the return currency as a
	///       result of decreasing the invested amount before being processed.
	ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
		swap: Swap<Balance, Currency>,
		invest_amount: Balance,
	},
	/// The investment is split into two:
	///     * The one part is swapping into pool currency.
	///     * The other part was swapped back into the return currency as a
	///       result of decreasing the invested amount before being processed.
	///
	/// NOTE: This state can be transitioned into `ActiveSwapIntoPoolCurrency`
	/// by applying the corresponding trigger to handle the return amount.
	ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone {
		swap: Swap<Balance, Currency>,
		done_amount: Balance,
	},
	/// The investment is swapped back into the return currency and was already
	/// partially fulfilled.
	ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone {
		swap: Swap<Balance, Currency>,
		done_amount: Balance,
	},
	/// The investment is split into three:
	///     * One part is currently swapping into the pool currency.
	///     * The second part is already waiting to be processed as investment.
	///     * The remaining part was swapped back into the return currency as a
	///       result of decreasing the invested amount before being processed.
	///
	/// NOTE: This state can be transitioned into
	/// `ActiveSwapIntoPoolCurrencyAndInvestmentOngoing` by applying the
	/// corresponding trigger to handle the return amount.
	ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
		swap: Swap<Balance, Currency>,
		done_amount: Balance,
		invest_amount: Balance,
	},
	/// The investment is split into three:
	///     * One is waiting to be processed as investment.
	///     * The second part is swapped back into the return currency as a
	///       result of decreasing the invested amount before being processed.
	///     * The remaining part was already swapped back into the return
	///       currency.
	///
	/// NOTE: This state should not be transitioned by applying the trigger for
	/// the done part but wait until the active swap is fulfilled.
	ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
		swap: Swap<Balance, Currency>,
		done_amount: Balance,
		invest_amount: Balance,
	},
	// TODO: Maybe remove
	/// The unprocessed investment was swapped back into return currency.
	///
	/// NOTE: This state can be killed by applying the corresponding trigger to
	/// handle the return amount.
	SwapIntoReturnDone { swap: Swap<Balance, Currency> },
	// TODO: Maybe remove
	/// The investment is split into two:
	///     * One part is waiting to be processed as an investment
	///     * The swapped back into the return currency as a result of
	///       decreasing the invested amount before being processed.
	///
	/// NOTE: This state can be transitioned into `InvestmentOngoing` by
	/// applying the corresponding trigger to handle the return amount.
	SwapIntoReturnDoneAndInvestmentOngoing {
		swap: Swap<Balance, Currency>,
		invest_amount: Balance,
	},
}

#[derive(Clone, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
// TODO: Complete
pub enum InvestTransition<
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord,
	Currency: Clone + Copy + PartialEq,
> {
	/// NOTE: Assumes `swap.currency_in` is pool currency
	IncreaseInvestOrder(Swap<Balance, Currency>),
	/// NOTE: Assumes `swap.currency_in` is return currency
	DecreaseInvestOrder(Swap<Balance, Currency>),
	/// NOTE: Assumes `swap.currency_in` is pool currency
	SwapIntoPool(Swap<Balance, Currency>),
	/// NOTE: Assumes `swap.currency_in` is return currency
	SwapIntoReturn(Swap<Balance, Currency>),
}
