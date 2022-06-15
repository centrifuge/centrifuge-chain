// Copyright 2022 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

use super::*;
use frame_support::{log, traits::Get, weights::Weight, Blake2_128Concat};
use frame_support::pallet_prelude::{ValueQuery};


#[frame_support::storage_alias]
pub type RootHashes<T: Config> = StorageMap<_, Blake2_128Concat, <T as frame_system::Config>::Hash, bool, ValueQuery>;


pub mod root_hashes {
	use super::*;

	#[allow(dead_code)]
	pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
		let count = RootHashes::<T>::iter_values().count();
		ensure!(count != 0, "RootHashes storage items not found!");
		log::info!(
				target: "runtime::claims::pre-migrate",
				"Pre Migrate check passed with count {:?}",
				count,
		);
		Ok(())
	}

	pub fn migrate<T: Config>() -> Weight {
		RootHashes::<T>::remove_all(None); // All keys should be deleted
		log::info!(target: "runtime::claims::migrate", "Done Migrating");
		T::DbWeight::get().reads_writes(1, 1)
	}

	#[allow(dead_code)]
	pub fn post_migrate<T: Config>() -> Result<(), &'static str> {
		ensure!(
			RootHashes::<T>::iter_values().count() == 0,
			"RootHashes storage should be empty!"
		);
		log::info!(target: "runtime::claims::post-migrate", "Post Migrate check passed");
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::mock::{MockRuntime as T, TestExternalitiesBuilder};
	use frame_support::assert_ok;
	use sp_core::H256;

	#[test]
	fn should_kill_root_hashes() {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				assert_eq!(RootHashes::<T>::iter_values().count(), 0);
				let root_hash: H256 = [1; 32].into();
				RootHashes::<T>::insert(root_hash, true);
				assert_eq!(RootHashes::<T>::iter_values().count(), 1);
				assert_ok!(root_hashes::pre_migrate::<T>());
				root_hashes::migrate::<T>();
				assert_eq!(RootHashes::<T>::iter_values().count(), 0);
				assert_eq!(
					root_hashes::pre_migrate::<T>(),
					Err("RootHashes storage items not found!")
				);
			});
	}
}
