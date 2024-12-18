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

use cfg_traits::UpdateState;
use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::assert_ok;

use crate::{mock::*, Event};

const POOL_ADMIN: AccountId = 1;
const POOL_A: PoolId = 1;
const POOL_CURRENCY: CurrencyId = 42;
const MAX_RESERVE: Balance = 100;
const METADATA: &[u8] = b"test metadata";

pub fn assert_event_with_pos(event: Event<Runtime>) -> usize {
	System::events()
		.iter()
		.position(|record| record.event == event.clone().into())
		.unwrap()
}

#[test]
fn register_pool_with_metadata() {
	System::externalities().execute_with(|| {
		PoolSystem::mock_create(|admin, depositor, _, _, _, _, _| {
			assert_eq!(admin, POOL_ADMIN);
			assert_eq!(depositor, POOL_ADMIN);
			Ok(())
		});
		WriteOffPolicy::mock_update(|_, _| Ok(()));

		assert_ok!(PoolRegistry::register(
			RuntimeOrigin::root(),
			POOL_ADMIN,
			POOL_A,
			Default::default(),
			POOL_CURRENCY,
			MAX_RESERVE,
			Some(METADATA.to_vec().try_into().unwrap()),
			(), // policy
			Default::default(),
		));

		assert_eq!(
			PoolRegistry::get_pool_metadata(POOL_A)
				.unwrap()
				.into_inner(),
			METADATA
		);

		let register_pos = assert_event_with_pos(Event::<Runtime>::Registered { pool_id: POOL_A });
		let metadata_pos = assert_event_with_pos(Event::<Runtime>::MetadataSet {
			pool_id: POOL_A,
			metadata: METADATA.to_vec().try_into().unwrap(),
		});

		assert!(register_pos < metadata_pos)
	});
}

#[test]
fn update_pool() {
	System::externalities().execute_with(|| {
		Permissions::mock_has(|scope, who, role| {
			assert!(matches!(scope, PermissionScope::Pool(POOL_A)));
			assert_eq!(who, POOL_ADMIN);
			assert!(matches!(role, Role::PoolRole(PoolRole::PoolAdmin)));
			true
		});
		PoolSystem::mock_update(|_, _| Ok(UpdateState::NoExecution));

		assert_ok!(PoolRegistry::update(
			RuntimeOrigin::signed(POOL_ADMIN),
			POOL_A,
			1,
		));

		System::assert_last_event(Event::<Runtime>::UpdateRegistered { pool_id: POOL_A }.into());
	});
}

#[test]
fn set_metadata() {
	System::externalities().execute_with(|| {
		Permissions::mock_has(|scope, who, role| {
			assert!(matches!(scope, PermissionScope::Pool(POOL_A)));
			assert_eq!(who, POOL_ADMIN);
			assert!(matches!(role, Role::PoolRole(PoolRole::PoolAdmin)));
			true
		});

		assert_ok!(PoolRegistry::set_metadata(
			RuntimeOrigin::signed(POOL_ADMIN),
			POOL_A,
			METADATA.to_vec().try_into().unwrap(),
		));

		assert_eq!(
			PoolRegistry::get_pool_metadata(POOL_A)
				.unwrap()
				.into_inner(),
			METADATA
		);

		System::assert_last_event(
			Event::<Runtime>::MetadataSet {
				pool_id: POOL_A,
				metadata: METADATA.to_vec().try_into().unwrap(),
			}
			.into(),
		);
	});
}
