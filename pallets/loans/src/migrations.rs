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

use cfg_traits::PoolNAV;
use frame_support::{
	dispatch::GetStorageVersion, inherent::Vec, log, pallet_prelude::StorageVersion, traits::Get,
	weights::Weight,
};

use crate::{ActiveLoans, Config, Pallet};

mod v2 {
	use frame_support::{pallet_prelude::*, storage_alias};

	use crate::{entities::loans::v2, Config, Pallet};

	pub type ActiveLoansVec<T> = BoundedVec<
		(<T as Config>::LoanId, v2::ActiveLoan<T>),
		<T as Config>::MaxActiveLoansPerPool,
	>;

	#[storage_alias]
	pub type ActiveLoans<T: Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		<T as Config>::PoolId,
		ActiveLoansVec<T>,
		ValueQuery,
	>;
}

pub fn migrate_from_v2_to_v3<T: Config>() -> Weight {
	if Pallet::<T>::on_chain_storage_version() == StorageVersion::new(2) {
		log::info!("Loans: Starting migration v2 -> v3");

		let mut changed_pools = Vec::new();
		ActiveLoans::<T>::translate::<v2::ActiveLoansVec<T>, _>(|pool_id, active_loans| {
			changed_pools.push(pool_id);
			Some(
				active_loans
					.into_iter()
					.map(|(loan_id, active_loan)| (loan_id, active_loan.migrate()))
					.collect::<Vec<_>>()
					.try_into()
					.expect("size doest not change, qed"),
			)
		});

		for pool_id in &changed_pools {
			match Pallet::<T>::update_nav(*pool_id) {
				Ok(_) => log::info!("Loans: updated portfolio for pool_id: {pool_id:?}"),
				Err(e) => log::error!("Loans: error updating the portfolio for {pool_id:?}: {e:?}"),
			}
		}

		Pallet::<T>::current_storage_version().put::<Pallet<T>>();

		let count = changed_pools.len() as u64;
		log::info!("Loans: Migrated {} pools", count);
		T::DbWeight::get().reads_writes(count + 1, count + 1)
	} else {
		// wrong storage version
		log::info!("Loans: Migration did not execute. This probably should be removed");
		T::DbWeight::get().reads_writes(1, 0)
	}
}
