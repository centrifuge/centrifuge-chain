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
use sp_runtime::traits::Zero;
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
#[derive(Encode, Default, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct CollectedAmount<Balance: Default> {
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

// /// The collected investment for an account
// #[derive(Encode, Default, Decode, Clone, Eq, PartialEq, RuntimeDebug,
// TypeInfo)] pub struct CollectedInvestment<Balance: Default> {
// 	/// The amount of tranche tokens which was was collected
// 	pub amount_collected: Balance,

// 	/// The amount of payment currency which invested and converted into tranche
// 	/// tokens during processing based on the fulfillment price(s)
// 	pub amount_payment: Balance,
// }

// /// The collected redemption for an account
// #[derive(Encode, Default, Decode, Clone, Eq, PartialEq, RuntimeDebug,
// TypeInfo)] pub struct CollectedRedemption<Balance: Default> {
// 	/// The amount of payment currency which was was collected
// 	pub amount_collected: Balance,

// 	/// The tranche tokens which which was held as an investment and converted
// 	/// into payment currency during processing based on the fulfillment
// 	/// price(s)
// 	pub amount_payment: Balance,
// }

/// A representation of an investment identifier and the corresponding owner.
///
/// NOTE: Trimmed version of `InvestmentInfo` required for foreign investments.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ForeignInvestmentInfo<AccountId, InvestmentId> {
	pub owner: AccountId,
	pub id: InvestmentId,
}

/// A representation of an executed decreased investment or redemption.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ExecutedDecrease<Balance, Currency> {
	pub return_currency: Currency,
	pub amount_decreased: Balance,
	pub amount_remaining: Balance,
}

/// A representation of an executed collected investment.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ExecutedCollectInvest<Balance> {
	/// The amount that was actually collected
	pub amount_currency_payout: Balance,
	/// The amount of tranche tokens received for the investment made
	pub amount_tranche_tokens_payout: Balance,
	// TODO: Processed or unprocessed?
	/// The remaining, unprocessed investment amount which the investor
	/// still has locked to invest at a later epoch execution
	pub amount_remaining: Balance,
}

/// A representation of an executed collected redemption.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ExecutedCollectRedeem<Balance, Currency> {
	/// The return currency in which the payout takes place
	pub currency: Currency,
	/// The amount of `currency` being paid out to the investor
	pub amount_currency_payout: Balance,
	/// How many tranche tokens were actually redeemed
	pub amount_tranche_tokens_payout: Balance,
	// TODO: Processed or unprocessed?
	/// The remaining amount of tranche tokens the investor still has locked
	/// to redeem at a later epoch execution
	pub amount_remaining: Balance,
}
