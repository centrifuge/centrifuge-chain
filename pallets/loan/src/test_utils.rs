// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Module provides testing utilities for benchmarking and tests.
use crate as pallet_loan;
use crate::{AssetOf, PoolIdOf};
use common_traits::{PoolNAV, PoolRole};
use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use frame_support::{assert_ok, parameter_types};
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;
use pallet_permissions::Permissions;
use pallet_tinlake_investor_pool::PoolLocator;
use pallet_tinlake_investor_pool::{Pallet as PoolPallet, Pool as PoolStorage};
use primitives_tokens::CurrencyId;
use runtime_common::CFG as CURRENCY;
use sp_runtime::traits::AccountIdConversion;
use sp_std::vec;

type PermissionsOf<T> = <T as pallet_tinlake_investor_pool::Config>::Permission;
pub(crate) fn set_role<T: pallet_tinlake_investor_pool::Config>(
	location: T::PoolId,
	who: T::AccountId,
	role: common_traits::PoolRole<T::TrancheId>,
) {
	PermissionsOf::<T>::add_permission(location, who, role)
		.expect("adding permissions should not fail");
}

parameter_types! {
	pub const SeniorTrancheId: u8 = 0;
	pub const JuniorTrancheId: u8 = 1;
}

pub(crate) fn create_nft_class<T>(
	class_id: u64,
	owner: T::AccountId,
	maybe_admin: Option<T::AccountId>,
) -> <T as pallet_loan::Config>::ClassId
where
	T: frame_system::Config
		+ pallet_loan::Config<ClassId = <T as pallet_uniques::Config>::ClassId>
		+ pallet_uniques::Config,
	<T as pallet_uniques::Config>::ClassId: From<u64>,
{
	// Create class. Shouldn't fail.
	let admin = maybe_admin.unwrap_or(owner.clone());
	let uniques_class_id: <T as pallet_uniques::Config>::ClassId = class_id.into();
	<pallet_uniques::Pallet<T> as Create<T::AccountId>>::create_class(
		&uniques_class_id,
		&owner,
		&admin,
	)
	.expect("class creation should not fail");
	uniques_class_id
}

pub(crate) fn mint_nft<T>(owner: T::AccountId, class_id: T::ClassId) -> T::LoanId
where
	T: frame_system::Config + pallet_loan::Config,
{
	let loan_id: T::LoanId = 1u128.into();
	T::NonFungible::mint_into(&class_id.into(), &loan_id.into(), &owner)
		.expect("mint should not fail");
	loan_id
}

pub(crate) fn create_pool<T>(
	pool_id: T::PoolId,
	owner: T::AccountId,
	junior_investor: T::AccountId,
	senior_investor: T::AccountId,
	currency_id: CurrencyId,
) where
	T: pallet_tinlake_investor_pool::Config + frame_system::Config + pallet_loan::Config,
	<T as pallet_tinlake_investor_pool::Config>::Balance: From<u128>,
	<T as pallet_tinlake_investor_pool::Config>::CurrencyId: From<CurrencyId>,
	<T as pallet_tinlake_investor_pool::Config>::TrancheId: From<u8>,
	<T as pallet_tinlake_investor_pool::Config>::EpochId: From<u32>,
	<T as pallet_tinlake_investor_pool::Config>::PoolId: Into<u64> + Into<PoolIdOf<T>>,
{
	let pool_account = PoolLocator { pool_id }.into_account();

	set_role::<T>(pool_id, owner.clone(), PoolRole::PoolAdmin);
	set_role::<T>(
		pool_id,
		junior_investor.clone(),
		PoolRole::TrancheInvestor(1.into()),
	);
	set_role::<T>(
		pool_id,
		senior_investor.clone(),
		PoolRole::TrancheInvestor(0.into()),
	);

	// Initialize pool with initial investments
	assert_ok!(PoolPallet::<T>::create_pool(
		RawOrigin::Signed(owner.clone()).into(),
		pool_id,
		vec![(10, 10), (0, 0)],
		currency_id.into(),
		(100_000 * CURRENCY).into(),
	));

	assert_ok!(PoolPallet::<T>::order_supply(
		RawOrigin::Signed(junior_investor.clone()).into(),
		pool_id,
		JuniorTrancheId::get().into(),
		(500 * CURRENCY).into(),
	));
	assert_ok!(PoolPallet::<T>::order_supply(
		RawOrigin::Signed(senior_investor.clone()).into(),
		pool_id,
		SeniorTrancheId::get().into(),
		(500 * CURRENCY).into(),
	));
	<pallet_loan::Pallet<T> as PoolNAV<PoolIdOf<T>, T::Amount>>::update_nav(pool_id.into())
		.expect("update nav should work");

	assert_ok!(PoolPallet::<T>::close_epoch(
		RawOrigin::Signed(owner).into(),
		pool_id,
	));

	let pool = PoolStorage::<T>::get(pool_id).unwrap();
	assert_eq!(pool.available_reserve, (1000 * CURRENCY).into());

	// TODO(ved) do disbursal manually for now
	assert_ok!(
		<T as pallet_tinlake_investor_pool::Config>::Tokens::transfer(
			CurrencyId::Tranche(pool_id.into(), 1).into(),
			&pool_account,
			&junior_investor,
			(500 * CURRENCY).into(),
		)
	);
	assert_ok!(
		<T as pallet_tinlake_investor_pool::Config>::Tokens::transfer(
			CurrencyId::Tranche(pool_id.into(), 0).into(),
			&pool_account,
			&senior_investor,
			(500 * CURRENCY).into(),
		)
	);
}

pub(crate) fn initialise_test_pool<T>(
	pool_id: PoolIdOf<T>,
	class_id: u64,
	pool_owner: T::AccountId,
	maybe_admin: Option<T::AccountId>,
) -> <T as pallet_loan::Config>::ClassId
where
	T: frame_system::Config
		+ pallet_loan::Config<ClassId = <T as pallet_uniques::Config>::ClassId>
		+ pallet_uniques::Config,
	<T as pallet_uniques::Config>::ClassId: From<u64>,
{
	let class_id = create_nft_class::<T>(class_id, pool_owner.clone(), maybe_admin);
	pallet_loan::Pallet::<T>::initialise_pool(
		RawOrigin::Signed(pool_owner).into(),
		pool_id,
		class_id,
	)
	.expect("initialisation of pool should not fail");
	class_id
}

pub(crate) fn assert_last_event<T, E>(generic_event: E)
where
	T: pallet_loan::Config + pallet_tinlake_investor_pool::Config,
	E: Into<<T as frame_system::Config>::Event>,
{
	let events = frame_system::Pallet::<T>::events();
	let system_event = generic_event.into();
	// compare to the last event record
	let frame_system::EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

pub(crate) fn expect_asset_owner<T: frame_system::Config + pallet_loan::Config>(
	asset: AssetOf<T>,
	owner: T::AccountId,
) {
	let (class_id, instance_id) = asset.destruct();
	assert_eq!(
		<T as pallet_loan::Config>::NonFungible::owner(&class_id.into(), &instance_id.into())
			.unwrap(),
		owner
	);
}
