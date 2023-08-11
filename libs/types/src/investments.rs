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

use cfg_traits::InvestmentProperties;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
use sp_std::cmp::PartialEq;

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

pub struct ExecutedDecrease<Balance> {
	pub amount_decreased: Balance,
	pub amount_remaining: Balance,
}

/// A representation of an executed collected investment or redemption.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]

pub struct ExecutedCollect<Balance, Currency> {
	// TODO: Check if necessary or can be used better
	pub currency: Option<Currency>,
	pub amount_currency_payout: Balance,
	pub amount_tranche_tokens_payout: Balance,
	pub amount_remaining: Balance,
}
