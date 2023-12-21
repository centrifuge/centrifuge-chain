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
use sp_arithmetic::{
	traits::{CheckedAdd, CheckedSub, EnsureAdd, EnsureSub},
	FixedPointNumber, FixedPointOperand,
};
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

/// The representation of a pool fee, its editor and destination address
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub struct PoolFee<AccountId, FeeId, FeeType> {
	/// Account that the fees are sent to
	pub destination: AccountId,

	/// Account that can update this fee
	pub editor: PoolFeeEditor<AccountId>,

	/// Amount of fees that can be charged
	pub amount: FeeType,

	/// The identifier
	pub id: FeeId,
}

impl<AccountId, Balance, FeeId, Rate> From<PoolFee<AccountId, FeeId, PoolFeeType<Balance, Rate>>>
	for PoolFee<AccountId, FeeId, PendingPoolFeeType<Balance, Rate>>
where
	Balance: Default + Clone + CheckedSub + CheckedAdd + EnsureSub + EnsureAdd,
	Rate: Clone,
	FeeId: Clone,
{
	fn from(fee: PoolFee<AccountId, FeeId, PoolFeeType<Balance, Rate>>) -> Self {
		let amount = match fee.amount {
			PoolFeeType::Fixed { limit } => PendingPoolFeeType::Fixed {
				limit,
				pending: Balance::default(),
				disbursement: Balance::default(),
			},
			PoolFeeType::ChargedUpTo { limit } => PendingPoolFeeType::ChargedUpTo {
				limit,
				pending: Balance::default(),
				payable: Balance::default(),
				disbursement: Balance::default(),
			},
		};

		Self {
			amount,
			destination: fee.destination,
			editor: fee.editor,
			id: fee.id,
		}
	}
}

/// The editor enum of pool fees
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub enum PoolFeeEditor<AccountId> {
	Root,
	Account(AccountId),
}

impl<AccountId> PoolFeeEditor<AccountId>
where
	AccountId: PartialEq,
{
	/// Checks whether the given account matches the wrapped fee editor address
	pub fn matches_account(&self, who: &AccountId) -> bool {
		match self {
			Self::Account(account) => account == who,
			_ => false,
		}
	}
}

// TODO: Improve name, maybe `PoolFeeDetails`
/// The fee amount wrapper type
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub enum PoolFeeType<Balance, Rate> {
	/// A fixed fee is deducted automatically every epoch
	Fixed { limit: PoolFeeAmount<Balance, Rate> },

	/// A fee can be charged up to a limit, paid every epoch
	ChargedUpTo { limit: PoolFeeAmount<Balance, Rate> },
}

/// The pending fee amount wrapper type
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]

pub enum PendingPoolFeeType<Balance, Rate>
where
	Balance: Clone + CheckedSub + CheckedAdd + EnsureSub + EnsureAdd,
	Rate: Clone,
{
	/// A fixed fee is deducted automatically every epoch
	Fixed {
		limit: PoolFeeAmount<Balance, Rate>,
		pending: Balance,
		disbursement: Balance,
	},

	/// A fee can be charged up to a limit, paid every epoch
	ChargedUpTo {
		limit: PoolFeeAmount<Balance, Rate>,
		pending: Balance,
		payable: Balance,
		disbursement: Balance,
	},
}

impl<Balance, Rate> PendingPoolFeeType<Balance, Rate>
where
	Balance: Clone + CheckedSub + CheckedAdd + EnsureSub + EnsureAdd,
	Rate: Clone,
{
	pub fn checked_mutate_pending(&mut self, mut f: impl FnMut(&mut Balance)) {
		match *self {
			Self::Fixed {
				ref mut pending, ..
			}
			| Self::ChargedUpTo {
				ref mut pending, ..
			} => {
				f(pending);
			}
		}
	}

	pub fn checked_mutate_disbursement(&mut self, mut f: impl FnMut(&mut Balance)) {
		match *self {
			Self::Fixed {
				ref mut disbursement,
				..
			}
			| Self::ChargedUpTo {
				ref mut disbursement,
				..
			} => {
				f(disbursement);
			}
		}
	}

	pub fn checked_mutate_payable(&mut self, mut f: impl FnMut(&mut Balance)) {
		if let Self::ChargedUpTo {
			ref mut payable, ..
		} = *self
		{
			f(payable);
		}
	}

	pub fn limit(&self) -> &PoolFeeAmount<Balance, Rate> {
		match self {
			PendingPoolFeeType::Fixed { limit, .. }
			| PendingPoolFeeType::ChargedUpTo { limit, .. } => limit,
		}
	}

	pub fn pending(&self) -> &Balance {
		match self {
			PendingPoolFeeType::Fixed { pending, .. }
			| PendingPoolFeeType::ChargedUpTo { pending, .. } => pending,
		}
	}

	pub fn payable(&self) -> Option<&Balance> {
		match self {
			PendingPoolFeeType::ChargedUpTo { payable, .. } => Some(payable),
			_ => None,
		}
	}

	pub fn disbursement(&self) -> &Balance {
		match self {
			PendingPoolFeeType::Fixed { disbursement, .. }
			| PendingPoolFeeType::ChargedUpTo { disbursement, .. } => disbursement,
		}
	}
}

/// The fee amount
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub enum PoolFeeAmount<Balance, Rate> {
	ShareOfPortfolioValuation(Rate),
	// TODO: AmountPerSecond(Balance) might be sufficient
	AmountPerYear(Balance),
	AmountPerMonth(Balance),
	AmountPerSecond(Balance),
}

/// The priority segregation of pool fees
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone, Copy)]

pub enum PoolFeeBucket {
	/// Fees that are charged first, before any redemptions, investments,
	/// repayments or originations
	Top,
	// Future: AfterTranche(TrancheId)
}

impl PoolFeeBucket {
	pub fn iterator() -> impl Iterator<Item = PoolFeeBucket> {
		[Self::Top].iter().copied()
	}
}

impl<Balance, Rate, Time> FeeAmountProration<Balance, Rate, Time> for PoolFeeAmount<Balance, Rate>
where
	Rate: SaturatedProration<Time = Time> + FixedPointNumber,
	Balance: From<Time> + From<u32> + SaturatedProration<Time = Time> + FixedPointOperand,
{
	fn saturated_prorated_amount(&self, portfolio_valuation: Balance, period: Time) -> Balance {
		match self {
			PoolFeeAmount::ShareOfPortfolioValuation(_) => {
				let proration: Rate =
					<Self as FeeAmountProration<Balance, Rate, Time>>::saturated_prorated_rate(
						self,
						portfolio_valuation,
						period,
					);
				proration.saturating_mul_int(portfolio_valuation)
			}
			PoolFeeAmount::AmountPerYear(amount) => Balance::saturated_proration(*amount, period),
			PoolFeeAmount::AmountPerMonth(amount) => {
				Balance::saturated_proration(amount.saturating_mul(12u32.into()), period)
			}
			PoolFeeAmount::AmountPerSecond(amount) => amount.saturating_mul(period.into()),
		}
	}

	fn saturated_prorated_rate(&self, portfolio_valuation: Balance, period: Time) -> Rate {
		match self {
			PoolFeeAmount::ShareOfPortfolioValuation(rate) => {
				Rate::saturated_proration(*rate, period)
			}
			PoolFeeAmount::AmountPerYear(_)
			| PoolFeeAmount::AmountPerMonth(_)
			| PoolFeeAmount::AmountPerSecond(_) => {
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
