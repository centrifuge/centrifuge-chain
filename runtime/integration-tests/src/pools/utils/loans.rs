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
use lazy_static::lazy_static;
use runtime_common::{ClassId, InstanceId, PoolId};
use std::sync::atomic::{AtomicU64, Ordering};

lazy_static! {
	static ref CLASS_ID: AtomicU64 = AtomicU64::new(0);
	static ref INSTANCE_ID: AtomicU64 = AtomicU64::new(0);
}

/// Returns the current class id counter. This id might
/// already be in use.
pub fn curr_class_id() -> ClassId {
	CLASS_ID.load(Ordering::SeqCst)
}

/// Increase the counter of class ids and returns the
/// next unused class id.
///
/// Overflows and panics of `curr_class_id == u64::MAX`
pub fn next_class_id() -> u64 {
	let prev = CLASS_ID.fetch_add(1, Ordering::SeqCst);
	prev + 1
}

/// Shifts the id 64 bits into the u128 range that
/// u64 does not cover
fn shift_to_instance_id_range(id: u64) -> u128 {
	((id as u128) << 64)
}
/// Increase the counter of asset ids and returns the
/// next unused asset id.
pub fn next_asset_id() -> InstanceId {
	let prev = INSTANCE_ID.fetch_add(1, Ordering::SeqCst);
	InstanceId(shift_to_instance_id_range(prev + 1))
}

/// Returns the current asset id counter. This id might
/// already be in use.
pub fn curr_asset_id() -> InstanceId {
	InstanceId(shift_to_instance_id_range(
		INSTANCE_ID.load(Ordering::SeqCst),
	))
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
