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
use cfg_primitives::{Balance, BlockNumber, EpochId, PoolId, TrancheId, TrancheWeight};
use cfg_types::{CurrencyId, Rate, TrancheCurrency};
use sp_arithmetic::{FixedPointNumber, Perquintill};
use sp_runtime::{traits::One, BoundedVec};

use super::mock::{MaxTokenNameLength, MaxTokenSymbolLength};
use crate::{
	tests::mock::{MaxSizeMetadata, MockAccountId, Origin, Pools},
	EpochExecutionInfo, EpochExecutionTranche, EpochExecutionTranches, EpochState, PoolDetails,
	PoolParameters, PoolStatus, ReserveDetails, Tranche, TrancheInput, TrancheMetadata,
	TrancheSolution, TrancheType, Tranches,
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

// Typed types with usually a lot of generics
pub type TTranche = Tranche<Balance, Rate, TrancheWeight, TrancheCurrency>;
pub type TTranches = Tranches<Balance, Rate, TrancheWeight, TrancheCurrency, TrancheId, PoolId>;
pub type TEpochTranche = EpochExecutionTranche<Balance, Rate, TrancheWeight, TrancheCurrency>;
pub type TEpochTranches = EpochExecutionTranches<Balance, Rate, TrancheWeight, TrancheCurrency>;
pub type TPoolDetails = PoolDetails<
	CurrencyId,
	TrancheCurrency,
	EpochId,
	Balance,
	Rate,
	MaxSizeMetadata,
	TrancheWeight,
	TrancheId,
	PoolId,
>;
pub type TEpochExecutionInfo =
	EpochExecutionInfo<Balance, Rate, EpochId, TrancheWeight, BlockNumber, TrancheCurrency>;

/// Creates a default tranche and allows to run op
/// for that tranche afterwards.
///
/// Returns the operated on tranches.
pub fn tranches<F>(number: usize, mut op: F) -> TTranches
where
	F: FnMut(&mut TTranche),
{
	TTranches::new(
		POOL_ID,
		std::iter::repeat(TTranche::default())
			.take(number)
			.map(|mut tranche| {
				op(&mut tranche);
				tranche
			})
			.collect(),
	)
	.expect("Creating Tranches struct in testing must work. Qed.")
}

/// Creates epoch tranches in the number of the given tranches.
/// Allows to run op on the epoch tranche.
///
/// Returns operated on EpochTranches
pub fn epoch_tranches<F>(tranches: &TTranches, mut op: F) -> TEpochTranches
where
	F: FnMut(&TTranche, &mut TEpochTranche),
{
	TEpochTranches::new(
		std::iter::repeat(TEpochTranche::default())
			.take(tranches.num_tranches())
			.zip(tranches.residual_top_slice())
			.map(|(mut epoch_tranche, tranche)| {
				op(tranche, &mut epoch_tranche);
				epoch_tranche
			})
			.collect(),
	)
}

/// Creates a default PoolDetails struct in the form of
///
/// ```ignore
/// PoolDetails {
/// 			currency: CurrencyId::AUSD,
/// 			tranches,
/// 			status: PoolStatus::Open,
/// 			epoch: EpochState {
/// 				current: Zero::zero(),
/// 				last_closed: Zero::zero()
/// 				last_executed: Zero::zero(),
/// 			},
/// 			reserve: ReserveDetails {
/// 				max: Zero::zero(),
/// 				available: Zero::zero(),
/// 				total: Zero::zero(),
/// 			},
/// 			parameters: PoolParameters {
/// 				min_epoch_time: Zero::zero(),
/// 				max_nav_age: Zero::zero(),
/// 			},
/// 			metadata: None,
/// 		};
/// ```
///
/// Allows to run op on the generated PoolDetails
pub fn pool_details<F>(tranches: &TTranches, mut op: F) -> TPoolDetails
where
	F: FnMut(&mut TPoolDetails),
{
	let mut details = PoolDetails {
		currency: CurrencyId::AUSD,
		tranches: tranches.clone(),
		status: PoolStatus::Open,
		epoch: EpochState {
			current: 0,
			last_closed: 0,
			last_executed: 0,
		},
		reserve: ReserveDetails {
			max: 0,
			available: 0,
			total: 0,
		},
		parameters: PoolParameters {
			min_epoch_time: 0,
			max_nav_age: 0,
		},
		metadata: None,
	};
	op(&mut details);
	details
}

/// Creates a default EpochExecutionInfo struct in the form of
///
/// ```ignore
/// EpochExecutionInfo {
// 		epoch: 0,
// 		nav: 0,
// 		reserve: pool.reserve.total,
// 		max_reserve: pool.reserve.max,
// 		tranches: epoch_tranches.clone(),
// 		best_submission: None,
// 		challenge_period_end: None,
// 	};
/// ```
///
/// Allows to run op on the generated EpochExecutionInfo
pub fn epoch_exection_info<F>(
	epoch_tranches: &TEpochTranches,
	pool: &TPoolDetails,
	mut op: F,
) -> TEpochExecutionInfo
where
	F: FnMut(&mut TEpochExecutionInfo),
{
	let mut epoch = TEpochExecutionInfo {
		epoch: 0,
		nav: 0,
		reserve: pool.reserve.total,
		max_reserve: pool.reserve.max,
		tranches: epoch_tranches.clone(),
		best_submission: None,
		challenge_period_end: None,
	};

	op(&mut epoch);
	epoch
}

/// Generates a solution with the right len and of fulfillment of 100%
pub fn full_solution<R>(len: impl AsRef<[R]>) -> Vec<TrancheSolution> {
	std::iter::repeat(TrancheSolution {
		invest_fulfillment: Perquintill::one(),
		redeem_fulfillment: Perquintill::one(),
	})
	.take(len.as_ref().len())
	.collect()
}

/// Solution with ops
///
/// Generates a solution of given len and allows to run ops on it
pub fn solution<R, F>(len: impl AsRef<[R]>, mut op: F) -> Vec<TrancheSolution>
where
	F: FnMut(&mut TrancheSolution),
{
	full_solution(len)
		.into_iter()
		.map(|mut sol| {
			op(&mut sol);
			sol
		})
		.collect()
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
