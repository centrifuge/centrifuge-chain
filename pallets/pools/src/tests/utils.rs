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

//! Helpers around tests
use cfg_primitives::PoolId;
use cfg_types::{CurrencyId, Rate};
use sp_arithmetic::{FixedPointNumber, Perquintill};
use sp_runtime::{traits::One, BoundedVec};

use super::mock::{MaxTokenNameLength, MaxTokenSymbolLength};
use crate::{
	tests::mock::{MockAccountId, Origin, Pools},
	TrancheInput, TrancheMetadata, TrancheType,
};

/// The default PoolId used in tests
pub const POOL_ID: PoolId = 0;
/// The default AdminAccount for pools
pub const ADMIN: MockAccountId = 1000;
/// AUSD decimals
pub const AUSD_DECIMALS: u128 = 1_000_000_000_000;

lazy_static::lazy_static! {
	/// A rate that is created from SECONDS_PER_YEAR.
	pub static ref SEONDS_PER_YEAR_AS_RATE: Rate = Rate::saturating_from_integer(cfg_primitives::SECONDS_PER_YEAR);
}

/// A function that takes an input as percent - e.g. 1582 -> 15,87%, 100 -> 1% -
/// and creates a rate per second in the form of 1.xxx.
///
/// This value can be used to calculate thinks like 100 * 1.15 = 115.
pub fn rate_per_second(four_decimals_percentage: u64) -> Rate {
	Rate::saturating_from_rational(four_decimals_percentage, 1000) / *SEONDS_PER_YEAR_AS_RATE
		+ One::one()
}

/// Creates a pool with the following properties:
///
/// * Admin: ADMIN -> This one will take the deposit
/// * PoolId: POOL_ID
/// * 5 Tranches
///     * 0: Junior Tranche
///     * 1: 10% APR, 5% Risk buffer
///     * 2: 7% APR, 5% Risk buffer
///     * 3: 5% APR, 10% Risk buffer
///     * 4: 3% APR, 25% Risk buffer
/// * Currency: CurrencyId::AUSD,
/// * MaxReserve: 1000 * AUSD_DECIMALS
/// * Metadata: None,
pub fn create_default_test_pool() {
	Pools::create(
		Origin::root(),
		ADMIN,
		POOL_ID,
		create_tranche_input(
			vec![None, Some(10), Some(7), Some(5), Some(3)],
			vec![None, Some(5), Some(5), Some(10), Some(25)],
			None,
		),
		CurrencyId::AUSD,
		1000 * AUSD_DECIMALS,
		None,
	)
	.expect("Creating a pool in testing must work. Qed.");
}

/// Creates a TrancheInput vector given the input.
/// The given input data MUST be sorted from residual-to-non-residual tranches.
///
/// DOES NOT check whether the length of the vectors match. It will simply zip starting with
/// rates.
pub fn create_tranche_input(
	rates: Vec<Option<u64>>,
	risk_buffs: Vec<Option<u64>>,
	seniorities: Option<Vec<Option<u32>>>,
) -> Vec<TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>> {
	let interest_rates = rates
		.into_iter()
		.map(|rate| {
			if let Some(rate) = rate {
				Some(rate_per_second(rate))
			} else {
				None
			}
		})
		.collect::<Vec<Option<_>>>();

	let risk_buffs = risk_buffs
		.into_iter()
		.map(|buff| {
			if let Some(buff) = buff {
				Some(Perquintill::from_percent(buff))
			} else {
				None
			}
		})
		.collect::<Vec<Option<_>>>();

	let seniority = if let Some(seniorites) = seniorities {
		seniorites
	} else {
		risk_buffs.iter().map(|_| None).collect()
	};

	interest_rates
		.into_iter()
		.zip(risk_buffs)
		.zip(seniority)
		.map(|((rate, buff), seniority)| {
			if let (Some(interest_rate_per_sec), Some(min_risk_buffer)) = (rate, buff) {
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec,
						min_risk_buffer,
					},
					seniority,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					},
				}
			} else {
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					},
				}
			}
		})
		.collect()
}
