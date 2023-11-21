// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::OrderId;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{dispatch::fmt::Debug, RuntimeDebug};
use scale_info::TypeInfo;
use sp_arithmetic::traits::{EnsureAdd, EnsureSub};
use sp_runtime::{traits::Zero, DispatchError, DispatchResult};
use sp_std::cmp::PartialEq;

use crate::orders::Order;

/// A representation of a investment identifier that can be converted to an
/// account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct InvestmentAccount<InvestmentId> {
	pub investment_id: InvestmentId,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct InvestmentInfo<AccountId, Currency, InvestmentId> {
	pub owner: AccountId,
	pub id: InvestmentId,
	pub payment_currency: Currency,
}

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct InvestCollection<Balance> {
	/// This is the payout in the denomination currency
	/// of an investment
	/// * If investment: In payment currency
	/// * If payout: In denomination currency
	pub payout_investment_invest: Balance,

	/// This is the remaining investment in the payment currency
	/// of an investment
	/// * If investment: In payment currency
	/// * If payout: In denomination currency
	pub remaining_investment_invest: Balance,
}

impl<Balance: Zero> Default for InvestCollection<Balance> {
	fn default() -> Self {
		InvestCollection {
			payout_investment_invest: Zero::zero(),
			remaining_investment_invest: Zero::zero(),
		}
	}
}

impl<Balance: Zero + Copy> InvestCollection<Balance> {
	/// Create a `InvestCollection` directly from an active invest order of
	/// a user.
	/// The field `remaining_investment_invest` is set to the
	/// amount of the active invest order of the user and will
	/// be subtracted from upon given fulfillment's
	pub fn from_order(order: &Order<Balance, OrderId>) -> Self {
		InvestCollection {
			payout_investment_invest: Zero::zero(),
			remaining_investment_invest: order.amount(),
		}
	}
}

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RedeemCollection<Balance> {
	/// This is the payout in the payment currency
	/// of an investment
	/// * If redemption: In denomination currency
	/// * If payout: In payment currency
	pub payout_investment_redeem: Balance,

	/// This is the remaining redemption in the denomination currency
	/// of an investment
	/// * If redemption: In denomination currency
	/// * If payout: In payment currency
	pub remaining_investment_redeem: Balance,
}

impl<Balance: Zero> Default for RedeemCollection<Balance> {
	fn default() -> Self {
		RedeemCollection {
			payout_investment_redeem: Zero::zero(),
			remaining_investment_redeem: Zero::zero(),
		}
	}
}

impl<Balance: Zero + Copy> RedeemCollection<Balance> {
	/// Create a `RedeemCollection` directly from an active redeem order of
	/// a user.
	/// The field `remaining_investment_redeem` is set to the
	/// amount of the active redeem order of the user and will
	/// be subtracted from upon given fulfillment's
	pub fn from_order(order: &Order<Balance, OrderId>) -> Self {
		RedeemCollection {
			payout_investment_redeem: Zero::zero(),
			remaining_investment_redeem: order.amount(),
		}
	}
}

/// The collected investment/redemption amount for an account
#[derive(Encode, Default, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct CollectedAmount<Balance> {
	/// The amount which was collected
	/// * If investment: Tranche tokens
	/// * If redemption: Payment currency
	pub amount_collected: Balance,

	/// The amount which was converted during processing based on the
	/// fulfillment price(s)
	/// * If investment: Payment currency
	/// * If redemption: Tranche tokens
	pub amount_payment: Balance,
}

/// A representation of an investment identifier and the corresponding owner.
///
/// NOTE: Trimmed version of `InvestmentInfo` required for foreign investments.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ForeignInvestmentInfo<AccountId, InvestmentId, TokenSwapReason> {
	pub owner: AccountId,
	pub id: InvestmentId,
	pub last_swap_reason: Option<TokenSwapReason>,
}

/// A simple representation of a currency swap.
#[derive(
	Clone,
	Default,
	Copy,
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
pub struct Swap<
	Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug,
	Currency: Clone + PartialEq,
> {
	/// The incoming currency, i.e. the desired one.
	pub currency_in: Currency,
	/// The outgoing currency, i.e. the one which should be replaced.
	pub currency_out: Currency,
	/// The amount of incoming currency which shall be bought.
	pub amount: Balance,
}

impl<Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord + Debug, Currency: Clone + PartialEq>
	Swap<Balance, Currency>
{
	/// Ensures that the ingoing and outgoing currencies of two swaps...
	/// * Either match fully (in1 = in2, out1 = out2) if the swap direction is
	///   the same for both swaps, i.e. (into pool, into pool) or (into foreign,
	///   into foreign)
	/// * Or the ingoing and outgoing currencies match (in1 = out2, out1 = in2)
	///   if the swap direction is opposite, i.e. (into pool, into foreign) or
	///   (into foreign, into pool)
	pub fn ensure_currencies_match(
		&self,
		other: &Self,
		is_same_swap_direction: bool,
	) -> DispatchResult {
		if is_same_swap_direction
			&& (self.currency_in != other.currency_in || self.currency_out != other.currency_out)
		{
			Err(DispatchError::Other(
				"Swap currency mismatch for same swap direction",
			))
		} else if !is_same_swap_direction
			&& (self.currency_in != other.currency_out || self.currency_out != other.currency_in)
		{
			Err(DispatchError::Other(
				"Swap currency mismatch for opposite swap direction",
			))
		} else {
			Ok(())
		}
	}
}

/// A representation of an executed investment decrement.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ExecutedForeignDecreaseInvest<Balance, Currency> {
	/// The currency in which `DecreaseInvestOrder` was realised
	pub foreign_currency: Currency,
	/// The amount of `currency` that was actually executed in the original
	/// `DecreaseInvestOrder` message, i.e., the amount by which the
	/// investment order was actually decreased by.
	pub amount_decreased: Balance,
	/// The unprocessed plus processed but not yet collected investment amount
	/// denominated in `foreign` payment currency
	pub amount_remaining: Balance,
}

/// A representation of an executed collected foreign investment or redemption.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ExecutedForeignCollect<Balance, Currency> {
	/// The foreign currency in which ...
	/// * If investment: the payment took place
	/// * If redemption: the payout takes place
	pub currency: Currency,

	/// The amount of `currency`...
	/// * If investment: that was collected
	/// * If redemption: paid out to the investor
	pub amount_currency_payout: Balance,

	/// The amount of tranche tokens...
	/// * If investment: received for the investment made
	/// * If redemption: which were actually redeemed
	pub amount_tranche_tokens_payout: Balance,

	/// The unprocessed ...
	/// * If investment: investment amount of `currency` (denominated in foreign
	///   currency)
	/// * If redemption: redemption amount of tranche tokens (denominated in
	///   pool currency)
	pub amount_remaining: Balance,
}

/// A representation of an investment portfolio consisting of free, pending and
/// claimable pool currency as well as tranche tokens.
#[derive(Encode, Decode, Default, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]

pub struct InvestmentPortfolio<Balance, CurrencyId> {
	/// The identifier of the pool currency
	pub pool_currency_id: CurrencyId,
	/// The unprocessed invest order amount in pool currency
	pub pending_invest_currency: Balance,
	/// The amount of tranche tokens which can be collected for an invest order
	pub claimable_tranche_tokens: Balance,
	/// The amount of tranche tokens which can be transferred
	pub free_tranche_tokens: Balance,
	/// The amount of tranche tokens which can not be used at all and could get
	/// slashed
	pub reserved_tranche_tokens: Balance,
	/// The unprocessed redeem order amount in tranche tokens
	pub pending_redeem_tranche_tokens: Balance,
	/// The amount of pool currency which can be collected for a redeem order
	pub claimable_currency: Balance,
}

impl<Balance: Default, CurrencyId> InvestmentPortfolio<Balance, CurrencyId> {
	pub fn new(pool_currency_id: CurrencyId) -> Self {
		Self {
			pool_currency_id,
			pending_invest_currency: Balance::default(),
			claimable_tranche_tokens: Balance::default(),
			free_tranche_tokens: Balance::default(),
			reserved_tranche_tokens: Balance::default(),
			pending_redeem_tranche_tokens: Balance::default(),
			claimable_currency: Balance::default(),
		}
	}

	pub fn with_pending_invest_currency(mut self, amount: Balance) -> Self {
		self.pending_invest_currency = amount;
		self
	}

	pub fn with_free_tranche_tokens(mut self, amount: Balance) -> Self {
		self.free_tranche_tokens = amount;
		self
	}

	pub fn with_reserved_tranche_tokens(mut self, amount: Balance) -> Self {
		self.reserved_tranche_tokens = amount;
		self
	}

	pub fn with_claimable_tranche_tokens(mut self, amount: Balance) -> Self {
		self.claimable_tranche_tokens = amount;
		self
	}

	pub fn with_pending_redeem_tranche_tokens(mut self, amount: Balance) -> Self {
		self.pending_redeem_tranche_tokens = amount;
		self
	}

	pub fn with_claimable_currency(mut self, amount: Balance) -> Self {
		self.claimable_currency = amount;
		self
	}
}
