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

/// The migration set for Development & Demo.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeDevelopment1047 = (
	pallet_collator_selection::migration::v1::MigrateToV1<crate::Runtime>,
	cleanup_foreign_investments::Migration<crate::Runtime>,
);

mod cleanup_foreign_investments {
	use cfg_types::tokens::CurrencyId;
	use frame_support::{
		storage::StoragePrefixedMap,
		traits::{Get, OnRuntimeUpgrade},
		weights::Weight,
	};
	use runtime_common::migrations::utils::{count_storage_keys, remove_undecodable_storage_keys};
	#[cfg(feature = "try-runtime")]
	use sp_runtime::DispatchError;
	use sp_runtime::SaturatedConversion;

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	const LOG_PREFIX: &str = "CleanupForeignInvestments";
	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: pallet_foreign_investments::Config + frame_system::Config,
	{
		fn on_runtime_upgrade() -> Weight {
			log::info!("{LOG_PREFIX} Initiating removal of undecodable keys");
			let (reads, writes) = remove_undecodable_storage_keys::<CurrencyId>(
				pallet_foreign_investments::ForeignInvestmentInfo::<T>::final_prefix(),
			);

			log::info!("{LOG_PREFIX} Removed {writes} undecodable keys");

			T::DbWeight::get().reads_writes(reads, writes)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, DispatchError> {
			let n: u32 = count_storage_keys(
				pallet_foreign_investments::ForeignInvestmentInfo::<T>::final_prefix(),
			);
			let n_count: u32 = pallet_foreign_investments::ForeignInvestmentInfo::<T>::iter_keys()
				.count()
				.saturated_into();

			if n == n_count {
				log::info!(
					"{LOG_PREFIX} Storage cleanup can be skipped because all keys can be decoded"
				);
			} else {
				log::info!(
					"{LOG_PREFIX} Failed to decode {} keys, cleanup necessary",
					n.saturating_sub(n_count)
				);
			}

			log::info!("{LOG_PREFIX} pre_upgrade done!",);

			Ok(sp_std::vec![])
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_pre_state: sp_std::vec::Vec<u8>) -> Result<(), DispatchError> {
			let n: u32 = count_storage_keys(
				pallet_foreign_investments::ForeignInvestmentInfo::<T>::final_prefix(),
			);
			let n_count: u32 = pallet_foreign_investments::ForeignInvestmentInfo::<T>::iter_keys()
				.count()
				.saturated_into();
			assert_eq!(n, n_count);

			log::info!("{LOG_PREFIX} post_upgrade done with {n} remaining storage keys!",);

			Ok(())
		}
	}
}
