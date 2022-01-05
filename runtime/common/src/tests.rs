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
use pallet_permissions::Properties;

#[test]
fn permission_roles_work() {
	assert!(PermissionRoles::default().empty());

	let mut roles = PermissionRoles::default();

	roles.add(PoolRole::TrancheInvestor(1));
	assert!(roles.exists(PoolRole::TrancheInvestor(1)));
	assert!(!roles.exists(PoolRole::TrancheInvestor(2)));

	roles.add(PoolRole::TrancheInvestor(9));
	assert!(!roles.exists(PoolRole::TrancheInvestor(9)));

	roles.add(PoolRole::TrancheInvestor(200));
	assert!(!roles.exists(PoolRole::TrancheInvestor(200)));

	roles.add(PoolRole::LiquidityAdmin);
	assert!(roles.exists(PoolRole::LiquidityAdmin));

	roles.add(PoolRole::TrancheInvestor(8));
	assert!(roles.exists(PoolRole::TrancheInvestor(8)));

	roles.rm(PoolRole::LiquidityAdmin);
	assert!(!roles.exists(PoolRole::LiquidityAdmin));
	assert!(roles.exists(PoolRole::TrancheInvestor(8)));

	roles.rm(PoolRole::TrancheInvestor(8));
	assert!(!roles.exists(PoolRole::LiquidityAdmin));
	assert!(!roles.exists(PoolRole::TrancheInvestor(8)));
}
