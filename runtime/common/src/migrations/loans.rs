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
	pallet_prelude::{Decode, Encode, StorageVersion},
	traits::{Get, GetStorageVersion, OnRuntimeUpgrade},
	weights::Weight,
};
use pallet_loans::{pallet::Pallet as Loans, Config};
use sp_arithmetic::traits::SaturatedConversion;
use sp_runtime::TryRuntimeError;
use sp_std::vec::Vec;

const LOG_PREFIX: &str = "LoansMigrationToV4:";

mod v3 {
	use frame_support::{
		pallet_prelude::{OptionQuery, ValueQuery},
		storage_alias, Blake2_128Concat, BoundedVec,
	};
	pub use pallet_loans::entities::{
		loans::v3::ActiveLoan, pricing::external::v3::CreatedLoan as CreatedLoanStruct,
	};
	use pallet_loans::Config;

	use super::Loans;

	pub type ActiveLoansVec<T> =
		BoundedVec<(<T as Config>::LoanId, ActiveLoan<T>), <T as Config>::MaxActiveLoansPerPool>;

	#[storage_alias]
	pub type ActiveLoans<T: Config> = StorageMap<
		Loans<T>,
		Blake2_128Concat,
		<T as Config>::PoolId,
		ActiveLoansVec<T>,
		ValueQuery,
	>;

	#[storage_alias]
	pub type CreatedLoan<T: Config> = StorageDoubleMap<
		Loans<T>,
		Blake2_128Concat,
		<T as Config>::PoolId,
		Blake2_128Concat,
		<T as Config>::LoanId,
		CreatedLoanStruct<T>,
		OptionQuery,
	>;
}

pub struct AddWithLinearPricing<T>(sp_std::marker::PhantomData<T>);
impl<T> OnRuntimeUpgrade for AddWithLinearPricing<T>
where
	T: Config,
{
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
		assert_eq!(
			Loans::<T>::on_chain_storage_version(),
			StorageVersion::new(3)
		);

		let created_loans: u64 = v3::CreatedLoan::<T>::iter_keys().count().saturated_into();
		let active_loans: u64 = v3::ActiveLoans::<T>::iter_values()
			.map(|v| v.len())
			.sum::<usize>()
			.saturated_into();

		log::info!("{LOG_PREFIX} Pre checks done!");

		Ok((created_loans, active_loans).encode())
	}

	fn on_runtime_upgrade() -> Weight {
		let mut weight = T::DbWeight::get().reads(1);
		if Loans::<T>::on_chain_storage_version() == StorageVersion::new(3) {
			log::info!("{LOG_PREFIX} Starting migration v3 -> v4");

			pallet_loans::CreatedLoan::<T>::translate::<v3::CreatedLoanStruct<T>, _>(
				|_, _, loan| {
					weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
					Some(loan.migrate(true))
				},
			);

			let mut changed_pools = Vec::new();
			pallet_loans::ActiveLoans::<T>::translate::<v3::ActiveLoansVec<T>, _>(
				|pool_id, active_loans| {
					changed_pools.push(pool_id);
					weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
					Some(
						active_loans
							.into_iter()
							.map(|(loan_id, active_loan)| (loan_id, active_loan.migrate(true)))
							.collect::<Vec<_>>()
							.try_into()
							.expect("size doesn't not change, qed"),
					)
				},
			);

			for pool_id in &changed_pools {
				match Loans::<T>::update_nav(*pool_id) {
					Ok(_) => log::info!("{LOG_PREFIX} updated portfolio for pool_id: {pool_id:?}"),
					Err(e) => {
						log::error!(
							"{LOG_PREFIX} error updating the portfolio for {pool_id:?}: {e:?}"
						)
					}
				}
			}

			Loans::<T>::current_storage_version().put::<Loans<T>>();

			let count = changed_pools.len() as u64;
			weight.saturating_accrue(T::DbWeight::get().reads_writes(count, count + 1));
			log::info!("{LOG_PREFIX} Migrated {} pools", count);

			weight
		} else {
			log::info!(
				"{LOG_PREFIX} Migration to v4 did not execute. This probably should be removed"
			);
			weight
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(pre_state: Vec<u8>) -> Result<(), TryRuntimeError> {
		assert_eq!(
			Loans::<T>::on_chain_storage_version(),
			StorageVersion::new(4)
		);

		let (pre_created, pre_active) =
			<(u64, u64)>::decode(&mut pre_state.as_slice()).expect("Pre state valid; qed");
		let post_created: u64 = pallet_loans::CreatedLoan::<T>::iter_keys()
			.count()
			.saturated_into();
		let post_active: u64 = pallet_loans::ActiveLoans::<T>::iter_values()
			.map(|v| v.len())
			.sum::<usize>()
			.saturated_into();
		assert_eq!(
			pre_created, post_created,
			"Number of CreatedLoans mismatches: pre {pre_created} vs post {post_created}"
		);
		assert_eq!(
			pre_active, post_active,
			"Number of ActiveLoans mismatches: pre {pre_active} vs post {post_active}"
		);

		log::info!("{LOG_PREFIX} Post checks done!");

		Ok(())
	}
}
