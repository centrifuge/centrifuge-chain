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
use frame_support::parameter_types;

parameter_types! {
	pub const MaxTranches: u8 = 32;
	pub const MinDelay: u64 = 4;
}

struct Now(u64);
impl Now {
	fn pass(delta: u64) {
		unsafe {
			let current = NOW_HOLDER.0;
			NOW_HOLDER = Now(current + delta);
		};
	}

	fn set(now: u64) {
		unsafe {
			NOW_HOLDER = Now(now);
		};
	}
}

static mut NOW_HOLDER: Now = Now(0);
impl Time for Now {
	type Moment = u64;

	fn now() -> Self::Moment {
		unsafe { NOW_HOLDER.0 }
	}
}

// The exists call does not care what is passed as moment. This type shall reflect that
const UNION: u64 = 0u64;

#[test]
fn permission_roles_work() {
	assert!(PermissionRoles::<Now, MaxTranches, MinDelay>::default().empty());

	let mut roles = PermissionRoles::<Now, MaxTranches, MinDelay>::default();

	// Test zero-tranche handling
	assert!(!roles.exists(PoolRole::TrancheInvestor(0, UNION)));
	roles.add(PoolRole::TrancheInvestor(0, 0));
	assert!(roles.exists(PoolRole::TrancheInvestor(0, UNION)));

	// Removing before MinDelay fails
	roles.rm(PoolRole::TrancheInvestor(0, 0));
	Now::pass(1);
	assert!(roles.exists(PoolRole::TrancheInvestor(0, UNION)));
	roles.rm(PoolRole::TrancheInvestor(0, MinDelay::get()));
	assert!(roles.exists(PoolRole::TrancheInvestor(0, MinDelay::get())));
	Now::set(0);

	// Removing after MinDelay works
	roles.rm(PoolRole::TrancheInvestor(0, 5));
	Now::pass(6);
	assert!(!roles.exists(PoolRole::TrancheInvestor(0, UNION)));
	Now::set(0);

	// Multiple tranches work
	roles.add(PoolRole::TrancheInvestor(1, UNION));
	roles.add(PoolRole::TrancheInvestor(2, UNION));
	assert!(roles.exists(PoolRole::TrancheInvestor(1, UNION)));
	assert!(roles.exists(PoolRole::TrancheInvestor(2, UNION)));

	// MaxTranches works
	roles.add(PoolRole::TrancheInvestor(32, 0));
	assert!(!roles.exists(PoolRole::TrancheInvestor(32, UNION)));

	// Adding roles works normally
	roles.add(PoolRole::LiquidityAdmin);
	roles.add(PoolRole::MemberListAdmin);
	assert!(roles.exists(PoolRole::LiquidityAdmin));
	assert!(roles.exists(PoolRole::MemberListAdmin));

	// Role exists for as long as permission is given
	roles.add(PoolRole::TrancheInvestor(8, 5));
	assert!(roles.exists(PoolRole::TrancheInvestor(8, UNION)));
	Now::pass(5);
	assert!(roles.exists(PoolRole::TrancheInvestor(8, UNION)));
	Now::pass(1);
	assert!(!roles.exists(PoolRole::TrancheInvestor(8, UNION)));
	Now::set(0);

	// Role is at least valid for MinDelay
	roles.add(PoolRole::TrancheInvestor(8, MinDelay::get() - 1));
	Now::pass(MinDelay::get());
	assert!(roles.exists(PoolRole::TrancheInvestor(8, UNION)));
	Now::set(0);

	// Removing roles work normally for Non-TrancheInvestor roles
	roles.rm(PoolRole::LiquidityAdmin);
	roles.rm(PoolRole::MemberListAdmin);
	assert!(!roles.exists(PoolRole::LiquidityAdmin));
	assert!(!roles.exists(PoolRole::MemberListAdmin));
}
