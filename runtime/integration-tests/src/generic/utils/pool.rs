// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

use cfg_primitives::{AccountId, Balance, PoolId};
use cfg_traits::Seconds;
use cfg_types::{
	fixed_point::Rate,
	permissions::{PermissionScope, PoolRole, Role},
	pools::TrancheMetadata,
	tokens::CurrencyId,
};
use frame_support::{dispatch::RawOrigin, BoundedVec};
use pallet_pool_system::tranches::{TrancheInput, TrancheType};
use sp_runtime::{
	traits::{CheckedAdd, One},
	FixedPointNumber, FixedPointOperand, Perquintill,
};

use crate::generic::config::{Runtime, RuntimeKind};

pub const POOL_MIN_EPOCH_TIME: Seconds = 24;

pub fn give_role<T: Runtime>(dest: AccountId, pool_id: PoolId, role: PoolRole) {
	pallet_permissions::Pallet::<T>::add(
		RawOrigin::Root.into(),
		Role::PoolRole(role),
		dest,
		PermissionScope::Pool(pool_id),
		Role::PoolRole(role),
	)
	.unwrap();
}

pub fn create_empty<T: Runtime>(admin: AccountId, pool_id: PoolId, currency_id: CurrencyId) {
	create::<T>(
		admin,
		pool_id,
		currency_id,
		[(Rate::one(), Perquintill::zero())],
	)
}

pub fn interest_rate<P: FixedPointOperand>(percent: P) -> Rate {
	Rate::one()
		.checked_add(&Rate::checked_from_rational(percent, 100).unwrap())
		.unwrap()
}

pub fn create_two_tranched<T: Runtime>(admin: AccountId, pool_id: PoolId, currency_id: CurrencyId) {
	create::<T>(
		admin,
		pool_id,
		currency_id,
		[(interest_rate(5), Perquintill::zero())],
	)
}

pub fn create_one_tranched<T: Runtime>(admin: AccountId, pool_id: PoolId, currency_id: CurrencyId) {
	create::<T>(admin, pool_id, currency_id, [])
}

pub fn create<T: Runtime>(
	admin: AccountId,
	pool_id: PoolId,
	currency_id: CurrencyId,
	non_residual_tranches: impl IntoIterator<Item = (Rate, Perquintill)>,
) {
	let mut tranches = vec![TrancheInput::<Rate, _, _> {
		tranche_type: TrancheType::Residual,
		seniority: None,
		metadata: TrancheMetadata {
			token_name: BoundedVec::default(),
			token_symbol: BoundedVec::default(),
		},
	}];

	tranches.extend(non_residual_tranches.into_iter().map(
		|(interest_rate_per_sec, min_risk_buffer)| TrancheInput {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec,
				min_risk_buffer,
			},
			seniority: None,
			metadata: TrancheMetadata {
				token_name: BoundedVec::default(),
				token_symbol: BoundedVec::default(),
			},
		},
	));

	pallet_pool_registry::Pallet::<T>::register(
		match T::KIND {
			RuntimeKind::Development => RawOrigin::Signed(admin.clone()).into(),
			_ => RawOrigin::Root.into(),
		},
		admin,
		pool_id,
		tranches,
		currency_id,
		Balance::MAX,
		None,
		BoundedVec::default(),
		vec![],
	)
	.unwrap();

	// In order to later close the epoch fastly,
	// we mofify here that requirement to significalty reduce the testing time.
	// The only way to do it is breaking the integration tests rules mutating
	// this state directly.
	pallet_pool_system::Pool::<T>::mutate(pool_id, |pool| {
		pool.as_mut().unwrap().parameters.min_epoch_time = POOL_MIN_EPOCH_TIME;
	});
}

pub fn close_epoch<T: Runtime>(admin: AccountId, pool_id: PoolId) {
	pallet_pool_system::Pallet::<T>::close_epoch(RawOrigin::Signed(admin.clone()).into(), pool_id)
		.unwrap();
}
