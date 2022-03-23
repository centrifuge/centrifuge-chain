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
use common_types::CurrencyId;
use common_types::PoolLocator;
use common_types::PoolRole;
use frame_support::sp_runtime::traits::One;
use frame_support::traits::fungibles::Transfer;
use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use frame_support::{assert_ok, parameter_types, StorageHasher, Twox128};
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

type PermissionsOf<T> = <T as pallet_loans::Config>::Permission;
pub(crate) fn set_role<T: pallet_loans::Config>(
	location: <T::Pool as common_traits::PoolInspect<T::AccountId>>::PoolId,
	who: T::AccountId,
	role: PoolRole,
) {
	PermissionsOf::<T>::add(location, who, role).expect("adding permissions should not fail");
}

fn create_tranche_id(pool: u64, tranche: u64) -> [u8; 16] {
	let hash_input = (tranche, pool).encode();
	Twox128::hash(&hash_input)
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
		+ pallet_loans::Config<ClassId = <T as pallet_uniques::Config>::ClassId>
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
	T: frame_system::Config + pallet_loans::Config,
{
	let loan_id: T::LoanId = 1u128.into();
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
	let pool_account = PoolLocator { pool_id }.into_account();

	set_role::<T>(
		pool_id.into(),
		junior_investor.clone(),
		PoolRole::TrancheInvestor(JuniorTrancheId::get().into(), 0u32.into()),
	);
	set_role::<T>(
		pool_id.into(),
		senior_investor.clone(),
		PoolRole::TrancheInvestor(SeniorTrancheId::get().into(), 0u32.into()),
	);

	// Initialize pool with initial investments
	assert_ok!(PoolPallet::<T>::create(
		RawOrigin::Signed(owner.clone()).into(),
		owner.clone(),
		pool_id,
		vec![
			(TrancheType::Residual, None),
			(
				TrancheType::NonResidual {
					interest_per_sec: One::one(),
					min_risk_buffer: Perquintill::from_percent(10),
				},
				None
			)
		],
		currency_id.into(),
		(100_000 * CURRENCY).into(),
	));

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
	<pallet_loans::Pallet<T> as PoolNAV<PoolIdOf<T>, T::Amount>>::update_nav(pool_id.into())
		.expect("update nav should work");

	assert_ok!(PoolPallet::<T>::close_epoch(
		RawOrigin::Signed(owner).into(),
		pool_id,
	));

	let pool = PoolStorage::<T>::get(pool_id).unwrap();
	assert_eq!(pool.reserve.available_reserve, (1000 * CURRENCY).into());

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
		+ pallet_loans::Config<ClassId = <T as pallet_uniques::Config>::ClassId>
		+ pallet_uniques::Config,
	<T as pallet_uniques::Config>::ClassId: From<u64>,
{
	let class_id = create_nft_class::<T>(class_id, pool_owner.clone(), maybe_admin);
	pallet_loans::Pallet::<T>::initialise_pool(
		RawOrigin::Signed(pool_owner).into(),
		pool_id,
		class_id,
	)
	.expect("initialisation of pool should not fail");
	let nav = pallet_loans::PoolNAV::<T>::get(pool_id).unwrap();
	assert!(nav.latest_nav == Zero::zero());
	class_id
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
