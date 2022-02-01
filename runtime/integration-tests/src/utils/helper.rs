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
#![allow(dead_code)]

use crate::utils::setup::*;
use codec::{Decode, Encode};
use common_traits::{Permissions as PermissionsT, PoolNAV};
use common_types::PoolRole;
use frame_support::dispatch::DispatchResult;
use pallet_loans::types::NAVDetails;
use pallet_loans::{
	loan_type::{BulletLoan, LoanType},
	math::*,
	types::Asset,
};
use pallet_pools::{Error as PoolError, PoolDetails, TrancheInput};
use runtime_common::{AccountId, Address, Balance, PoolId, TrancheId, CFG as CURRENCY};
use sp_arithmetic::traits::Zero;
use sp_io::hashing::blake2_256;
use sp_runtime::sp_std::collections::btree_map::BTreeMap;
use sp_runtime::traits::StaticLookup;
use sp_runtime::{traits::One, FixedPointNumber, Perquintill};

static mut ASSET_IDS: *mut BTreeMap<ClassId, InstanceId> =
	0usize as *mut BTreeMap<ClassId, InstanceId>;

pub struct AssetIds;
impl AssetIds {
	pub fn get() -> &'static mut BTreeMap<ClassId, InstanceId> {
		unsafe {
			if ASSET_IDS.is_null() {
				let map = Box::new(BTreeMap::<ClassId, InstanceId>::new());
				ASSET_IDS = Box::into_raw(map);

				&mut *(ASSET_IDS)
			} else {
				&mut *(ASSET_IDS)
			}
		}
	}

	pub fn next_id(id: ClassId) -> InstanceId {
		let ids = AssetIds::get();
		if let Some(loan_id) = ids.get_mut(&id) {
			*loan_id = InstanceId(loan_id.0 + 1);
			*loan_id
		} else {
			let loan_id = InstanceId(0);
			ids.insert(id, loan_id);
			loan_id
		}
	}

	pub fn current_id(id: ClassId) -> InstanceId {
		let ids = AssetIds::get();
		if let Some(loan_id) = ids.get_mut(&id) {
			*loan_id
		} else {
			let loan_id = InstanceId(0);
			ids.insert(id, loan_id);
			loan_id
		}
	}
}

pub fn invest_close_and_collect(
	pool_id: PoolId,
	investments: Vec<(Origin, TrancheId, Balance)>,
) -> DispatchResult {
	for (who, tranche_id, investment) in investments.clone() {
		Pools::update_invest_order(who, pool_id, tranche_id, investment)?;
	}

	Pools::close_epoch(Origin::signed(get_admin()), pool_id)?;

	let epoch = pallet_pools::Pool::<Runtime>::try_get(pool_id)
		.map_err(|_| PoolError::<Runtime>::NoSuchPool)?
		.last_epoch_closed;

	for (who, tranche_id, _) in investments {
		Pools::collect(who, pool_id, tranche_id, epoch as u32)?;
	}

	Ok(())
}

/// Creates a pool with the following spec
///
/// * Pool id as given
/// * Admin = get_admin() account
/// * 5 Tranches
///     * 0: 3% APR, 25% Risk buffer, Seniority 5
///     * 1: 5% APR, 10% Risk buffer, Seniority 4
///     * 2: 7% APR, 5% Risk buffer, Seniority 2
///     * 3: 10% APR, 5% Risk buffer, Seniority 1
///     * 4: Junior Tranche
/// * Whitelistings
/// 	* accounts with index 0 - 9 for tranche with id 0
///  	* accounts with index 10 - 19 for tranche with id 1
/// 	* accounts with index 20 - 29 for tranche with id 2
/// 	* accounts with index 30 - 39 for tranche with id 3
/// 	* accounts with index 40 - 49 for tranche with id 4
/// * Currency: CurrencyId::USD,
/// * MaxReserve: 1000
pub fn create_default_pool(id: PoolId) {
	let year_rate = Rate::saturating_from_integer(SECONDS_PER_YEAR);

	let rates = vec![3, 5, 7, 10];
	let mut interest_rates = rates
		.into_iter()
		.map(|rate| Some(Rate::saturating_from_rational(rate, 100) / year_rate + One::one()))
		.collect::<Vec<Option<_>>>();
	interest_rates.push(None);

	let risk_buffs: Vec<u64> = vec![25, 10, 5, 5];
	let mut risk_buffs = risk_buffs
		.into_iter()
		.map(|buffs| Some(Perquintill::from_percent(buffs)))
		.collect::<Vec<Option<_>>>();
	risk_buffs.push(None);

	let seniority: Vec<u32> = vec![5, 4, 2, 1];
	let mut seniority: Vec<Option<u32>> = seniority.into_iter().map(|sen| Some(sen)).collect();
	seniority.push(None);

	let tranches = interest_rates
		.into_iter()
		.zip(risk_buffs)
		.zip(seniority)
		.rev()
		.map(|((rate, buff), seniority)| TrancheInput {
			interest_per_sec: rate,
			min_risk_buffer: buff,
			seniority: seniority,
		})
		.collect();

	(0..10)
		.into_iter()
		.map(|idx| permit_investor(idx, 0, 0))
		.for_each(drop);
	(10..20)
		.into_iter()
		.map(|idx| permit_investor(idx, 0, 1))
		.for_each(drop);
	(20..30)
		.into_iter()
		.map(|idx| permit_investor(idx, 0, 2))
		.for_each(drop);
	(30..40)
		.into_iter()
		.map(|idx| permit_investor(idx, 0, 3))
		.for_each(drop);
	(40..50)
		.into_iter()
		.map(|idx| permit_investor(idx, 0, 4))
		.for_each(drop);

	permit_admin(id);

	Pools::create(
		into_signed(get_admin()),
		id,
		tranches,
		CurrencyId::Usd,
		1000 * CURRENCY,
	)
	.unwrap();
	Uniques::create(
		into_signed(get_admin()),
		get_loan_nft_class_id(id),
		Address::Id(get_admin()),
	)
	.unwrap();
	Uniques::create(into_signed(get_admin()), id, Address::Id(get_admin())).unwrap();
	Loans::initialise_pool(into_signed(get_admin()), id, get_loan_nft_class_id(id)).unwrap();
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

pub fn account(name: &'static str, index: u32, seed: u32) -> AccountId {
	let entropy = (name, index, seed).using_encoded(blake2_256);
	AccountId::decode(&mut &entropy[..]).unwrap()
}

pub fn permission_for(who: AccountId, pool: PoolId, role: PoolRole) {
	<Permissions as PermissionsT<AccountId>>::add_permission(pool, who, role).unwrap();
}

pub fn permit_admin(id: PoolId) {
	permission_for(get_admin(), id, PoolRole::PricingAdmin);
	permission_for(get_admin(), id, PoolRole::LiquidityAdmin);
	permission_for(get_admin(), id, PoolRole::RiskAdmin);
	permission_for(get_admin(), id, PoolRole::MemberListAdmin);
	permission_for(get_admin(), id, PoolRole::Borrower);
}

pub fn permit_investor(investor: u32, pool: PoolId, tranche: TrancheId) {
	permission_for(
		get_account(investor),
		pool,
		PoolRole::TrancheInvestor(tranche, u64::MAX),
	)
}

pub fn get_account(idx: u32) -> AccountId {
	account("user", idx, 0)
}

pub fn get_signed(idx: u32) -> Origin {
	into_signed(get_account(idx))
}

pub fn into_signed(account: AccountId) -> Origin {
	Origin::signed(account)
}

pub fn get_root() -> Origin {
	Origin::root()
}

pub fn get_admin() -> AccountId {
	account("admin", 0, 0)
}
