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

use cfg_traits::{
	fee::{FeeAmountProration, PoolFeeBucket},
	Seconds,
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::FixedPointOperand;
use sp_runtime::{traits::Get, BoundedVec, RuntimeDebug};
use sp_std::vec::Vec;

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

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct PoolNav<Balance> {
	pub nav_aum: Balance,
	pub nav_fees: Balance,
	pub reserve: Balance,
	pub total: Balance,
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
			PoolFeeType::ChargedUpTo { .. } => PayableFeeAmount::UpTo(Balance::default()),
			PoolFeeType::Fixed { .. } => PayableFeeAmount::AllPending,
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
	/// Can only be changed by Root (e.g. Treasury fee)
	Root,
	/// Can only be changed by the encapsulated account
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

/// The static fee amount wrapper type
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub enum PoolFeeType<Balance, Rate> {
	/// A fixed fee is deducted automatically every epoch
	Fixed { limit: PoolFeeAmount<Balance, Rate> },

	/// A fee can be charged up to a limit, paid every epoch
	ChargedUpTo { limit: PoolFeeAmount<Balance, Rate> },
}

/// The pending fee amount wrapper type. The `pending`, `disbursement` and
/// `payable` fields are updated on each NAV update, the `fee_type` is static.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub struct PoolFeeAmounts<Balance, Rate> {
	/// The static fee type
	pub fee_type: PoolFeeType<Balance, Rate>,
	/// The dynamic pending amount which represents outstanding fee amounts
	/// which could not be paid. This can happen if
	///  * Either the reserve is insufficient; or
	///  * In case of a charged fee: If more was charged than can be paid.
	pub pending: Balance,
	/// The amount which will be paid during epoch closing. It is always ensured
	/// that the reserve is sufficient for the sum of all fees' disbursement
	/// amounts.
	pub disbursement: Balance,
	/// The maximum payable fee amount which is only used for charged fees.
	/// Necessary to determine how much can be paid if the nothing or an excess
	/// was charged.
	pub payable: PayableFeeAmount<Balance>,
}

/// The payable fee amount representation which is either
///  * `AllPending` if the fee is not chargeable; or
///  * `UpTo(amount)` if the fee is chargeable. The `amount` reflects the max
///    payable amount at the time of the calculation. The disbursement of such
///    fee is the minimum of pending and payable.
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub enum PayableFeeAmount<Balance> {
	AllPending,
	UpTo(Balance),
}

impl<Balance, Rate> PoolFeeAmounts<Balance, Rate> {
	pub fn limit(&self) -> &PoolFeeAmount<Balance, Rate> {
		match &self.fee_type {
			PoolFeeType::Fixed { limit } | PoolFeeType::ChargedUpTo { limit } => limit,
		}
	}
}

/// The static fee amount
#[derive(Debug, Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, Clone)]
pub enum PoolFeeAmount<Balance, Rate> {
	/// The relative amount dependent on the AssetsUnderManagement valuation
	ShareOfPortfolioValuation(Rate),
	/// The absolute amount per second
	AmountPerSecond(Balance),
}

impl<Balance, Rate> FeeAmountProration<Balance, Rate, Seconds> for PoolFeeAmount<Balance, Rate>
where
	Rate: FixedPointNumberExtension,
	Balance: From<Seconds> + FixedPointOperand + sp_std::ops::Div<Output = Balance>,
{
	fn saturated_prorated_amount(&self, portfolio_valuation: Balance, period: Seconds) -> Balance {
		match self {
			PoolFeeAmount::ShareOfPortfolioValuation(_) => {
				let proration: Rate =
					<Self as FeeAmountProration<Balance, Rate, Seconds>>::saturated_prorated_rate(
						self,
						portfolio_valuation,
						period,
					);
				proration.saturating_mul_int(portfolio_valuation)
			}
			PoolFeeAmount::AmountPerSecond(amount) => amount.saturating_mul(period.into()),
		}
	}

	fn saturated_prorated_rate(&self, portfolio_valuation: Balance, period: Seconds) -> Rate {
		match self {
			PoolFeeAmount::ShareOfPortfolioValuation(rate) => {
				saturated_rate_proration(*rate, period)
			}
			PoolFeeAmount::AmountPerSecond(_) => {
				let prorated_amount: Balance = <Self as FeeAmountProration<
					Balance,
					Rate,
					Seconds,
				>>::saturated_prorated_amount(
					self, portfolio_valuation, period
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

/// Represents all active fees of a pool fee bucket
#[derive(Decode, Encode, TypeInfo)]
pub struct PoolFeesOfBucket<FeeId, AccountId, Balance, Rate> {
	/// The corresponding pool fee bucket
	pub bucket: PoolFeeBucket,
	/// The list of active fees for the bucket
	pub fees: Vec<PoolFee<AccountId, FeeId, PoolFeeAmounts<Balance, Rate>>>,
}

/// Represent all active fees of a pool divided by buckets
pub type PoolFeesList<FeeId, AccountId, Balance, Rate> =
	Vec<PoolFeesOfBucket<FeeId, AccountId, Balance, Rate>>;

#[cfg(test)]
mod tests {
	use super::*;

	mod saturated_proration {
		use cfg_primitives::{CFG, DAYS, SECONDS_PER_YEAR};
		use sp_arithmetic::{
			traits::{One, Zero},
			FixedPointNumber,
		};

		use super::*;
		use crate::fixed_point::{Quantity, Rate};

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

		#[test]
		fn precision_quantity_vs_rate() {
			let period = (DAYS / 4) as Seconds;
			let nav_multiplier = 1_000_000;
			let nav = nav_multiplier * CFG;

			let q_proration = saturated_rate_proration::<Quantity>(
				Quantity::checked_from_rational(1, 100).unwrap(),
				period,
			);
			let r_proration = saturated_rate_proration::<Rate>(
				Rate::checked_from_rational(1, 100).unwrap(),
				period,
			);

			let q_amount = q_proration.saturating_mul_int(nav);
			let r_amount = r_proration.saturating_mul_int(nav);
			let r_amount_rounded_up = (r_amount / (nav_multiplier) + 1) * (nav_multiplier);

			assert_eq!(q_amount, r_amount_rounded_up);
		}
	}
}
