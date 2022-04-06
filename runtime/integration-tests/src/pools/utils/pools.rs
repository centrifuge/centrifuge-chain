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
	Call, Index, Loans, Permissions, Pools, UncheckedExtrinsic, PARA_ID,
};
use crate::pools::utils::extrinsics::{ext_centrifuge, nonce_centrifuge};
use crate::pools::utils::{
	accounts::{default_investors, Keyring},
	env::TestEnv,
	time::{blocks::*, secs::*},
	tokens::{DECIMAL_BASE_12, YEAR_RATE},
};
use codec::Encode;
use common_traits::Permissions as PermissionsT;
use common_types::{CurrencyId, PoolRole};
use frame_support::Blake2_128;
use fudge::primitives::Chain;
use pallet_pools::{Call as PoolsCall, TrancheIndex, TrancheInput, TrancheType};
use runtime_common::{
	AccountId, Balance, ClassId, InstanceId, Moment, PoolId, Rate, TrancheId, CFG,
};
use sp_runtime::traits::{One, StaticLookup};

/// Creates a pool with the following spec
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
pub fn default_pool(env: &TestEnv, admin: Keyring, nonce: Index, id: PoolId) -> Index {
	let xt = create_pool_xt(
		env,
		admin,
		nonce,
		id,
		CurrencyId::Usd,
		100_000 * DECIMAL_BASE_12,
		vec![None, Some(10), Some(7), Some(5), Some(3)],
		vec![None, Some(5), Some(5), Some(10), Some(25)],
		None,
	)
	.expect("ESSENTIAL: Creating a pool failed.");

	env.append_extrinsic(Chain::Para(PARA_ID), xt.encode())
		.expect("ESSENTIAL: Appending extrinisc to parachain failed.");

	env.centrifuge
		.with_state(|| whitelist_investors(id, 5))
		.expect("ESSENTIAL: with_state failed for centrifuge.");

	Uniques::create(
		into_signed(get_admin()),
		get_loan_nft_class_id(id),
		Address::Id(get_admin()),
	)
	.unwrap();
	Uniques::create(into_signed(get_admin()), id, Address::Id(get_admin())).unwrap();
	Loans::initialise_pool(into_signed(get_admin()), id, get_loan_nft_class_id(id)).unwrap();
}

pub fn create_pool_xt(
	env: &TestEnv,
	who: Keyring,
	nonce: Index,
	pool_id: PoolId,
	currency: CurrencyId,
	max_reserve: u128,
	rates: Vec<Option<u64>>,
	risk_buffs: Vec<Option<u64>>,
	seniorities: Option<Vec<Option<u32>>>,
) -> Result<UncheckedExtrinsic, ()> {
	let mut interest_rates = rates
		.into_iter()
		.map(|rate| {
			if let Some(rate) = rate {
				Some(Rate::saturating_from_rational(rate, 100) / YEAR_RATE + One::one())
			} else {
				None
			}
		})
		.collect::<Vec<Option<_>>>();

	let mut risk_buffs = risk_buffs
		.into_iter()
		.map(|buffs| {
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
		risk_buffs.iter().map(|(index, _)| None).collect()
	};

	let tranches = interest_rates
		.into_iter()
		.zip(risk_buffs)
		.zip(seniority)
		.rev()
		.map(|((rate, buff), seniority)| {
			if (Some(interest_rate_per_sec), Some(min_risk_buffer)) = (rate, buff) {
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
		.collect();

	ext_centrifuge(
		env,
		who,
		nonce,
		Call::Pools(PoolCalls::create {
			admin: who.into(),
			pool_id,
			tranches,
			currency,
			max_reserve,
		}),
	)
}

/// Whitelists 10 tranche-investors per tranche.
///
/// **Needs: Externalities**
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

pub fn get_loan_nft_class_id(id: PoolId) -> ClassId {
	id + 1000
}

pub fn issue_loan(pool: PoolId, amount: Balance) -> InstanceId {
	let rev_admin =
		<<Runtime as frame_system::Config>::Lookup as StaticLookup>::unlookup(get_admin());

	Uniques::mint(
		into_signed(get_admin()),
		pool,
		get_next_asset_id(pool),
		rev_admin,
	)
	.unwrap();

	let id = InstanceId(Loans::get_next_loan_id());
	Loans::create(
		into_signed(get_admin()),
		pool,
		Asset(pool, get_curr_asset_id(pool)),
	)
	.unwrap();

	Loans::price(
		into_signed(get_admin()),
		pool,
		id,
		rate_per_sec(get_rate(15)).unwrap(),
		LoanType::BulletLoan(BulletLoan::new(
			get_rate(90),
			get_rate(5),
			get_rate(50),
			Amount::from_inner(amount),
			rate_per_sec(get_rate(4)).unwrap(),
			get_date_from_delta(30 * SECONDS_PER_DAY),
		)),
	)
	.unwrap();

	id
}

pub fn get_date_from_delta(delta: Moment) -> Moment {
	Timestamp::now() + delta
}

pub fn get_rate(perc: u32) -> Rate {
	Rate::saturating_from_rational(perc, 100)
}
pub fn get_next_asset_id(class: ClassId) -> InstanceId {
	AssetIds::next_id(class)
}

pub fn get_curr_asset_id(class: ClassId) -> InstanceId {
	AssetIds::current_id(class)
}

pub fn get_tranche_prices(pool: PoolId) -> Vec<Balance> {
	let (epoch_nav, _) = <Loans as PoolNAV<PoolId, Amount>>::nav(pool).unwrap();
	let PoolDetails {
		owner,
		currency,
		mut tranches, // ordered junior => senior
		current_epoch,
		last_epoch_closed,
		last_epoch_executed,
		max_reserve,
		available_reserve,
		total_reserve,
		metadata,
		min_epoch_time,
		challenge_time,
		max_nav_age,
	} = Pools::pool(pool).unwrap();

	let epoch_reserve = total_reserve;

	Pools::calculate_tranche_prices(pool, epoch_nav.into(), epoch_reserve, &mut tranches)
		.unwrap()
		.into_iter()
		.map(|rate| rate.into_inner())
		.collect()
}

/// Add a permission for who, at pool with role.
///
/// **Needs: Externalities**
pub fn permission_for(who: AccountId, pool: PoolId, role: PoolRole) {
	<Permissions as PermissionsT<AccountId>>::add(pool, who, role)
		.expect("ESSENTIAL: Adding a permission for a role should not fail.");
}

/// Adds all permissions `PoolRole` currently provides to the Keyring::Admin account
///
/// **Needs: Externalities**
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
/// **Needs: Externalities**
pub fn permit_investor(investor: u32, pool: PoolId, tranche: TrancheId) {
	permission_for(
		Keyring::TrancheInvestor(investor).into(),
		pool,
		PoolRole::TrancheInvestor(tranche, SECONDS_PER_YEAR),
	)
}

pub fn tranche_id(pool: PoolId, index: TrancheIndex) -> TrancheId {
	Blake2_128::hash((index, pool).encode().as_slice()).into()
}
