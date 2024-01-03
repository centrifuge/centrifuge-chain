// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use sp_runtime::DispatchError;

/// Trait to add fees to a pool
pub trait AddPoolFees {
	type PoolId;
	type FeeBucket;
	type FeeInfo;

	/// Add a new fee to the pool and bucket.
	///
	/// NOTE: Assumes call permissions are separately checked beforehand.
	fn add_fee(
		pool_id: Self::PoolId,
		bucket: Self::FeeBucket,
		fee: Self::FeeInfo,
	) -> Result<(), DispatchError>;
}

/// Trait to prorate a fee amount to a rate or amount
pub trait FeeAmountProration<Balance, Rate, Time> {
	/// Returns the prorated amount based on the NAV passed time period.
	fn saturated_prorated_amount(&self, portfolio_valuation: Balance, period: Time) -> Balance;

	/// Returns the proratio rate based on the NAV and passed time period.
	fn saturated_prorated_rate(&self, portfolio_valuation: Balance, period: Time) -> Rate;
}
