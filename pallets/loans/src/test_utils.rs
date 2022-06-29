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
use crate as pallet_loans;
use crate::{AssetOf, PoolIdOf};
use codec::Encode;
use common_traits::{Permissions, PoolNAV};
use common_types::{CurrencyId, Moment, PermissionScope, PoolLocator, PoolRole, Role};
use frame_support::sp_runtime::traits::One;
use frame_support::traits::fungibles::Transfer;
use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use frame_support::traits::{Currency, Get};
use frame_support::{assert_ok, parameter_types, Blake2_128, StorageHasher};
use frame_system::RawOrigin;
use pallet_pools::TrancheLoc;
use pallet_pools::TrancheType;
use pallet_pools::{Pallet as PoolPallet, Pool as PoolStorage};
use runtime_common::CFG as CURRENCY;
use sp_runtime::{
	traits::{AccountIdConversion, Zero},
	Perquintill,
};
use sp_std::vec;

type TrancheId = [u8; 16];
type PermissionsOf<T> = <T as pallet_loans::Config>::Permission;

pub(crate) fn set_role<T: pallet_loans::Config>(
	scope: PermissionScope<
		<T::Pool as common_traits::PoolInspect<T::AccountId>>::PoolId,
		<T as pallet_loans::Config>::CurrencyId,
	>,
	who: T::AccountId,
	role: Role<TrancheId, Moment>,
) {
	PermissionsOf::<T>::add(scope, who, role).expect("adding permissions should not fail");
}

fn create_tranche_id(pool: u64, tranche: u64) -> [u8; 16] {
	let hash_input = (tranche, pool).encode();
	Blake2_128::hash(&hash_input)
}

parameter_types! {
	pub JuniorTrancheId: [u8; 16] = create_tranche_id(0, 0);
	pub SeniorTrancheId: [u8; 16] = create_tranche_id(0, 1);
}

pub(crate) fn create_nft_class<T>(
	class_id: u64,
	owner: T::AccountId,
	maybe_admin: Option<T::AccountId>,
) -> <T as pallet_loans::Config>::ClassId
where
	T: frame_system::Config
		+ pallet_loans::Config<ClassId = <T as pallet_uniques::Config>::CollectionId>
		+ pallet_uniques::Config,
	<T as pallet_uniques::Config>::CollectionId: From<u64>,
{
	// Create class. Shouldn't fail.
	let admin = maybe_admin.unwrap_or(owner.clone());
	let uniques_class_id: <T as pallet_uniques::Config>::CollectionId = class_id.into();
	<pallet_uniques::Pallet<T> as Create<T::AccountId>>::create_collection(
		&uniques_class_id,
		&owner,
		&admin,
	)
	.expect("class creation should not fail");
	uniques_class_id
}

#[cfg(feature = "runtime-benchmarks")]
pub(crate) fn create_nft_class_if_needed<T>(
	class_id: u64,
	owner: T::AccountId,
	maybe_admin: Option<T::AccountId>,
) -> <T as pallet_loans::Config>::ClassId
where
	T: frame_system::Config
		+ pallet_loans::Config<ClassId = <T as pallet_uniques::Config>::CollectionId>
		+ pallet_uniques::Config,
	<T as pallet_uniques::Config>::CollectionId: From<u64>,
{
	if pallet_uniques::Pallet::<T>::collection_owner(class_id.into()).is_none() {
		create_nft_class::<T>(class_id, owner, maybe_admin)
	} else {
		class_id.into()
	}
}

#[cfg(test)]
pub(crate) fn mint_nft<T>(owner: T::AccountId, class_id: T::ClassId) -> T::LoanId
where
	T: frame_system::Config + pallet_loans::Config,
{
	mint_nft_of::<T>(owner, class_id, 1.into())
}

pub(crate) fn mint_nft_of<T>(
	owner: T::AccountId,
	class_id: T::ClassId,
	loan_id: T::LoanId,
) -> T::LoanId
where
	T: frame_system::Config + pallet_loans::Config,
{
	T::NonFungible::mint_into(&class_id.into(), &loan_id.into(), &owner)
		.expect("mint should not fail");
	loan_id
}

pub(crate) fn create<T>(
	pool_id: T::PoolId,
	owner: T::AccountId,
	junior_investor: T::AccountId,
	senior_investor: T::AccountId,
	currency_id: CurrencyId,
) where
	T: pallet_pools::Config + frame_system::Config + pallet_loans::Config,
	<T as pallet_pools::Config>::Balance: From<u128>,
	<T as pallet_pools::Config>::CurrencyId: From<CurrencyId>,
	<T as pallet_pools::Config>::EpochId: From<u32>,
	<T as pallet_pools::Config>::PoolId: Into<u64> + Into<PoolIdOf<T>>,
{
	let pool_account = PoolLocator { pool_id }.into_account_truncating();

	let mint_amount = <T as pallet_pools::Config>::PoolDeposit::get() * 2.into();
	<T as pallet_pools::Config>::Currency::deposit_creating(&owner.clone().into(), mint_amount);

	// Initialize pool with initial investments
	assert_ok!(PoolPallet::<T>::create(
		RawOrigin::Signed(owner.clone()).into(),
		owner.clone(),
		pool_id,
		vec![
			(TrancheType::Residual, None),
			(
				TrancheType::NonResidual {
					interest_rate_per_sec: One::one(),
					min_risk_buffer: Perquintill::from_percent(10),
				},
				None
			)
		],
		currency_id.into(),
		(100_000 * CURRENCY).into(),
	));

	set_role::<T>(
		PermissionScope::Pool(pool_id.into()),
		junior_investor.clone(),
		Role::PoolRole(PoolRole::TrancheInvestor(
			JuniorTrancheId::get().into(),
			999_999_999u32.into(),
		)),
	);
	set_role::<T>(
		PermissionScope::Pool(pool_id.into()),
		senior_investor.clone(),
		Role::PoolRole(PoolRole::TrancheInvestor(
			SeniorTrancheId::get().into(),
			999_999_999u32.into(),
		)),
	);

	assert_ok!(PoolPallet::<T>::update_invest_order(
		RawOrigin::Signed(junior_investor.clone()).into(),
		pool_id,
		TrancheLoc::Id(JuniorTrancheId::get().into()),
		(500 * CURRENCY).into(),
	));
	assert_ok!(PoolPallet::<T>::update_invest_order(
		RawOrigin::Signed(senior_investor.clone()).into(),
		pool_id,
		TrancheLoc::Id(SeniorTrancheId::get().into()),
		(500 * CURRENCY).into(),
	));
	<pallet_loans::Pallet<T> as PoolNAV<PoolIdOf<T>, <T as pallet_loans::Config>::Balance>>::update_nav(
		pool_id.into(),
	)
	.expect("update nav should work");

	pallet_pools::Pool::<T>::try_mutate(pool_id, |pool| -> Result<(), pallet_pools::Error<T>> {
		let pool = pool.as_mut().ok_or(pallet_pools::Error::<T>::NoSuchPool)?;
		pool.parameters.min_epoch_time = 0;
		pool.parameters.max_nav_age = 999_999_999_999;
		Ok(())
	})
	.expect("Could not fixup pool parameters");

	assert_ok!(PoolPallet::<T>::close_epoch(
		RawOrigin::Signed(owner).into(),
		pool_id,
	));

	let pool = PoolStorage::<T>::get(pool_id).unwrap();
	assert_eq!(pool.reserve.available, (1000 * CURRENCY).into());

	// TODO(ved) do disbursal manually for now
	assert_ok!(<T as pallet_pools::Config>::Tokens::transfer(
		CurrencyId::Tranche(pool_id.into(), JuniorTrancheId::get()).into(),
		&pool_account,
		&junior_investor,
		(500 * CURRENCY).into(),
		false
	));
	assert_ok!(<T as pallet_pools::Config>::Tokens::transfer(
		CurrencyId::Tranche(pool_id.into(), SeniorTrancheId::get()).into(),
		&pool_account,
		&senior_investor,
		(500 * CURRENCY).into(),
		false
	));
}

pub(crate) fn initialise_test_pool<T>(
	pool_id: PoolIdOf<T>,
	class_id: u64,
	pool_owner: T::AccountId,
	maybe_admin: Option<T::AccountId>,
) -> <T as pallet_loans::Config>::ClassId
where
	T: frame_system::Config
		+ pallet_loans::Config<ClassId = <T as pallet_uniques::Config>::CollectionId>
		+ pallet_uniques::Config,
	<T as pallet_uniques::Config>::CollectionId: From<u64>,
{
	let class_id = create_nft_class::<T>(class_id, pool_owner.clone(), maybe_admin);
	pallet_loans::Pallet::<T>::initialise_pool(
		RawOrigin::Signed(pool_owner).into(),
		pool_id,
		class_id,
	)
	.expect("initialisation of pool should not fail");
	let nav = pallet_loans::PoolNAV::<T>::get(pool_id).unwrap();
	assert!(nav.latest == Zero::zero());
	class_id
}

/// Only used for runtime benchmarks at the moment
#[cfg(feature = "runtime-benchmarks")]
pub(crate) fn get_tranche_id<T>(
	pool_id: <T as pallet_pools::Config>::PoolId,
	index: u64,
) -> <T as pallet_pools::Config>::TrancheId
where
	T: pallet_pools::Config,
{
	pallet_pools::Pool::<T>::get(pool_id)
		.unwrap()
		.tranches
		.tranche_id(TrancheLoc::Index(index))
		.unwrap()
}

pub(crate) fn assert_last_event<T, E>(generic_event: E)
where
	T: pallet_loans::Config + pallet_pools::Config,
	E: Into<<T as frame_system::Config>::Event>,
{
	let events = frame_system::Pallet::<T>::events();
	let system_event = generic_event.into();
	// compare to the last event record
	let frame_system::EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

pub(crate) fn expect_asset_owner<T: frame_system::Config + pallet_loans::Config>(
	asset: AssetOf<T>,
	owner: T::AccountId,
) {
	let (class_id, instance_id) = asset.destruct();
	assert_eq!(
		<T as pallet_loans::Config>::NonFungible::owner(&class_id.into(), &instance_id.into())
			.unwrap(),
		owner
	);
}

pub(crate) fn expect_asset_to_be_burned<T: frame_system::Config + pallet_loans::Config>(
	asset: AssetOf<T>,
) {
	let (class_id, instance_id) = asset.destruct();
	assert_eq!(
		<T as pallet_loans::Config>::NonFungible::owner(&class_id.into(), &instance_id.into()),
		None
	);
}
