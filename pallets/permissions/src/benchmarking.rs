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

#[cfg(test)]
use crate::mock::{OrganisationRole, Role as MockRole, XcmRole};

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
		<T as pallet_permissions::Config>::Role: BenchRole + Clone,
		<T as pallet_permissions::Config>::Location: Default + Clone,
	}

	add_as_admin {
		let acc = admin::<T>(0);
		let with_role = T::Role::editor();
		let role = T::Role::editor();
		let pool_id: T::Location = Default::default();
	}:add(RawOrigin::Root, with_role.clone(), acc.clone(), pool_id.clone(), role.clone())
	verify {
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id, acc, role));
	}

	add_as_editor {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = T::Role::editor();
		let role = T::Role::editor();
		let pool_id: T::Location = Default::default();
		let res = PermissionsPallet::<T>::add(RawOrigin::Root.into(), with_role.clone(), acc.clone(), pool_id.clone(), role.clone());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id.clone(), acc.clone(), role.clone()));

		// setup borrower through pool admin
		let acc2 = admin::<T>(1);
		let role = T::Role::user();
	}:add(RawOrigin::Signed(acc.clone()), with_role.clone(), acc2.clone(), pool_id.clone(), role.clone())
	verify {
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id, acc2, role));
	}

	remove_as_admin {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = T::Role::editor();
		let role = T::Role::editor();
		let pool_id: T::Location = Default::default();
		let res = PermissionsPallet::<T>::add(RawOrigin::Root.into(), with_role.clone(), acc.clone(), pool_id.clone(), role.clone());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id.clone(), acc.clone(), role.clone()));
	}:remove(RawOrigin::Root, with_role.clone(), acc.clone(), pool_id.clone(), role.clone())
	verify {
		assert!(!<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id, acc, role));
	}

	remove_as_editor {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = T::Role::editor();
		let role = T::Role::editor();
		let pool_id: T::Location = Default::default();
		let res = PermissionsPallet::<T>::add(RawOrigin::Root.into(), with_role.clone(), acc.clone(), pool_id.clone(), role.clone());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id.clone(), acc.clone(), role.clone()));

		// setup borrower through pool admin
		let acc2 = admin::<T>(1);
		let role = T::Role::user();
		let res = PermissionsPallet::<T>::add(RawOrigin::Signed(acc.clone()).into(), with_role.clone(), acc2.clone(), pool_id.clone(), role.clone());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id.clone(), acc2.clone(), role.clone()));
	}:remove(RawOrigin::Signed(acc), with_role.clone(), acc2.clone(), pool_id.clone(), role.clone())
	verify {
		assert!(!<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id, acc2, role));
	}

	purge {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = T::Role::editor();
		let role = T::Role::editor();
		let pool_id: T::Location = Default::default();
		let res = PermissionsPallet::<T>::add(RawOrigin::Root.into(), with_role.clone(), acc.clone(), pool_id.clone(), role.clone());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id.clone(), acc.clone(), role.clone()));
	}:_(RawOrigin::Signed(acc.clone()), pool_id.clone())
	verify {
		assert!(!<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id, acc, role));
	}

	admin_purge {
		// setup pool admin
		let acc = admin::<T>(0);
		let with_role = T::Role::editor();
		let role = T::Role::editor();
		let pool_id: T::Location = Default::default();
		let res = PermissionsPallet::<T>::add(RawOrigin::Root.into(), with_role.clone(), acc.clone(), pool_id.clone(), role.clone());
		assert_ok!(res);
		assert!(<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id.clone(), acc.clone(), role.clone()));
	}:_(RawOrigin::Root, acc.clone(), pool_id.clone())
	verify {
		assert!(!<PermissionsPallet::<T> as TPermissions<T::AccountId>>::has(pool_id, acc, role));
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(|| {}),
	crate::mock::MockRuntime,
);

pub trait BenchRole {
	fn editor() -> Self;
	fn user() -> Self;
}

impl BenchRole for PoolRole {
	fn editor() -> Self {
		Self::PoolAdmin
	}

	fn user() -> Self {
		Self::Borrower
	}
}

#[cfg(test)]
impl BenchRole for MockRole {
	fn editor() -> Self {
		Self::Organisation(OrganisationRole::Admin)
	}

	fn user() -> Self {
		Self::Xcm(XcmRole::Sender)
	}
}
