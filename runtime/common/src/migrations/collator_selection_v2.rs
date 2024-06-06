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

// More info: https://github.com/paritytech/polkadot-sdk/pull/4229#issuecomment-2151690311
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::{
	pallet_prelude::*,
	storage_alias,
	traits::{Currency, ReservableCurrency},
};

use pallet_collator_selection::*;
use sp_runtime::traits::{Saturating, Zero};
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

const LOG_TARGET: &str = "runtime::collator-selection";

/// [`UncheckedMigrationToV2`] wrapped in a
/// [`VersionedMigration`](frame_support::migrations::VersionedMigration), ensuring the
/// migration is only performed when on-chain version is 1.
pub type MigrationToV2<T> = frame_support::migrations::VersionedMigration<
	1,
	2,
	UncheckedMigrationToV2<T>,
	Pallet<T>,
	<T as frame_system::Config>::DbWeight,
>;

#[storage_alias]
pub type Candidates<T: Config> = StorageValue<
	Pallet<T>,
	BoundedVec<
		CandidateInfo<
			<T as frame_system::Config>::AccountId,
			<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance,
		>,
		<T as Config>::MaxCandidates,
	>,
	ValueQuery,
>;

/// Migrate to V2.
pub struct UncheckedMigrationToV2<T>(sp_std::marker::PhantomData<T>);
impl<T: Config + pallet_balances::Config> OnRuntimeUpgrade for UncheckedMigrationToV2<T> {
	fn on_runtime_upgrade() -> Weight {
		let mut weight = Weight::zero();
		let mut count: u64 = 0;
		// candidates who exist under the old `Candidates` key
		let candidates = Candidates::<T>::take();

		// New candidates who have registered since the upgrade. Under normal circumstances,
		// this should not exist because the migration should be applied when the upgrade
		// happens. But in Polkadot/Kusama we messed this up, and people registered under
		// `CandidateList` while their funds were locked in `Candidates`.
		let new_candidate_list = CandidateList::<T>::get();
		if new_candidate_list.len().is_zero() {
			// The new list is empty, so this is essentially being applied correctly. We just
			// put the candidates into the new storage item.
			log::info!(
				target: LOG_TARGET,
				"New candidate list is empty, adding {} previous candidates",
				candidates.len(),
			);
			CandidateList::<T>::put(&candidates);
			// 1 write for the new list
			weight.saturating_accrue(T::DbWeight::get().reads_writes(0, 1));
		} else {
			// Oops, the runtime upgraded without the migration. There are new candidates in
			// `CandidateList`. So, let's just refund the old ones and assume they have already
			// started participating in the new system.
			for candidate in candidates {
				let err = T::Currency::unreserve(&candidate.who, candidate.deposit);
				if err > Zero::zero() {
					log::error!(
						target: LOG_TARGET,
						"{:?} balance was unable to be unreserved from {:?}",
						err, &candidate.who,
					);
				}
				count.saturating_inc();
			}
			weight.saturating_accrue(
                    <<T as pallet_balances::Config>::WeightInfo as pallet_balances::WeightInfo>::force_unreserve().saturating_mul(count.into()),
                );
		}

		log::info!(
			target: LOG_TARGET,
			"Unreserved locked bond of {} candidates, upgraded storage to version 2",
			count,
		);

		weight.saturating_accrue(T::DbWeight::get().reads_writes(3, 2));
		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
		let number_of_candidates = Candidates::<T>::get().to_vec().len();
		Ok((number_of_candidates as u32).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(number_of_candidates: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		let new_number_of_candidates = Candidates::<T>::get().to_vec().len();
		assert_eq!(
			new_number_of_candidates, 0 as usize,
			"after migration, the candidates map should be empty"
		);
		let count_pre: u32 = Decode::decode(&mut number_of_candidates.as_slice())
			.expect("pre_upgrade provides a valid state; qed");
		assert_eq!(
			count_pre,
			CandidateList::<T>::get().len() as u32,
			"after migration, the CandidateList should equal old Candidate storage"
		);
		Ok(())
	}
}
