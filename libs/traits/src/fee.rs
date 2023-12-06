// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// TODO: Docs
// TODO: Enable paying without FeeBucket knowledge or expose buckets
pub trait PoolFees {
	type PoolId;
	type FeeBucket;
	type Balance;
	type Time;
	type Fee;
	type FeeId;
	type Error;
	type Rate;

	/// Withdraw any due fees. The waterfall of fee payment follows the order of
	/// the corresponding [FeeBucket].
	///
	/// Assumes `prepare_disbursements` to have been executed beforehand.
	fn pay_disbursements(pool_id: Self::PoolId, bucket: Self::FeeBucket)
		-> Result<(), Self::Error>;

	/// Determine the amount of any due fees. The waterfall of fee payment
	/// follows the order of the corresponding [FeeBucket] as long as the
	/// reserve is not empty.
	///
	/// Returns the updated reserve amount.
	fn prepare_disbursements(
		pool_id: Self::PoolId,
		bucket: Self::FeeBucket,
		portfolio_valuation: Self::Balance,
		reserve: Self::Balance,
		epoch_duration: Self::Time,
	) -> Self::Balance;

	/// Charge a fee for the given pair of pool id and fee bucket.
	///
	/// NOTE: Assumes call permissions are separately checked beforehand.
	fn charge_fee(fee_id: Self::FeeId, amount: Self::Balance) -> Result<(), Self::Error>;

	/// Cancel a previously charged fee for the given pair of pool id and fee
	/// bucket.
	///
	/// NOTE: Assumes call permissions are separately checked beforehand.
	fn uncharge_fee(fee_id: Self::FeeId, amount: Self::Balance) -> Result<(), Self::Error>;

	/// Add a new fee to the pool and bucket.
	///
	/// NOTE: Assumes call permissions are separately checked beforehand.
	fn add_fee(
		pool_id: Self::PoolId,
		bucket: Self::FeeBucket,
		fee: Self::Fee,
	) -> Result<(), Self::Error>;

	/// Entirely remove a stored fee from the given pair of pool id and fee
	/// bucket.
	///
	/// NOTE: Assumes call permissions are separately checked beforehand.
	fn remove_fee(fee_id: Self::FeeId) -> Result<(), Self::Error>;
}

/// Trait to prorate a fee amount to a rate or amount
pub trait FeeAmountProration<Balance, Rate, Time> {
	/// Returns the prorated amount based on the NAV passed time period.
	fn saturated_prorated_amount(&self, portfolio_valuation: Balance, period: Time) -> Balance;

	/// Returns the proratio based on the NAV and passed time period.
	fn saturated_prorated_rate(&self, portfolio_valuation: Balance, period: Time) -> Rate;
}
