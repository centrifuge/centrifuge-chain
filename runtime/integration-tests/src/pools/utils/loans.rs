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

//! Utilities around the loans pallet
use crate::chain::centrifuge::UncheckedExtrinsic;
use crate::pools::utils::accounts::Keyring;
use crate::pools::utils::env::TestEnv;
use runtime_common::{ClassId, Index, InstanceId, PoolId};
use std::collections::HashMap;

/// Structure that manages collateral and loan nft ids
pub struct NftManager {
	collaterals: HashMap<PoolId, InstanceId>,
	loans: HashMap<PoolId, InstanceId>,
}

/// The id we use for loans
pub type LoanId = InstanceId;

// The id we use for collaterals
pub type CollateralId = InstanceId;

impl NftManager {
	pub fn new() -> Self {
		Self {
			collaterals: HashMap::new(),
			loans: HashMap::new(),
		}
	}

	/// Currently simply maps pool_id = loan_class_id for a pool
	pub fn loan_class_id(&self, pool_id: PoolId) -> ClassId {
		pool_id
	}

	/// Maps pool_id << 32 = collateral_class id
	///
	/// panics if pool_id > u32::MAX as this would result in an overflow
	/// during shifting.
	pub fn collateral_class_id(&self, pool_id: PoolId) -> ClassId {
		assert!(pool_id <= u32::MAX.into());
		pool_id << 32
	}

	pub fn curr_loan_id(&mut self, pool_id: PoolId) -> InstanceId {
		self.loans.entry(pool_id).or_insert(InstanceId(0)).clone()
	}

	pub fn next_loan_id(&mut self, pool_id: PoolId) -> InstanceId {
		let id = self.loans.entry(pool_id).or_insert(InstanceId(0));
		let next = id.clone();
		*id = InstanceId(id.0);
		next
	}

	pub fn curr_collateral_id(&mut self, pool_id: PoolId) -> InstanceId {
		self.loans.entry(pool_id).or_insert(InstanceId(0)).clone()
	}

	pub fn next_collateral_id(&mut self, pool_id: PoolId) -> InstanceId {
		let id = self.collaterals.entry(pool_id).or_insert(InstanceId(0));
		let next = id.clone();
		*id = InstanceId(id.0);
		next
	}
}

/// Creates the necessary extrinsics to initialises a pool in the loans pallet.
/// The pool must already exist for this extrinsics to succeed.
///
/// Extrinsics that are generated:
/// * Loans::initialise_pool
/// * Uniques::create -> for Loan nft class
/// * Uniques::create -> for Collateral nft class
pub fn init_loans_for_pool(
	env: &mut TestEnv,
	owner: Keyring,
	nonce: Index,
	pool: PoolId,
) -> Result<(Vec<UncheckedExtrinsic>, Index), ()> {
	todo!()
}

pub fn initialise_pool_xt() -> Result<(UncheckedExtrinsic, Index), ()> {
	todo!()
}

pub fn price_loan_xt() -> Result<(UncheckedExtrinsic, Index), ()> {
	todo!()
}

pub fn create_nft_xt() -> Result<(UncheckedExtrinsic, Index), ()> {
	todo!()
}

pub fn mint_nft_xt() -> Result<(UncheckedExtrinsic, Index), ()> {
	todo!()
}

pub fn issue_loan() -> Result<(Vec<UncheckedExtrinsic>, Index), ()> {
	todo!()
}

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

/*

/// A module where all calls need to be called within an
/// externalities provided environment.
pub mod with_ext {

	/// Issues a loan with given amount for the pool
	///
	/// **Needs: Mut Externalities to persist**
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
}

 */
