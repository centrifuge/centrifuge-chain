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

use crate::chain::centrifuge::{
	Call, Index, Loans, OrmlTokens, Permissions, Pools, Timestamp, UncheckedExtrinsic,
};
use crate::pools::utils::extrinsics::xt_centrifuge;
use crate::pools::utils::{
	accounts::{increase_nonce, Keyring},
	env::TestEnv,
	time::secs::*,
	tokens,
	tokens::{DECIMAL_BASE_12, YEAR_RATE},
};
use codec::Encode;
use common_traits::Permissions as PermissionsT;
use common_types::{CurrencyId, PoolRole};
use frame_support::{Blake2_128, StorageHasher};
use pallet_permissions::Call as PermissionsCall;
use pallet_pools::{Call as PoolsCall, TrancheIndex, TrancheInput, TrancheType};
use runtime_common::{AccountId, Balance, PoolId, Rate, TrancheId};
use sp_runtime::{traits::One, FixedPointNumber, Perquintill};

/// Creates the necessary extrinsics for initialising a pool.
/// This includes:
/// * creating a pool
/// * whitelisting investors
/// * initialising the loans pallet for the given pool
///
/// Extrinsics are returned and must be submitted to the transaction pool
/// in order to be included into the next block.
///
/// * Pool id as given
/// * Admin as provided (also owner of pool)
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
pub fn default_pool(
	env: &mut TestEnv,
	admin: Keyring,
	nonce: Index,
	id: PoolId,
) -> Result<Vec<Call>, ()> {
	let mut curr_nonce = nonce;
	let mut xts = Vec::new();

	let (xt, mut curr_nonce) = create_pool_xt(
		env,
		admin,
		curr_nonce,
		id,
		CurrencyId::Usd,
		100_000 * DECIMAL_BASE_12,
		create_tranche_input(
			vec![None, Some(10), Some(7), Some(5), Some(3)],
			vec![None, Some(5), Some(5), Some(10), Some(25)],
			None,
		),
	)
	.expect("ESSENTIAL: Creating a pool failed.");
	xts.push(xt);

	let (whitelists_xts, mut curr_nonce) =
		whitelist_10_for_each_tranche_xts(env, id, admin, curr_nonce, 5);
	xts.extend(whitelists_xts);

	/*
	Uniques::create(
		into_signed(get_admin()),
		get_loan_nft_class_id(id),
		Address::Id(get_admin()),
	)
	.unwrap();
	Uniques::create(into_signed(get_admin()), id, Address::Id(get_admin())).unwrap();
	Loans::initialise_pool(into_signed(get_admin()), id, get_loan_nft_class_id(id)).unwrap();

	 */

	Ok((xts, curr_nonce))
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
	let mut interest_rates = rates
		.into_iter()
		.map(|rate| {
			if let Some(rate) = rate {
				Some(tokens::rate_from_percent(rate) / *YEAR_RATE + One::one())
			} else {
				None
			}
		})
		.collect::<Vec<Option<_>>>();

	let mut risk_buffs = risk_buffs
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
		.rev()
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

/// This should only be used at start-up of a pool
/// The function generated xts for whitelisting 10
/// investors per tranche.
///
/// Note:
/// * Tranche-ids are calcualted as if no tranches were removed or added
///    -> tranche-id for residual tranche blake2_128::hash((0, pool_id))
/// * Investor accounts whitelisted for respective tranche like
///    * Investors whitelisted for tranche 0
///       * Keyring::TrancheInvestor(1)
///       * Keyring::TrancheInvestor(2)
///       * Keyring::TrancheInvestor(3)
///       * Keyring::TrancheInvestor(4)
///       * Keyring::TrancheInvestor(5)
///       * Keyring::TrancheInvestor(6)
///       * Keyring::TrancheInvestor(7)
///       * Keyring::TrancheInvestor(8)
///       * Keyring::TrancheInvestor(9)
///       * Keyring::TrancheInvestor(10)
///   * Investors whitelisted for tranche 1
///       * Keyring::TrancheInvestor(11)
///       * Keyring::TrancheInvestor(12)
///       * Keyring::TrancheInvestor(13)
///       * Keyring::TrancheInvestor(14)
///       * Keyring::TrancheInvestor(15)
///       * Keyring::TrancheInvestor(16)
///       * Keyring::TrancheInvestor(17)
///       * Keyring::TrancheInvestor(18)
///       * Keyring::TrancheInvestor(19)
///       * Keyring::TrancheInvestor(20)
pub fn whitelist_10_for_each_tranche_xts(
	env: &TestEnv,
	pool: PoolId,
	admin: Keyring,
	nonce: Index,
	num_tranches: u32,
) -> (Vec<UncheckedExtrinsic>, Index) {
	let mut xts = Vec::with_capacity(10 * num_tranches as usize);
	let mut curr_nonce = nonce;

	let mut x: u32 = 0;
	while x < num_tranches {
		for id in 1..11 {
			xts.push(whitelist_investor_xt(
				env,
				admin,
				nonce,
				pool,
				Keyring::TrancheInvestor((x * 10) + id),
				tranche_id(pool, x as u64),
			));
			increase_nonce(&mut curr_nonce)
		}
	}

	(xts, curr_nonce)
}

/// Whitelist a given investor for a fiven pool and tranche for 1 year of time
pub fn whitelist_investor_xt(
	env: &TestEnv,
	admin: Keyring,
	nonce: Index,
	pool: PoolId,
	investor: Keyring,
	tranche: TrancheId,
) -> UncheckedExtrinsic {
	permission_xt(
		env,
		admin,
		nonce,
		PoolRole::PoolAdmin,
		investor.to_account_id(),
		pool,
		PoolRole::TrancheInvestor(tranche, SECONDS_PER_YEAR),
	)
	.expect("ESSENTIAL: Adding new roles must not fail here.")
}

/// Creates a permission xt with the given input
pub fn permission_xt(
	env: &TestEnv,
	who: Keyring,
	nonce: Index,
	with_role: PoolRole,
	to: AccountId,
	location: PoolId,
	role: PoolRole,
) -> Result<UncheckedExtrinsic, ()> {
	xt_centrifuge(
		env,
		who,
		nonce,
		Call::Permissions(PermissionsCall::add {
			with_role,
			to,
			location,
			role,
		}),
	)
}

pub fn create_pool_xt(
	env: &TestEnv,
	who: Keyring,
	nonce: Index,
	pool_id: PoolId,
	currency: CurrencyId,
	max_reserve: Balance,
	tranches: Vec<TrancheInput<Rate>>,
) -> Result<(UncheckedExtrinsic, Index), ()> {
	let mut curr_nonce = nonce;
	let xt = xt_centrifuge(
		env,
		who,
		nonce,
		Call::Pools(PoolsCall::create {
			admin: who.into(),
			pool_id,
			tranches,
			currency,
			max_reserve,
		}),
	);
	increase_nonce(&mut curr_nonce);
	xt.map(|xt| (xt, curr_nonce))
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
mod with_ext {
	use super::*;
	use common_traits::PoolNAV;
	use runtime_common::Amount;

	/// Whitelists 10 tranche-investors per tranche.
	///
	/// **Needs: Mut Externalities to persist**
	/// -------------------------------
	/// E.g.: num_tranches = 2
	/// * Investors whitelisted for tranche 0
	///    * Keyring::TrancheInvestor(1)
	///    * Keyring::TrancheInvestor(2)
	///    * Keyring::TrancheInvestor(3)
	///    * Keyring::TrancheInvestor(4)
	///    * Keyring::TrancheInvestor(5)
	///    * Keyring::TrancheInvestor(6)
	///    * Keyring::TrancheInvestor(7)
	///    * Keyring::TrancheInvestor(8)
	///    * Keyring::TrancheInvestor(9)
	///    * Keyring::TrancheInvestor(10)
	/// * Investors whitelisted for tranche 1
	///    * Keyring::TrancheInvestor(11)
	///    * Keyring::TrancheInvestor(12)
	///    * Keyring::TrancheInvestor(13)
	///    * Keyring::TrancheInvestor(14)
	///    * Keyring::TrancheInvestor(15)
	///    * Keyring::TrancheInvestor(16)
	///    * Keyring::TrancheInvestor(17)
	///    * Keyring::TrancheInvestor(18)
	///    * Keyring::TrancheInvestor(19)
	///    * Keyring::TrancheInvestor(20)
	pub fn whitelist_investors(pool_id: PoolId, num_tranches: u32) {
		let mut x: u32 = 0;
		while x < num_tranches {
			for id in 1..11 {
				let id = (x * 10) + id;
				permit_investor(id, pool_id, tranche_id(pool_id, x as u64));
			}
		}
	}

	/// Retrieves the token prices of a pool at the state that
	/// this is called with.
	///
	/// **Needs: Externalities**
	///
	/// NOTE:
	/// * update_nav() is called with Keyring::Admin as calley
	pub fn get_tranche_prices(pool: PoolId) -> Vec<Rate> {
		let now = Timestamp::now();
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

	/// Add a permission for who, at pool with role.
	///
	/// **Needs: Mut Externalities to persist**
	pub fn permission_for(who: AccountId, pool: PoolId, role: PoolRole) {
		<Permissions as PermissionsT<AccountId>>::add(pool, who, role)
			.expect("ESSENTIAL: Adding a permission for a role should not fail.");
	}

	/// Adds all roles that `PoolRole`s currently provides to the Keyring::Admin account
	///
	/// **Needs: Mut Externalities to persist**
	pub fn permit_admin(id: PoolId) {
		permission_for(Keyring::Admin.into(), id, PoolRole::PricingAdmin);
		permission_for(Keyring::Admin.into(), id, PoolRole::LiquidityAdmin);
		permission_for(Keyring::Admin.into(), id, PoolRole::RiskAdmin);
		permission_for(Keyring::Admin.into(), id, PoolRole::MemberListAdmin);
		permission_for(Keyring::Admin.into(), id, PoolRole::Borrower);
	}

	/// Add a `PoolRole::TrancheInvestor to a Keyring::TrancheInvestor(u32) account.
	/// Role is permitted for 1 year.
	///
	/// **Needs: Mut Externalities to persist**
	pub fn permit_investor(investor: u32, pool: PoolId, tranche: TrancheId) {
		permission_for(
			Keyring::TrancheInvestor(investor).into(),
			pool,
			PoolRole::TrancheInvestor(tranche, SECONDS_PER_YEAR),
		)
	}
}
