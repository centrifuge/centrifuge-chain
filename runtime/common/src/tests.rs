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
use common_traits::PoolRole;
use frame_support::parameter_types;
use pallet_permissions::Properties;

parameter_types! {
	pub const MaxTranches: TrancheId = 32;
	pub const MaxHold: Moment = 8;
	pub const MinDelay: Moment = 4;
}

#[test]
fn permission_roles_work() {
	assert!(PermissionRoles::<MaxTranches, MaxHold, MinDelay>::default().empty());

	let mut roles = PermissionRoles::<MaxTranches, MaxHold, MinDelay>::default();

	// Test zero-tranche handling
	assert!(!roles.exists(PoolRole::TrancheInvestor(0, 0)));
	roles.add(PoolRole::TrancheInvestor(0, 0));
	assert!(roles.exists(PoolRole::TrancheInvestor(0, 0)));

	// Removing before MinDelay fails
	roles.rm(PoolRole::TrancheInvestor(0, 0));
	assert!(roles.exists(PoolRole::TrancheInvestor(0, 2)));
	roles.rm(PoolRole::TrancheInvestor(0, 3));
	assert!(roles.exists(PoolRole::TrancheInvestor(0, 4)));

	// Removing after MinDelay works
	roles.rm(PoolRole::TrancheInvestor(0, 5));
	assert!(!roles.exists(PoolRole::TrancheInvestor(0, 5)));

	// Multiple tranches work
	roles.add(PoolRole::TrancheInvestor(1, 0));
	roles.add(PoolRole::TrancheInvestor(2, 0));
	assert!(roles.exists(PoolRole::TrancheInvestor(1, 3)));
	assert!(roles.exists(PoolRole::TrancheInvestor(2, 3)));

	// MaxTranches works
	roles.add(PoolRole::TrancheInvestor(32, 0));
	assert!(!roles.exists(PoolRole::TrancheInvestor(32, 4)));

	// Adding roles works normally
	roles.add(PoolRole::LiquidityAdmin);
	assert!(roles.exists(PoolRole::LiquidityAdmin));

	// Role exists for as long as MaxHold is defined
	roles.add(PoolRole::TrancheInvestor(8, 0));
	assert!(roles.exists(PoolRole::TrancheInvestor(8, 4)));
	assert!(roles.exists(PoolRole::TrancheInvestor(8, 8)));
	assert!(!roles.exists(PoolRole::TrancheInvestor(8, 9)));

	// Removing roles work normally
	roles.rm(PoolRole::LiquidityAdmin);
	assert!(!roles.exists(PoolRole::LiquidityAdmin));
	assert!(roles.exists(PoolRole::TrancheInvestor(8, 1)));
	assert!(roles.exists(PoolRole::TrancheInvestor(1, 1)));
	assert!(roles.exists(PoolRole::TrancheInvestor(2, 1)));

	roles.rm(PoolRole::TrancheInvestor(8, 5));
	assert!(!roles.exists(PoolRole::LiquidityAdmin));
	assert!(!roles.exists(PoolRole::TrancheInvestor(8, 2)));
}
