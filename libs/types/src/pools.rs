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

use cfg_traits::{fee::FeeAmountProration, SaturatedProration};
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::{FixedPointNumber, FixedPointOperand};
use sp_runtime::{traits::Get, BoundedVec, RuntimeDebug};

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	pub token_name: BoundedVec<u8, MaxTokenNameLength>,
	pub token_symbol: BoundedVec<u8, MaxTokenSymbolLength>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct PoolMetadata<MetaSize>
where
	MetaSize: Get<u32>,
{
	pub metadata: BoundedVec<u8, MetaSize>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum PoolRegistrationStatus {
	Registered,
	Unregistered,
}

// TODO(william): Docs
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub struct PoolFee<AccountId, Balance, Rate> {
	/// Account that the fees are sent to
	pub destination: AccountId,

	/// Account that can update this fee
	pub editor: FeeEditor<AccountId>,

	/// Amount of fees that can be charged
	pub amount: FeeAmountType<Balance, Rate>,
}

// TODO(william): Docs

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub enum FeeEditor<AccountId> {
	Root,
	Account(AccountId),
}

impl<AccountId> FeeEditor<AccountId>
where
	AccountId: PartialEq,
{
	// TODO(william): Docs
	pub fn matches_account(&self, who: &AccountId) -> bool {
		match self {
			Self::Account(account) => account == who,
			_ => false,
		}
	}
}

// TODO(william): Docs

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub enum FeeAmountType<Balance, Rate> {
	/// A fixed fee is deducted automatically every epoch
	Fixed { amount: FeeAmount<Balance, Rate> },

	/// A fee can be charged up to a limit, paid every epoch
	ChargedUpTo { limit: FeeAmount<Balance, Rate> },
}

// TODO(william): Docs

#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub enum FeeAmount<Balance, Rate> {
	ShareOfPortfolioValuation(Rate),
	// TODO: AmountPerSecond(Balance) might be sufficient
	AmountPerYear(Balance),
	AmountPerMonth(Balance),
	AmountPerSecond(Balance),
}

// TODO(william): Docs
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub enum FeeBucket {
	/// Fees that are charged first, before any redemptions, investments,
	/// repayments or originations
	Top,
	// Future: AfterTranche(TrancheId)
}

impl<Balance, Rate, Time> FeeAmountProration<Balance, Rate, Time> for FeeAmount<Balance, Rate>
where
	Rate: SaturatedProration<Time = Time> + FixedPointNumber,
	Balance: From<Time> + From<u32> + SaturatedProration<Time = Time> + FixedPointOperand,
{
	fn saturated_prorated_amount(&self, portfolio_valuation: Balance, period: Time) -> Balance {
		match self {
			FeeAmount::ShareOfPortfolioValuation(_) => {
				let proration: Rate =
					<Self as FeeAmountProration<Balance, Rate, Time>>::saturated_prorated_rate(
						self,
						portfolio_valuation,
						period,
					);
				proration.saturating_mul_int(portfolio_valuation)
			}
			FeeAmount::AmountPerYear(amount) => Balance::saturated_proration(*amount, period),
			FeeAmount::AmountPerMonth(amount) => {
				Balance::saturated_proration(amount.saturating_mul(12u32.into()), period)
			}
			FeeAmount::AmountPerSecond(amount) => amount.saturating_mul(period.into()),
		}
	}

	fn saturated_prorated_rate(&self, portfolio_valuation: Balance, period: Time) -> Rate {
		match self {
			FeeAmount::ShareOfPortfolioValuation(rate) => Rate::saturated_proration(*rate, period),
			FeeAmount::AmountPerYear(_)
			| FeeAmount::AmountPerMonth(_)
			| FeeAmount::AmountPerSecond(_) => {
				let prorated_amount: Balance =
					<Self as FeeAmountProration<Balance, Rate, Time>>::saturated_prorated_amount(
						self,
						portfolio_valuation,
						period,
					);
				Rate::saturating_from_rational(prorated_amount, portfolio_valuation)
			}
		}
	}
}
