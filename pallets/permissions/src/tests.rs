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

///! Tests for the permissions pallet
use crate as pallet_permissions;
use crate::{mock::*, Error as PermissionsError};

use frame_support::{assert_noop, assert_ok};
use pallet_permissions::{Permissions, Properties};

#[test]
fn add_ext_works() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Organisation(OrganisationRole::SeniorExeutive)
			));

			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Xcm(XcmRole::Sender)
			));

			let roles =
				pallet_permissions::Permission::<MockRuntime>::get(2, Location::PalletA).unwrap();

			assert!(roles.exists(Role::Organisation(OrganisationRole::HeadOfSaubermaching)));
			assert!(roles.exists(Role::Organisation(OrganisationRole::SeniorExeutive)));
			assert!(roles.exists(Role::Xcm(XcmRole::Sender)));
			assert!(!roles.exists(Role::Xcm(XcmRole::Receiver)));

			assert!(
				pallet_permissions::Permission::<MockRuntime>::get(2, Location::PalletB).is_none()
			);
		})
}

#[test]
fn add_ext_fails() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert_noop!(
				pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(1),
					Role::Organisation(OrganisationRole::HeadOfSaubermaching),
					2,
					Location::PalletA,
					Role::Organisation(OrganisationRole::HeadOfSaubermaching)
				),
				PermissionsError::<MockRuntime>::RoleAlreadyGiven
			);
		})
}

#[test]
fn remove_ext_works() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Xcm(XcmRole::Sender)
			));

			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::remove(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Xcm(XcmRole::Sender)
			));

			assert!(
				pallet_permissions::Permission::<MockRuntime>::get(2, Location::PalletA).is_none()
			);
		})
}

#[test]
fn remove_ext_fails() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_noop!(
				pallet_permissions::Pallet::<MockRuntime>::remove(
					Origin::signed(1),
					Role::Organisation(OrganisationRole::HeadOfSaubermaching),
					2,
					Location::PalletA,
					Role::Organisation(OrganisationRole::HeadOfSaubermaching)
				),
				PermissionsError::<MockRuntime>::NoRoles
			);

			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Organisation(OrganisationRole::SeniorExeutive)
			));

			assert_noop!(
				pallet_permissions::Pallet::<MockRuntime>::remove(
					Origin::signed(1),
					Role::Organisation(OrganisationRole::HeadOfSaubermaching),
					2,
					Location::PalletA,
					Role::Organisation(OrganisationRole::HeadOfSaubermaching)
				),
				PermissionsError::<MockRuntime>::RoleNotGiven
			);

			assert!(
				pallet_permissions::Permission::<MockRuntime>::get(2, Location::PalletA)
					.unwrap()
					.exists(Role::Organisation(OrganisationRole::SeniorExeutive))
			);
		})
}

#[test]
fn user_purge_permission_ext_works() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Organisation(OrganisationRole::SeniorExeutive)
			));

			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::purge(
				Origin::signed(2),
				Location::PalletA
			));

			assert!(
				pallet_permissions::Permission::<MockRuntime>::get(2, Location::PalletA).is_none()
			);
		})
}

#[test]
fn user_purge_permission_ext_fails() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_noop!(
				pallet_permissions::Pallet::<MockRuntime>::purge(
					Origin::signed(2),
					Location::PalletA
				),
				PermissionsError::<MockRuntime>::NoRoles
			);

			assert!(
				pallet_permissions::Permission::<MockRuntime>::get(2, Location::PalletA).is_none()
			);
		})
}

#[test]
fn admin_purge_permission_ext_works() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(1),
				Role::Organisation(OrganisationRole::HeadOfSaubermaching),
				2,
				Location::PalletA,
				Role::Organisation(OrganisationRole::SeniorExeutive)
			));

			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::admin_purge(
				Origin::signed(1),
				2,
				Location::PalletA,
			));

			assert!(
				pallet_permissions::Permission::<MockRuntime>::get(2, Location::PalletA,).is_none()
			);
		})
}

#[test]
fn admin_purge_permission_ext_fails() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_noop!(
				pallet_permissions::Pallet::<MockRuntime>::admin_purge(
					Origin::signed(1),
					2,
					Location::PalletA,
				),
				PermissionsError::<MockRuntime>::NoRoles
			);

			assert!(
				pallet_permissions::Permission::<MockRuntime>::get(2, Location::PalletA,).is_none()
			);
		})
}

#[test]
fn trait_add_fails() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::add(
				Location::PalletA,
				2,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert_noop!(
				<pallet_permissions::Pallet<MockRuntime> as Permissions<AccountId>>::add(
					Location::PalletA,
					2,
					Role::Organisation(OrganisationRole::HeadOfSaubermaching)
				),
				PermissionsError::<MockRuntime>::RoleAlreadyGiven
			);
		})
}

#[test]
fn trait_add_works() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(pallet_dummy::Pallet::<MockRuntime>::test_add(
				Origin::signed(2),
				Location::PalletA,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert_noop!(
				<pallet_permissions::Pallet<MockRuntime> as Permissions<AccountId>>::add(
					Location::PalletA,
					2,
					Role::Organisation(OrganisationRole::HeadOfSaubermaching)
				),
				PermissionsError::<MockRuntime>::RoleAlreadyGiven
			);
		})
}

#[test]
fn trait_remove_fails() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_noop!(
				pallet_dummy::Pallet::<MockRuntime>::test_rm(
					Origin::signed(2),
					Location::PalletA,
					Role::Organisation(OrganisationRole::HeadOfSaubermaching)
				),
				pallet_dummy::Error::<MockRuntime>::NotCleared
			);
		})
}

#[test]
fn trait_remove_works() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::add(
				Location::PalletA,
				2,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert_ok!(pallet_dummy::Pallet::<MockRuntime>::test_rm(
				Origin::signed(2),
				Location::PalletA,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));
		})
}

#[test]
fn trait_has_permission_works() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert_ok!(<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::add(
				Location::PalletA,
				2,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert!(<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::has(
				Location::PalletA,
				2,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));

			assert!(!<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::has(
				Location::PalletA,
				2,
				Role::Organisation(OrganisationRole::SeniorExeutive)
			));
		})
}

#[test]
fn add_too_many_permissions_fails() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			for who in 0..MaxRoles::get() {
				assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(1),
					Role::Organisation(OrganisationRole::HeadOfSaubermaching),
					who.into(),
					Location::PalletA,
					Role::Organisation(OrganisationRole::SeniorExeutive)
				));
			}
			let who = MaxRoles::get() + 1;
			assert_noop!(
				pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(1),
					Role::Organisation(OrganisationRole::HeadOfSaubermaching),
					who.into(),
					Location::PalletA,
					Role::Organisation(OrganisationRole::SeniorExeutive)
				),
				PermissionsError::<MockRuntime>::TooManyRoles
			);
		})
}

#[test]
fn permission_counting() {
	TestExternalitiesBuilder::default()
		.build(|| {})
		.execute_with(|| {
			assert!(
				pallet_permissions::PermissionCount::<MockRuntime>::get(Location::PalletA,)
					.is_none()
			);

			assert_ok!(<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::add(
				Location::PalletA,
				2,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));
			assert_eq!(
				pallet_permissions::PermissionCount::<MockRuntime>::get(Location::PalletA,),
				Some(1)
			);

			assert_ok!(<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::add(
				Location::PalletA,
				3,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));
			assert_eq!(
				pallet_permissions::PermissionCount::<MockRuntime>::get(Location::PalletA,),
				Some(2)
			);

			assert_ok!(<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::remove(
				Location::PalletA,
				3,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));
			assert_eq!(
				pallet_permissions::PermissionCount::<MockRuntime>::get(Location::PalletA,),
				Some(1)
			);
			assert_ok!(<pallet_permissions::Pallet<MockRuntime> as Permissions<
				AccountId,
			>>::remove(
				Location::PalletA,
				2,
				Role::Organisation(OrganisationRole::HeadOfSaubermaching)
			));
			assert!(
				pallet_permissions::PermissionCount::<MockRuntime>::get(Location::PalletA,)
					.is_none(),
			);
		})
}
