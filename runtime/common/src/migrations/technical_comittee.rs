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

use cfg_primitives::{TechnicalCollective, TechnicalMembership};
use frame_support::traits::{Get, InitializeMembers, OnRuntimeUpgrade};
use pallet_order_book::weights::Weight;
use sp_arithmetic::traits::SaturatedConversion;
use sp_std::vec::Vec;

const LOG_PREFIX: &str = "InitTechnicalCommittee:";

pub struct InitMigration<T, Members>(sp_std::marker::PhantomData<(T, Members)>);

impl<T, Members> OnRuntimeUpgrade for InitMigration<T, Members>
where
	T: frame_system::Config
		+ pallet_membership::Config<TechnicalMembership>
		+ pallet_collective::Config<TechnicalCollective>,
	Members: Get<Vec<T::AccountId>>,
{
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
		let membership_members = pallet_membership::Members::<T, TechnicalMembership>::get();
		let collective_members = pallet_collective::Members::<T, TechnicalCollective>::get();
		assert!(membership_members.is_empty());
		assert!(collective_members.is_empty());
		assert!(!Members::get().is_empty());

		log::info!("{LOG_PREFIX} Pre checks done");

		Ok(sp_std::vec![])
	}

	fn on_runtime_upgrade() -> Weight {
		let membership_members = pallet_membership::Members::<T, TechnicalMembership>::get();
		let collective_members = pallet_collective::Members::<T, TechnicalCollective>::get();

		if membership_members.is_empty() && collective_members.is_empty() {
			log::info!("{LOG_PREFIX} Setting up members");
			<pallet_collective::Pallet<T, TechnicalCollective> as InitializeMembers<
				T::AccountId,
			>>::initialize_members(&Members::get());

			log::info!("{LOG_PREFIX} Migration done");

			T::DbWeight::get().reads_writes(2, Members::get().len().saturated_into())
		} else {
			log::warn!("{LOG_PREFIX} Members are not empty. Skipping initialization. This migration should probably be removed");
			T::DbWeight::get().reads(2)
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		let membership_members = pallet_membership::Members::<T, TechnicalMembership>::get();
		let collective_members = pallet_collective::Members::<T, TechnicalCollective>::get();
		assert_eq!(membership_members.into_inner(), Members::get());
		assert_eq!(collective_members, Members::get());

		log::info!("{LOG_PREFIX} Post checks done");

		Ok(())
	}
}
