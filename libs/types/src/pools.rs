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

use cfg_traits::{fee::FeeAmountProration, Seconds};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::FixedPointOperand;
use sp_runtime::{traits::Get, BoundedVec, RuntimeDebug};
use strum::{EnumCount, EnumIter};

use crate::fixed_point::FixedPointNumberExtension;

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

/// The dynamic representation of a pool fee, its editor and destination
/// address.
///
/// The pending and disbursement fee amounts are frequently updated based on the
/// positive NAV.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub struct PoolFee<AccountId, FeeId, FeeAmounts> {
	/// Account that the fees are sent to
	pub destination: AccountId,

	/// Account that can update this fee
	pub editor: PoolFeeEditor<AccountId>,

	/// Amount of fees that can be charged
	pub amounts: FeeAmounts,

	/// The identifier
	pub id: FeeId,
}

/// The static representation of a pool fee used for creation.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub struct PoolFeeInfo<AccountId, Balance, Rate> {
	/// Account that the fees are sent to
	pub destination: AccountId,

	/// Account that can update this fee
	pub editor: PoolFeeEditor<AccountId>,

	/// Amount of fees that can be charged
	pub fee_type: PoolFeeType<Balance, Rate>,
}

impl<AccountId, Balance, FeeId, Rate> PoolFee<AccountId, FeeId, PoolFeeAmounts<Balance, Rate>>
where
	Balance: Default,
{
	pub fn from_info(fee: PoolFeeInfo<AccountId, Balance, Rate>, fee_id: FeeId) -> Self {
		let payable = match fee.fee_type {
			PoolFeeType::ChargedUpTo { .. } => Some(Balance::default()),
			PoolFeeType::Fixed { .. } => None,
		};
		let amount = PoolFeeAmounts {
			payable,
			fee_type: fee.fee_type,
			pending: Balance::default(),
			disbursement: Balance::default(),
		};

		Self {
			amounts: amount,
			destination: fee.destination,
			editor: fee.editor,
			id: fee_id,
		}
	}
}

/// The editor enum of pool fees
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub enum PoolFeeEditor<AccountId> {
	Root,
	Account(AccountId),
}

impl<AccountId> From<PoolFeeEditor<AccountId>> for Option<AccountId> {
	fn from(editor: PoolFeeEditor<AccountId>) -> Option<AccountId> {
		match editor {
			PoolFeeEditor::Account(acc) => Some(acc),
			_ => None,
		}
	}
}

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
pub struct PoolFeeAmounts<Balance, Rate> {
	pub fee_type: PoolFeeType<Balance, Rate>,
	pub pending: Balance,
	pub disbursement: Balance,
	pub payable: Option<Balance>,
}

impl<Balance, Rate> PoolFeeAmounts<Balance, Rate> {
	pub fn limit(&self) -> &PoolFeeAmount<Balance, Rate> {
		match &self.fee_type {
			PoolFeeType::Fixed { limit } | PoolFeeType::ChargedUpTo { limit } => limit,
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
///
/// NOTE: Whenever a new variant is added, must bump
/// [cfg_primitives::MAX_FEES_PER_POOL].
#[derive(
	Debug, Encode, Decode, EnumIter, EnumCount, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone, Copy,
)]
pub enum PoolFeeBucket {
	/// Fees that are charged first, before any redemptions, investments,
	/// repayments or originations
	Top,
	// Future: AfterTranche(TrancheId)
}

impl<Balance, Rate, Time> FeeAmountProration<Balance, Rate, Time> for PoolFeeAmount<Balance, Rate>
where
	Rate: FixedPointNumberExtension,
	Balance: From<Seconds> + FixedPointOperand + sp_std::ops::Div<Output = Balance>,
	Time: Into<Seconds>,
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
			PoolFeeAmount::AmountPerYear(amount) => {
				saturated_balance_proration(*amount, period.into())
			}
			PoolFeeAmount::AmountPerMonth(amount) => {
				saturated_balance_proration(amount.saturating_mul(12u64.into()), period.into())
			}
			PoolFeeAmount::AmountPerSecond(amount) => amount.saturating_mul(period.into().into()),
		}
	}

	fn saturated_prorated_rate(&self, portfolio_valuation: Balance, period: Time) -> Rate {
		match self {
			PoolFeeAmount::ShareOfPortfolioValuation(rate) => {
				saturated_rate_proration(*rate, period.into())
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

/// Converts an annual balance amount into its proratio based on the given
/// period duration.
pub fn saturated_balance_proration<
	Balance: From<Seconds> + FixedPointOperand + sp_std::ops::Div<Output = Balance>,
>(
	annual_amount: Balance,
	period: Seconds,
) -> Balance {
	let amount = annual_amount.saturating_mul(period.into());
	amount.div(cfg_primitives::SECONDS_PER_YEAR.into())
}

/// Converts an annual rate into its proratio based on the given
/// period duration.
pub fn saturated_rate_proration<Rate: FixedPointNumberExtension>(
	annual_rate: Rate,
	period: Seconds,
) -> Rate {
	let rate = annual_rate.saturating_mul(Rate::saturating_from_integer::<u64>(period));

	rate.saturating_div_ceil(&Rate::saturating_from_integer::<u64>(
		cfg_primitives::SECONDS_PER_YEAR,
	))
}

#[cfg(test)]
mod tests {
	use strum::IntoEnumIterator;

	use super::*;

	#[test]
	fn max_fees_per_pool() {
		assert!(
			cfg_primitives::MAX_POOL_FEES_PER_BUCKET
				<= (cfg_primitives::MAX_FEES_PER_POOL * PoolFeeBucket::iter().count() as u32),
			"Need to bump MAX_FEES_PER_POOL after adding variant(s) to PoolFeeBuckets"
		);
	}

	mod saturated_proration {
		use cfg_primitives::SECONDS_PER_YEAR;
		use sp_arithmetic::traits::{One, Zero};

		use super::*;
		use crate::fixed_point::Rate;

		type Balance = u128;

		#[test]
		fn balance_zero() {
			assert_eq!(
				saturated_balance_proration::<Balance>(SECONDS_PER_YEAR.into(), 0),
				0
			);
			assert_eq!(
				saturated_balance_proration::<Balance>(0u128, SECONDS_PER_YEAR),
				0
			);
			assert_eq!(
				saturated_balance_proration::<Balance>((SECONDS_PER_YEAR - 1).into(), 1),
				0
			);
			assert_eq!(
				saturated_balance_proration::<Balance>(1u128, SECONDS_PER_YEAR - 1),
				0
			);
		}

		#[test]
		fn balance_one() {
			assert_eq!(
				saturated_balance_proration::<Balance>(SECONDS_PER_YEAR.into(), 1),
				1u128
			);
			assert_eq!(
				saturated_balance_proration::<Balance>(1u128, SECONDS_PER_YEAR),
				1u128
			);
		}
		#[test]
		fn balance_overflow() {
			assert_eq!(
				saturated_balance_proration::<Balance>(u128::MAX, u64::MAX),
				u128::MAX / u128::from(SECONDS_PER_YEAR)
			);
		}

		#[test]
		fn rate_zero() {
			assert_eq!(
				saturated_rate_proration::<Rate>(Rate::from_integer(SECONDS_PER_YEAR.into()), 0),
				Rate::zero()
			);
			assert_eq!(
				saturated_rate_proration::<Rate>(Rate::zero(), SECONDS_PER_YEAR),
				Rate::zero()
			);
			assert!(
				saturated_rate_proration::<Rate>(
					Rate::from_integer((SECONDS_PER_YEAR - 1).into()),
					1
				) > Rate::zero()
			);
			assert!(
				saturated_rate_proration::<Rate>(Rate::one(), SECONDS_PER_YEAR - 1) > Rate::zero()
			);
		}

		#[test]
		fn rate_one() {
			assert_eq!(
				saturated_rate_proration::<Rate>(Rate::from_integer(SECONDS_PER_YEAR.into()), 1),
				Rate::one()
			);
			assert_eq!(
				saturated_rate_proration::<Rate>(Rate::one(), SECONDS_PER_YEAR),
				Rate::one()
			);
		}
		#[test]
		fn rate_overflow() {
			let left_bound = Rate::from_integer(10790);
			let right_bound = Rate::from_integer(10791);

			let rate = saturated_rate_proration::<Rate>(
				Rate::from_integer(u128::from(u128::MAX / 10u128.pow(27))),
				1,
			);
			assert!(left_bound < rate);
			assert!(rate < right_bound);

			assert!(saturated_rate_proration::<Rate>(Rate::one(), u64::MAX) > left_bound);
			assert!(saturated_rate_proration::<Rate>(Rate::one(), u64::MAX) < right_bound);

			assert!(saturated_rate_proration::<Rate>(Rate::from_integer(2), u64::MAX) > left_bound);
			assert!(
				saturated_rate_proration::<Rate>(Rate::from_integer(2), u64::MAX) < right_bound
			);
		}
	}
}
