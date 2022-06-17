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
	assert_eq!(CurrencyId::Native.encode(), vec![0]);
	assert_eq!(
		CurrencyId::Tranche(42, [42; 16]).encode(),
		[
			1, 42, 0, 0, 0, 0, 0, 0, 0, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42,
			42
		]
	);
	assert_eq!(CurrencyId::KSM.encode(), vec![2]);
	assert_eq!(CurrencyId::KUSD.encode(), vec![3]);
	assert_eq!(CurrencyId::AUSD.encode(), vec![4]);
	assert_eq!(CurrencyId::ForeignAsset(1).encode(), vec![5, 1, 0, 0, 0]);
	assert_eq!(
		CurrencyId::ForeignAsset(789).encode(),
		vec![5, 789, 0, 0, 0]
	);
}
