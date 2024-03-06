// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use frame_support::pallet_prelude::TypeInfo;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_runtime::DispatchResult;
use strum::{EnumCount, EnumIter};

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

/// Trait to add fees to a pool
pub trait PoolFees {
	type PoolId;
	type FeeInfo;

	/// Add a new fee to the pool and bucket.
	///
	/// NOTE: Assumes call permissions are separately checked beforehand.
	fn add_fee(pool_id: Self::PoolId, bucket: PoolFeeBucket, fee: Self::FeeInfo) -> DispatchResult;

	/// Returns the maximum number of pool fees per bucket required for accurate
	/// weights
	fn get_max_fees_per_bucket() -> u32;

	/// Returns the current amount of active fees for the given pool and bucket
	/// pair
	fn get_pool_fee_bucket_count(pool: Self::PoolId, bucket: PoolFeeBucket) -> u32;
}

/// Trait to prorate a fee amount to a rate or amount
pub trait FeeAmountProration<Balance, Rate, Time> {
	/// Returns the prorated amount based on the NAV passed time period.
	fn saturated_prorated_amount(&self, portfolio_valuation: Balance, period: Time) -> Balance;

	/// Returns the proratio rate based on the NAV and passed time period.
	fn saturated_prorated_rate(&self, portfolio_valuation: Balance, period: Time) -> Rate;
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
}
