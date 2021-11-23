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
use frame_support::pallet_prelude::Get;
use frame_support::parameter_types;
use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use runtime_common::CurrencyId;

parameter_types! {
	pub const GetUSDCurrencyId: CurrencyId = 1;
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

pub(crate) fn create_pool<T, GetCurrencyId>(owner: T::AccountId) -> T::PoolId
where
	T: pallet_pool::Config + frame_system::Config,
	GetCurrencyId: Get<pallet_pool::CurrencyIdOf<T>>,
{
	// currencyId is 1
	pallet_pool::Pallet::<T>::create_new_pool(owner, "USD Pool".into(), GetCurrencyId::get())
}

pub(crate) fn initialise_test_pool<T>(
	pool_id: PoolIdOf<T>,
	class_id: u64,
	admin_origin: T::Origin,
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
	pallet_loan::Pallet::<T>::initialise_pool(admin_origin, pool_id, class_id)
		.expect("initialisation of pool should not fail");
	class_id
}

pub(crate) fn assert_last_event<T: pallet_loan::Config>(
	generic_event: <T as pallet_loan::Config>::Event,
) {
	let events = frame_system::Pallet::<T>::events();
	let system_event: <T as frame_system::Config>::Event = generic_event.into();
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
