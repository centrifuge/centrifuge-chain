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

//! Utilities around creating a pool

use crate::chain::centrifuge::{Call, Loans, OrmlTokens, Permissions, Pools, Timestamp, PARA_ID};
use crate::pools::utils::loans::NftManager;
use crate::pools::utils::{
	accounts::Keyring,
	env::TestEnv,
	time::secs::*,
	tokens,
	tokens::{DECIMAL_BASE_12, YEAR_RATE},
};
use codec::Encode;
use common_traits::Permissions as PermissionsT;
use common_types::{CurrencyId, PoolRole};
use frame_support::{Blake2_128, StorageHasher};
use fudge::primitives::Chain;
use pallet_permissions::Call as PermissionsCall;
use pallet_pools::{Call as PoolsCall, TrancheIndex, TrancheInput, TrancheLoc, TrancheType};
use runtime_common::{AccountId, Balance, PoolId, Rate, TrancheId};
use sp_runtime::{traits::One, FixedPointNumber, Perquintill};

/// Creates the necessary calls for initialising a pool.
/// This includes:
/// * creating a pool
/// * whitelisting investors
/// * initialising the loans pallet for the given pool
///
/// Extrinsics are returned and must be submitted to the transaction pool
/// in order to be included into the next block.
///
/// * Pool id as given
/// * Admin as provided (also owner of pool, and owner of nft-classes for collateral and loans)
/// * 5 Tranches
///     * 0: Junior Tranche
///     * 1: 10% APR, 5% Risk buffer
///     * 2: 7% APR, 5% Risk buffer
///     * 3: 5% APR, 10% Risk buffer
///     * 4: 3% APR, 25% Risk buffer
/// * Whitelistings
/// 	* Keyring::TrancheInvestor(index) accounts with index 0 - 9 for tranche with id 0
///  	* Keyring::TrancheInvestor(index) accounts with index 10 - 19 for tranche with id 1
/// 	* Keyring::TrancheInvestor(index) accounts with index 20 - 29 for tranche with id 2
/// 	* Keyring::TrancheInvestor(index) accounts with index 30 - 39 for tranche with id 3
/// 	* Keyring::TrancheInvestor(index) accounts with index 40 - 49 for tranche with id 4
/// * Currency: CurrencyId::Usd,
/// * MaxReserve: 100_000 Usd
pub fn default_pool_calls(admin: AccountId, pool_id: PoolId, nfts: &mut NftManager) -> Vec<Call> {
	pool_setup_calls(
		admin,
		pool_id,
		CurrencyId::Usd,
		100_000 * DECIMAL_BASE_12,
		create_tranche_input(
			vec![None, Some(10), Some(7), Some(5), Some(3)],
			vec![None, Some(5), Some(5), Some(10), Some(25)],
			None,
		),
		nfts,
	)
}

/// Creates the necessary calls for setting up a pool. Given the input.
/// Per default there will be 10 investors whitelisted per tranche.
/// Order goes like follow `whitelist_10_for_each_tranche_calls` docs explain.
/// Furthermore, it will create the necessary calls for creating the
/// collateral-nft and loan-nft classes in the Uniques pallet.
pub fn pool_setup_calls(
	admin: AccountId,
	pool_id: PoolId,
	currency: CurrencyId,
	max_reserve: Balance,
	tranche_input: Vec<TrancheInput<Rate>>,
	nfts: &mut NftManager,
) -> Vec<Call> {
	let mut calls = Vec::new();
	let num_tranches = tranche_input.len();
	calls.push(create_pool_call(
		admin.clone(),
		pool_id,
		currency,
		max_reserve,
		tranche_input,
	));
	calls.extend(whitelist_admin(admin.clone(), pool_id));
	calls.extend(whitelist_10_for_each_tranche_calls(
		pool_id,
		num_tranches as u32,
	));
	calls.extend(super::loans::init_loans_for_pool(admin, pool_id, nfts));
	calls
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
) -> Vec<TrancheInput<Rate>> {
	let interest_rates = rates
		.into_iter()
		.map(|rate| {
			if let Some(rate) = rate {
				Some(tokens::rate_from_percent(rate) / *YEAR_RATE + One::one())
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
				(
					TrancheType::NonResidual {
						interest_rate_per_sec,
						min_risk_buffer,
					},
					seniority,
				)
			} else {
				(TrancheType::Residual, seniority)
			}
		})
		.collect()
}

/// Enables permission for all existing `PoolRole` variants
/// (except for PoolRole::TrancheInvestor) for the given account
pub fn whitelist_admin(admin: AccountId, pool_id: PoolId) -> Vec<Call> {
	let mut calls = Vec::new();
	calls.push(permission_call(
		PoolRole::PoolAdmin,
		admin.clone(),
		pool_id,
		PoolRole::Borrower,
	));
	calls.push(permission_call(
		PoolRole::PoolAdmin,
		admin.clone(),
		pool_id,
		PoolRole::LiquidityAdmin,
	));
	calls.push(permission_call(
		PoolRole::PoolAdmin,
		admin.clone(),
		pool_id,
		PoolRole::RiskAdmin,
	));
	calls.push(permission_call(
		PoolRole::PoolAdmin,
		admin.clone(),
		pool_id,
		PoolRole::MemberListAdmin,
	));
	calls.push(permission_call(
		PoolRole::PoolAdmin,
		admin.clone(),
		pool_id,
		PoolRole::PricingAdmin,
	));

	calls
}

/// This should only be used at start-up of a pool
/// The function generated xts for whitelisting 10
/// investors per tranche.
///
/// Note:
/// * Tranche-ids are calcualted as if no tranches were removed or added
///    -> tranche-id for residual tranche blake2_128::hash((0, pool_id))
/// * Investor accounts whitelisted for respective tranche like
///    * Investors whitelisted for tranche 0
///		  * Keyring::TrancheInvestor(0)
///       * Keyring::TrancheInvestor(1)
///       * Keyring::TrancheInvestor(2)
///       * Keyring::TrancheInvestor(3)
///       * Keyring::TrancheInvestor(4)
///       * Keyring::TrancheInvestor(5)
///       * Keyring::TrancheInvestor(6)
///       * Keyring::TrancheInvestor(7)
///       * Keyring::TrancheInvestor(8)
///       * Keyring::TrancheInvestor(9)
///   * Investors whitelisted for tranche 1
///       * Keyring::TrancheInvestor(10)
///       * Keyring::TrancheInvestor(11)
///       * Keyring::TrancheInvestor(12)
///       * Keyring::TrancheInvestor(13)
///       * Keyring::TrancheInvestor(14)
///       * Keyring::TrancheInvestor(15)
///       * Keyring::TrancheInvestor(16)
///       * Keyring::TrancheInvestor(17)
///       * Keyring::TrancheInvestor(18)
///       * Keyring::TrancheInvestor(19)
pub fn whitelist_10_for_each_tranche_calls(pool: PoolId, num_tranches: u32) -> Vec<Call> {
	let mut calls = Vec::with_capacity(10 * num_tranches as usize);

	let mut x: u32 = 0;
	while x < num_tranches {
		for id in 0..10 {
			calls.push(whitelist_investor_call(
				pool,
				Keyring::TrancheInvestor((x * 10) + id),
				tranche_id(pool, x as u64),
			));
		}
		x += 1;
	}

	calls
}

/// Whitelist a given investor for a fiven pool and tranche for 1 year of time
pub fn whitelist_investor_call(pool: PoolId, investor: Keyring, tranche: TrancheId) -> Call {
	permission_call(
		PoolRole::PoolAdmin,
		investor.to_account_id(),
		pool,
		PoolRole::TrancheInvestor(tranche, 2 * SECONDS_PER_YEAR),
	)
}

/// Creates a permission xt with the given input
pub fn permission_call(
	with_role: PoolRole,
	to: AccountId,
	location: PoolId,
	role: PoolRole,
) -> Call {
	Call::Permissions(PermissionsCall::add {
		with_role,
		to,
		location,
		role,
	})
}

pub fn create_pool_call(
	admin: AccountId,
	pool_id: PoolId,
	currency: CurrencyId,
	max_reserve: Balance,
	tranches: Vec<TrancheInput<Rate>>,
) -> Call {
	Call::Pools(PoolsCall::create {
		admin,
		pool_id,
		tranches,
		currency,
		max_reserve,
	})
}

pub fn invest_order_call(pool_id: PoolId, index: TrancheIndex, amount: Balance) -> Call {
	Call::Pools(PoolsCall::update_invest_order {
		pool_id,
		tranche_loc: TrancheLoc::Index(index),
		amount,
	})
}

pub fn redeem_order_call(pool_id: PoolId, index: TrancheIndex, amount: Balance) -> Call {
	Call::Pools(PoolsCall::update_redeem_order {
		pool_id,
		tranche_loc: TrancheLoc::Index(index),
		amount,
	})
}

pub fn close_epoch(pool_id: PoolId) -> Call {
	Call::Pools(PoolsCall::close_epoch { pool_id })
}

/// Calculates the tranche-id for pools at start-up. Makes it easier
/// to whitelist.
///
/// Logic: Blake2_128::hash((tranche_index, pool_id))
fn tranche_id(pool: PoolId, index: TrancheIndex) -> TrancheId {
	Blake2_128::hash((index, pool).encode().as_slice()).into()
}

/// A module where all calls need to be called within an
/// externalities provided environment.
pub mod with_ext {
	use super::*;
	use common_traits::PoolNAV;
	use runtime_common::Amount;
	use std::time::Duration;

	/// Retrieves the token prices of a pool at the state that
	/// this is called with.
	///
	/// **Needs: Externalities**
	///
	/// NOTE:
	/// * update_nav() is called with Keyring::Admin as calley
	pub fn get_tranche_prices(pool: PoolId) -> Vec<Rate> {
		let now = Duration::from_millis(Timestamp::now()).as_secs();
		let mut details = Pools::pool(pool).expect("POOLS: Getting pool failed.");
		Loans::update_nav(Keyring::Admin.into(), pool).expect("LOANS: UpdatingNav failed");
		let (epoch_nav, _) =
			<Loans as PoolNAV<PoolId, Amount>>::nav(pool).expect("LOANS: Getting NAV failed");

		let total_assets = details.reserve.total + epoch_nav.into_inner();

		details
			.tranches
			.calculate_prices::<_, OrmlTokens, _>(total_assets, now)
			.expect("POOLS: Calculating tranche-prices failed")
	}
}
