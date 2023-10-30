// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#[cfg(feature = "try-runtime")]
use frame_support::ensure;
use frame_support::{
	dispatch::GetStorageVersion,
	storage::unhashed,
	traits::{Get, OnRuntimeUpgrade, PalletInfoAccess, StorageVersion},
	weights::{RuntimeDbWeight, Weight},
};
#[cfg(feature = "try-runtime")]
use sp_runtime::DispatchError;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

/// This upgrade nukes all storages from the pallet individually.
/// This upgrade is only executed if pallet version has changed.
///
/// To handle possible issues forgetting removing the upgrade,
/// you must specify the ON_CHAIN_VERSION,
/// which represent the expected previous on-chain version when the upgrade is
/// done. If these numbers mismatch, the upgrade will not take effect.
pub struct Migration<Pallet, DbWeight, const ON_CHAIN_VERSION: u16>(
	sp_std::marker::PhantomData<(Pallet, DbWeight)>,
);

impl<Pallet, DbWeight, const ON_CHAIN_VERSION: u16> OnRuntimeUpgrade
	for Migration<Pallet, DbWeight, ON_CHAIN_VERSION>
where
	Pallet: GetStorageVersion<CurrentStorageVersion = StorageVersion> + PalletInfoAccess,
	DbWeight: Get<RuntimeDbWeight>,
{
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
		ensure!(
			Pallet::on_chain_storage_version() == StorageVersion::new(ON_CHAIN_VERSION),
			"Pallet on-chain version must match with ON_CHAIN_VERSION"
		);

		ensure!(
			Pallet::on_chain_storage_version() < Pallet::current_storage_version(),
			"Pallet is already updated"
		);

		// NOTE: We still want to be able to bump StorageVersion
		if !unhashed::contains_prefixed_key(&pallet_prefix::<Pallet>()) {
			log::info!(
				"Nuke-{}: Pallet prefix doesn't exist, storage is empty already",
				Pallet::name(),
			)
		}

		Ok(Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		if Pallet::on_chain_storage_version() != StorageVersion::new(ON_CHAIN_VERSION) {
			log::error!(
				"Nuke-{}: nuke aborted. This upgrade must be removed!",
				Pallet::name()
			);
			return Weight::zero();
		}

		if Pallet::on_chain_storage_version() < Pallet::current_storage_version() {
			log::info!("Nuke-{}: nuking pallet...", Pallet::name());

			let result = unhashed::clear_prefix(&pallet_prefix::<Pallet>(), None, None);
			match result.maybe_cursor {
				None => log::info!("Nuke-{}: storage cleared successful", Pallet::name()),
				Some(_) => {
					// TODO: Should we loop over maybe_cursor as a new prefix?
					// By now, returning error.
					log::error!("Nuke-{}: storage not totally cleared", Pallet::name())
				}
			}

			log::info!(
				"Nuke-{}: iteration result. backend: {} unique: {} loops: {}",
				Pallet::name(),
				result.backend,
				result.unique,
				result.loops,
			);

			Pallet::current_storage_version().put::<Pallet>();

			DbWeight::get().writes(result.unique.into())
				+ DbWeight::get().reads(result.loops.into())
				+ DbWeight::get().reads_writes(1, 1) // Version read & writen
		} else {
			log::warn!(
				"Nuke-{}: pallet on-chain version is not less than {:?}. This upgrade can be removed.",
				Pallet::name(),
				Pallet::current_storage_version()
			);
			DbWeight::get().reads(1)
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), DispatchError> {
		assert_eq!(
			Pallet::on_chain_storage_version(),
			Pallet::current_storage_version(),
			"on-chain storage version should have been updated"
		);

		ensure!(
			!contains_prefixed_key_skip_storage_version::<Pallet>(&pallet_prefix::<Pallet>()),
			"Pallet prefix still exists!"
		);

		Ok(())
	}
}

fn pallet_prefix<Pallet: PalletInfoAccess>() -> [u8; 16] {
	sp_io::hashing::twox_128(Pallet::name().as_bytes())
}

pub fn contains_prefixed_key_skip_storage_version<Pallet: PalletInfoAccess>(prefix: &[u8]) -> bool {
	let mut next_key = prefix.to_vec();
	loop {
		match sp_io::storage::next_key(&next_key) {
			// We catch the storage version if it is found.
			// If we catch another key first, the trie contains keys that are not the
			// the storage version. We check the prefix and break the loop.
			Some(key) if key == StorageVersion::storage_key::<Pallet>() => next_key = key,
			Some(key) => break key.starts_with(prefix),
			None => {
				break false;
			}
		}
	}
}
