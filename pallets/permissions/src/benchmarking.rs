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

use super::*;
use crate as pallet_permissions;
use crate::Pallet as PermissionsPallet;
use common_traits::Permissions as TPermissions;
use common_types::PoolRole;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::assert_ok;
use frame_system::RawOrigin;

fn whitelist_acc<T: frame_system::Config>(acc: &T::AccountId) {
	frame_benchmarking::benchmarking::add_to_whitelist(
		frame_system::Account::<T>::hashed_key_for(acc).into(),
	);
}

fn admin<T: frame_system::Config>(index: u32) -> T::AccountId {
	let admin = account::<T::AccountId>("admin", index, 0);
	whitelist_acc::<T>(&admin);
	admin
}

benchmarks! {
	where_clause {
		where
		<T as pallet_permissions::Config>::Role: From<PoolRole>,
		<T as pallet_permissions::Config>::Location: From<u64>,
	}

	add_permission_root {
		let acc = admin::<T>(0);
		let with_role = PoolRole::PoolAdmin;
		let role = PoolRole::PoolAdmin;
		let pool_id = 0;
	}:add_permission(RawOrigin::Root, with_role.into(), acc.clone(), pool_id.into(), role.into())
	verify {
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc, role.into()));
	}

	add_permission_editor {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = PoolRole::PoolAdmin;
		let role = PoolRole::PoolAdmin;
		let pool_id = 0;
		let res = PermissionsPallet::<T>::add_permission(RawOrigin::Root.into(), with_role.into(), acc.clone(), pool_id.into(), role.into());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc.clone(), role.into()));

		// setup borrower through pool admin
		let acc2 = admin::<T>(1);
		let role = PoolRole::Borrower;
	}:add_permission(RawOrigin::Signed(acc.clone()), with_role.into(), acc2.clone(), pool_id.into(), role.into())
	verify {
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc2, role.into()));
	}

	rm_permission_root {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = PoolRole::PoolAdmin;
		let role = PoolRole::PoolAdmin;
		let pool_id = 0;
		let res = PermissionsPallet::<T>::add_permission(RawOrigin::Root.into(), with_role.into(), acc.clone(), pool_id.into(), role.into());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc.clone(), role.into()));
	}:rm_permission(RawOrigin::Root, with_role.into(), acc.clone(), pool_id.into(), role.into())
	verify {
		assert!(!<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc, role.into()));
	}

	rm_permission_editor {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = PoolRole::PoolAdmin;
		let role = PoolRole::PoolAdmin;
		let pool_id = 0;
		let res = PermissionsPallet::<T>::add_permission(RawOrigin::Root.into(), with_role.into(), acc.clone(), pool_id.into(), role.into());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc.clone(), role.into()));

		// setup borrower through pool admin
		let acc2 = admin::<T>(1);
		let role = PoolRole::Borrower;
		let res = PermissionsPallet::<T>::add_permission(RawOrigin::Signed(acc.clone()).into(), with_role.into(), acc2.clone(), pool_id.into(), role.into());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc2.clone(), role.into()));
	}:rm_permission(RawOrigin::Signed(acc), with_role.into(), acc2.clone(), pool_id.into(), role.into())
	verify {
		assert!(!<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc2, role.into()));
	}

	purge_permissions {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = PoolRole::PoolAdmin;
		let role = PoolRole::PoolAdmin;
		let pool_id = 0;
		let res = PermissionsPallet::<T>::add_permission(RawOrigin::Root.into(), with_role.into(), acc.clone(), pool_id.into(), role.into());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc.clone(), role.into()));
	}:_(RawOrigin::Signed(acc.clone()), pool_id.into())
	verify {
		assert!(!<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc, role.into()));
	}

	admin_purge_permissions {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = PoolRole::PoolAdmin;
		let role = PoolRole::PoolAdmin;
		let pool_id = 0;
		let res = PermissionsPallet::<T>::add_permission(RawOrigin::Root.into(), with_role.into(), acc.clone(), pool_id.into(), role.into());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc.clone(), role.into()));
	}:_(RawOrigin::Root, acc.clone(), pool_id.into())
	verify {
		assert!(!<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has_permission(pool_id.into(), acc, role.into()));
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::MockRuntime,
);
