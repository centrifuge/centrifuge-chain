// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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

use cfg_primitives::{PoolId, TrancheId};
use cfg_types::{
	permissions::{v0::PermissionRoles as PermissionRolesV0, PermissionRoles, PermissionScope},
	time::TimeProvider,
	tokens::CurrencyId,
};
#[cfg(feature = "try-runtime")]
use frame_support::pallet_prelude::{Decode, Encode};
use frame_support::{
	storage_alias,
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
	Blake2_128Concat,
};
use sp_arithmetic::traits::SaturatedConversion;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

mod v0 {
	use super::*;

	#[storage_alias]
	pub type Permission<T: pallet_permissions::Config + frame_system::Config, MinD, MaxT> =
		StorageDoubleMap<
			pallet_permissions::Pallet<T>,
			Blake2_128Concat,
			<T as frame_system::Config>::AccountId,
			Blake2_128Concat,
			PermissionScope<PoolId, CurrencyId>,
			PermissionRolesV0<TimeProvider<pallet_timestamp::Pallet<T>>, MinD, TrancheId, MaxT>,
		>;
}

const LOG_PREFIX: &str = "PermssionsV1";

pub struct Migration<T, MinDelay, MaxTranches>(
	sp_std::marker::PhantomData<(T, MinDelay, MaxTranches)>,
);

impl<T, MinDelay, MaxTranches> OnRuntimeUpgrade for Migration<T, MinDelay, MaxTranches>
where
	T: pallet_permissions::Config<
			Storage = PermissionRoles<
				TimeProvider<pallet_timestamp::Pallet<T>>,
				MinDelay,
				TrancheId,
				MaxTranches,
			>,
		> + frame_system::Config
		+ pallet_timestamp::Config,
	MinDelay: 'static,
	MaxTranches: Get<u32> + 'static,
{
	fn on_runtime_upgrade() -> Weight {
		let writes = v0::Permission::<T, MinDelay, MaxTranches>::iter_keys()
			.count()
			.saturated_into();

		pallet_permissions::Permission::<T>::translate_values::<
			PermissionRolesV0<
				TimeProvider<pallet_timestamp::Pallet<T>>,
				MinDelay,
				TrancheId,
				MaxTranches,
			>,
			_,
		>(|role| Some(role.migrate().into()));

		log::info!("{LOG_PREFIX}: Migrated {writes} permissions!");
		T::DbWeight::get().reads_writes(1, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		let count: u64 = v0::Permission::<T, MinDelay, MaxTranches>::iter_keys()
			.count()
			.saturated_into();

		log::info!("{LOG_PREFIX}: Pre checks done!");

		Ok(count.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		let pre_count: u64 = Decode::decode(&mut pre_state.as_slice())
			.expect("pre_upgrade provides a valid state; qed");
		let post_count: u64 = pallet_permissions::Permission::<T>::iter_keys()
			.count()
			.saturated_into();
		assert_eq!(
			pre_count, post_count,
			"{LOG_PREFIX}: Mismatching number of permission roles after migration!"
		);

		log::info!("{LOG_PREFIX}: Post checks done!");

		Ok(())
	}
}
