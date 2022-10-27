// Copyright 2022 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use codec::{Decode, Encode};
use frame_support::{pallet_prelude::Get, BoundedVec, RuntimeDebug};
use scale_info::TypeInfo;
use sp_runtime::Perquintill;

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum TrancheType<Rate> {
	Residual,
	NonResidual {
		interest_rate_per_sec: Rate,
		min_risk_buffer: Perquintill,
	},
}

impl<Rate> TrancheType<Rate>
where
	Rate: PartialOrd + PartialEq,
{
	/// Compares tranches with the following schema:
	///
	/// * (Residual, Residual) => false
	/// * (Residual, NonResidual) => true,
	/// * (NonResidual, Residual) => false,
	/// * (NonResidual, NonResidual) =>
	///         interest rate of next tranche must be smaller
	///         equal to the interest rate of self.
	///
	pub fn valid_next_tranche(&self, next: &TrancheType<Rate>) -> bool {
		match (self, next) {
			(TrancheType::Residual, TrancheType::Residual) => false,
			(TrancheType::Residual, TrancheType::NonResidual { .. }) => true,
			(TrancheType::NonResidual { .. }, TrancheType::Residual) => false,
			(
				TrancheType::NonResidual {
					interest_rate_per_sec: ref interest_prev,
					..
				},
				TrancheType::NonResidual {
					interest_rate_per_sec: ref interest_next,
					..
				},
			) => interest_prev >= interest_next,
		}
	}
}

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo)]
pub struct TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>
	where
		MaxTokenNameLength: Get<u32>,
		MaxTokenSymbolLength: Get<u32>,
{
	pub token_name: BoundedVec<u8, MaxTokenNameLength>,
	pub token_symbol: BoundedVec<u8, MaxTokenSymbolLength>,
}

/// Type that indicates the seniority of a tranche
pub type Seniority = u32;

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo)]
pub struct TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>
	where
		MaxTokenNameLength: Get<u32>,
		MaxTokenSymbolLength: Get<u32>,
{
	pub tranche_type: TrancheType<Rate>,
	pub seniority: Option<Seniority>,
	pub metadata: TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct TrancheUpdate<Rate> {
	pub tranche_type: TrancheType<Rate>,
	pub seniority: Option<Seniority>,
}
