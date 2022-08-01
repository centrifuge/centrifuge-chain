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

///! Tests for some types in the common section for our runtimes
use super::*;
use core::time::Duration;
use frame_support::parameter_types;

parameter_types! {
	pub const MinDelay: u64 = 4;
}

struct Now(core::time::Duration);
impl Now {
	fn pass(delta: u64) {
		unsafe {
			let current = NOW_HOLDER;
			NOW_HOLDER = current + delta;
		};
	}

	fn set(now: u64) {
		unsafe {
			NOW_HOLDER = now;
		};
	}
}

static mut NOW_HOLDER: u64 = 0;
impl UnixTime for Now {
	fn now() -> Duration {
		unsafe { Duration::new(NOW_HOLDER, 0) }
	}
}

/// The exists call does not care what is passed as moment. This type shall reflect that
const UNION: u64 = 0u64;
/// The tranceh id type we use in our runtime-common. But we don't want a dependency here.
type TrancheId = [u8; 16];

fn into_tranche_id(val: u8) -> TrancheId {
	[val; 16]
}

#[test]
fn permission_roles_work() {
	assert!(PermissionRoles::<Now, MinDelay, TrancheId>::default().empty());

	let mut roles = PermissionRoles::<Now, MinDelay, TrancheId>::default();

	// Updating works only when increasing permissions
	assert!(roles
		.add(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(30),
			10
		)))
		.is_ok());
	assert!(roles
		.add(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(30),
			9
		)))
		.is_err());
	assert!(roles
		.add(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(30),
			11
		)))
		.is_ok());

	// Test zero-tranche handling
	assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(0),
		UNION
	))));
	assert!(roles
		.add(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			MinDelay::get()
		)))
		.is_ok());
	assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(0),
		UNION
	))));

	// Removing before MinDelay fails
	assert!(roles
		.rm(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			0
		)))
		.is_err());
	Now::pass(1);
	assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(0),
		UNION
	))));
	assert!(roles
		.rm(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			MinDelay::get() - 1
		)))
		.is_err());
	assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(0),
		UNION
	))));
	Now::set(0);

	// Removing after MinDelay works (i.e. this is after min_delay the account will be invalid)
	assert!(roles
		.rm(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(0),
			MinDelay::get()
		)))
		.is_ok());
	Now::pass(6);
	assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(0),
		UNION
	))));
	Now::set(0);

	// Multiple tranches work
	assert!(roles
		.add(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(1),
			MinDelay::get()
		)))
		.is_ok());
	assert!(roles
		.add(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(2),
			MinDelay::get()
		)))
		.is_ok());
	assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(1),
		UNION
	))));
	assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(2),
		UNION
	))));

	// Adding roles works normally
	assert!(roles.add(Role::PoolRole(PoolRole::LiquidityAdmin)).is_ok());
	assert!(roles.add(Role::PoolRole(PoolRole::MemberListAdmin)).is_ok());
	assert!(roles.exists(Role::PoolRole(PoolRole::LiquidityAdmin)));
	assert!(roles.exists(Role::PoolRole(PoolRole::MemberListAdmin)));

	// Role exists for as long as permission is given
	assert!(roles
		.add(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(8),
			MinDelay::get() + 2
		)))
		.is_ok());
	assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(8),
		UNION
	))));
	Now::pass(MinDelay::get() + 2);
	assert!(roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(8),
		UNION
	))));
	Now::pass(1);
	assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(8),
		UNION
	))));
	Now::set(0);

	// Role must be added for at least min_delay
	assert!(roles
		.add(Role::PoolRole(PoolRole::TrancheInvestor(
			into_tranche_id(5),
			MinDelay::get() - 1
		)))
		.is_err());
	assert!(!roles.exists(Role::PoolRole(PoolRole::TrancheInvestor(
		into_tranche_id(5),
		UNION
	))));

	// Removing roles work normally for Non-TrancheInvestor roles
	assert!(roles.rm(Role::PoolRole(PoolRole::LiquidityAdmin)).is_ok());
	assert!(roles.rm(Role::PoolRole(PoolRole::MemberListAdmin)).is_ok());
	assert!(!roles.exists(Role::PoolRole(PoolRole::LiquidityAdmin)));
	assert!(!roles.exists(Role::PoolRole(PoolRole::MemberListAdmin)));
}

/// Sanity check for every CurrencyId variant's encoding value.
/// This will stop us from accidentally moving or dropping variants
/// around which could have silent but serious negative consequences.
#[test]
fn currency_id_encode_sanity() {
	use CurrencyId::*;

	// Verify that every variant encodes to what we would expect it to.
	// If this breaks, we must have changed the order of a variant, added
	// a new variant in between existing variants, or deleted one.
	vec![Native, Tranche(42, [42; 16]), KSM, AUSD, ForeignAsset(89)]
		.into_iter()
		.for_each(|variant| {
			let encoded_u64: Vec<u64> = variant.encode().iter().map(|x| *x as u64).collect();

			assert_eq!(encoded_u64, expected_encoding_value(variant))
		});

	/// Return the expected encoding.
	/// This is useful to force at compile time that we handle all existing variants.
	fn expected_encoding_value(id: CurrencyId) -> Vec<u64> {
		match id {
			Native => vec![0],
			Tranche(pool_id, tranche_id) => {
				let mut r = vec![1, pool_id, 0, 0, 0, 0, 0, 0, 0];
				r.append(&mut tranche_id.map(|x| x as u64).to_vec());
				r
			}
			KSM => vec![2],
			AUSD => vec![3],
			ForeignAsset(id) => vec![4, id as u64, 0, 0, 0],
		}
	}
}
