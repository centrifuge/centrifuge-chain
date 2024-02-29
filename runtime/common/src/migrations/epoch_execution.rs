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

use frame_support::traits::{Get, GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use pallet_order_book::weights::Weight;
use pallet_pool_system::{Config, EpochExecution, EpochExecutionInfo, Nav, Pallet as PoolSystem};
#[cfg(feature = "try-runtime")]
use parity_scale_codec::{Decode, Encode};
use sp_runtime::traits::Zero;

const LOG_PREFIX: &str = "EpochExecutionMigration: ";

pub(crate) mod v1 {
	use frame_support::{
		dispatch::{Decode, Encode, MaxEncodedLen, TypeInfo},
		pallet_prelude::Get,
		RuntimeDebug,
	};
	use pallet_pool_system::{tranches::EpochExecutionTranches, Config, EpochSolution};

	#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub(crate) struct EpochExecutionInfo<
		Balance,
		BalanceRatio,
		EpochId,
		Weight,
		BlockNumber,
		TrancheCurrency,
		MaxTranches,
	> where
		MaxTranches: Get<u32>,
	{
		pub epoch: EpochId,
		pub nav: Balance,
		pub reserve: Balance,
		pub max_reserve: Balance,
		pub tranches:
			EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency, MaxTranches>,
		pub best_submission: Option<EpochSolution<Balance, MaxTranches>>,
		pub challenge_period_end: Option<BlockNumber>,
	}

	pub(crate) type EpochExecutionInfoOf<T> = EpochExecutionInfo<
		<T as Config>::Balance,
		<T as Config>::BalanceRatio,
		<T as Config>::EpochId,
		<T as Config>::TrancheWeight,
		<T as frame_system::Config>::BlockNumber,
		<T as Config>::TrancheCurrency,
		<T as Config>::MaxTranches,
	>;

	#[cfg(feature = "try-runtime")]
	#[frame_support::storage_alias]
	pub(crate) type EpochExecution<T: Config> = StorageMap<
		pallet_pool_system::Pallet<T>,
		frame_support::Blake2_128Concat,
		<T as Config>::PoolId,
		EpochExecutionInfoOf<T>,
	>;
}

pub struct Migration<T>
where
	T: Config + frame_system::Config,
{
	_phantom: sp_std::marker::PhantomData<T>,
}

impl<T> OnRuntimeUpgrade for Migration<T>
where
	T: Config + frame_system::Config,
{
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		frame_support::ensure!(
			PoolSystem::<T>::on_chain_storage_version() == 1,
			"Can only upgrade from PoolSystem version 1"
		);
		let count = v1::EpochExecution::<T>::iter().count() as u32;
		log::info!(
			"{LOG_PREFIX} EpochExecution count pre migration is {}",
			count
		);

		Ok(count.encode())
	}

	fn on_runtime_upgrade() -> Weight {
		let mut weight = T::DbWeight::get().reads(1);
		if PoolSystem::<T>::on_chain_storage_version() != 1 {
			log::info!(
					"{LOG_PREFIX} Skipping on_runtime_upgrade: executed on wrong storage version. Expected version 1"
				);
			return weight;
		}

		EpochExecution::<T>::translate::<v1::EpochExecutionInfoOf<T>, _>(|_, old| {
			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
			Some(EpochExecutionInfo {
				epoch: old.epoch,
				nav: Nav::<<T as Config>::Balance>::new(old.nav, <T as Config>::Balance::zero()),
				tranches: old.tranches,
				best_submission: old.best_submission,
				challenge_period_end: old.challenge_period_end,
			})
		});

		PoolSystem::<T>::current_storage_version().put::<PoolSystem<T>>();

		weight.saturating_add(T::DbWeight::get().writes(1))
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		frame_support::ensure!(
			PoolSystem::<T>::on_chain_storage_version()
				== PoolSystem::<T>::current_storage_version(),
			"EpochExecutionMigration: StorageVersion of PoolSystem is not 2"
		);

		let old_count: u32 = Decode::decode(&mut &state[..])
			.expect("EpochExecutionMigration: pre_upgrade provides a valid state; qed");
		let new_count = EpochExecution::<T>::iter().count() as u32;
		log::info!("{LOG_PREFIX} EpochExecutionV2 count post migration is {new_count}",);
		frame_support::ensure!(
			old_count == new_count,
			"EpochExecutionMigration: Mismatch in pre and post counters, must migrate all EpochExecution values!"
		);

		Ok(())
	}
}
