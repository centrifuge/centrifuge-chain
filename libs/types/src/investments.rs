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
use cfg_traits::investments::InvestmentProperties;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
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

impl<AccountId, Currency, InvestmentId> InvestmentProperties<AccountId>
	for InvestmentInfo<AccountId, Currency, InvestmentId>
where
	AccountId: Clone,
	Currency: Clone,
	InvestmentId: Clone,
{
	type Currency = Currency;
	type Id = InvestmentId;

	fn owner(&self) -> AccountId {
		self.owner.clone()
	}

	fn id(&self) -> Self::Id {
		self.id.clone()
	}

	fn payment_currency(&self) -> Self::Currency {
		self.payment_currency.clone()
	}
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
pub struct CollectedAmount<Balance: Default + MaxEncodedLen> {
	/// The amount which was was collected
	/// * If investment: Tranche tokens
	/// * If redemption: Payment currency
	pub amount_collected: Balance,

	/// The amount which invested and converted during processing based on the
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
pub struct Swap<Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord, Currency: Clone + PartialEq> {
	/// The incoming currency, i.e. the desired one.
	pub currency_in: Currency,
	/// The outgoing currency, i.e. the one which should be replaced.
	pub currency_out: Currency,
	/// The amount of outgoing currency which shall be exchanged.
	pub amount: Balance,
}

impl<Balance: Clone + Copy + EnsureAdd + EnsureSub + Ord, Currency: Clone + PartialEq>
	Swap<Balance, Currency>
{
	/// Ensures that the ingoing and outgoing currencies of two swaps...
	/// * Either match fully (in1 = in2, out1 = out2) if the swap direction is
	///   the same for both swaps, i.e. (pool, pool) or (return, return)
	/// * Or the ingoing and outgoing currencies match (in1 = out2, out1 = in2)
	///   if the swap direction is opposite, i.e. (pool, return) or (return,
	///   pool)
	pub fn ensure_currencies_match(
		&self,
		other: &Self,
		is_same_swap_direction: bool,
	) -> DispatchResult {
		if is_same_swap_direction
			&& self.currency_in != other.currency_in
			&& self.currency_out != other.currency_out
		{
			Err(DispatchError::Other(
				"Swap currency mismatch for same swap direction",
			))
		} else if !is_same_swap_direction
			&& self.currency_in != other.currency_out
			&& self.currency_out != other.currency_in
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

pub struct ExecutedForeignDecrease<Balance, Currency> {
	/// The currency in which `DecreaseInvestOrder` was realised
	pub return_currency: Currency,
	/// The amount of `currency` that was actually executed in the original
	/// `DecreaseInvestOrder` message, i.e., the amount by which the
	/// investment order was actually decreased by.
	pub amount_decreased: Balance,
}

/// A representation of an executed collected investment.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ExecutedForeignCollectInvest<Balance> {
	/// The amount that was actually collected
	pub amount_currency_payout: Balance,
	/// The amount of tranche tokens received for the investment made
	pub amount_tranche_tokens_payout: Balance,
}

/// A representation of an executed collected redemption.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ExecutedForeignCollectRedeem<Balance, Currency> {
	/// The return currency in which the payout takes place
	pub currency: Currency,
	/// The amount of `currency` being paid out to the investor
	pub amount_currency_payout: Balance,
	/// How many tranche tokens were actually redeemed
	pub amount_tranche_tokens_payout: Balance,
}
