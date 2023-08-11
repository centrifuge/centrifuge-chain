#[cfg(feature = "try-runtime")]
use frame_support::ensure;
use frame_support::{
	dispatch::GetStorageVersion,
	storage,
	traits::{Get, OnRuntimeUpgrade, PalletInfoAccess, StorageVersion},
	weights::{RuntimeDbWeight, Weight},
};
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

use crate::*;

/// This upgrade nukes all storages from the pallet individually.
/// This upgrade is only executed if pallet version has changed.
///
/// To handle possible issues forgeting removing the upgrade,
/// you must specify the PREV_VERSION,
/// which represent the expected on-chain version when the upgrade is done
/// If these numbers mistmatch, the upgrade will not take effect.
pub struct Migration<Pallet, DbWeight, const PREV_VERSION: u16>(
	sp_std::marker::PhantomData<(Pallet, DbWeight)>,
);

impl<Pallet, DbWeight, const PREV_VERSION: u16> OnRuntimeUpgrade
	for Migration<Pallet, DbWeight, PREV_VERSION>
where
	Pallet: GetStorageVersion + PalletInfoAccess,
	DbWeight: Get<RuntimeDbWeight>,
{
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		ensure!(
			Pallet::on_chain_storage_version() == StorageVersion::new(PREV_VERSION),
			"Pallet on-chain version must match with PREV_VERSION"
		);

		ensure!(
			Pallet::on_chain_storage_version() < STORAGE_VERSION,
			"Pallet is already updated"
		);

		ensure!(
			storage::unhashed::contains_prefixed_key(&pallet_prefix::<Pallet>()),
			"Pallet prefix doesn't exists"
		);

		Ok(Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		if Pallet::on_chain_storage_version() == StorageVersion::new(PREV_VERSION) {
			log::error!(
				"Nuke-{}: Nuke aborted. This upgrade must be removed!",
				Pallet::name()
			);
			return Weight::zero();
		}

		if Pallet::on_chain_storage_version() < STORAGE_VERSION {
			log::info!("Nuke-{}: Nuking pallet...", Pallet::name());

			// TODO: Future improvements of this upgrade should loop over `clear_prefix`
			// calls removing the entire storage.
			let result = storage::unhashed::clear_prefix(&pallet_prefix::<Pallet>(), None, None);
			log::info!(
				"Nuke-{}: iteration result. backend: {} unique: {} loops: {}",
				Pallet::name(),
				result.backend,
				result.unique,
				result.loops,
			);
			match result.maybe_cursor {
				None => log::info!("Nuke-{}: storage cleared successful", Pallet::name()),
				Some(_) => log::error!("Nuke-{}: storage not totally cleared", Pallet::name()),
			}

			Pallet::current_storage_version().put::<Pallet>();

			DbWeight::get().writes(result.unique.into())
				+ DbWeight::get().reads(result.loops.into())
				+ DbWeight::get().reads_writes(1, 1) // Version read & writen
		} else {
			log::warn!(
                "Nuke-{}: pallet on-chain version is not {STORAGE_VERSION:?}. This upgrade can be removed.",
                Pallet::name()
            );
			DbWeight::get().reads(1)
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
		assert_eq!(
			Pallet::on_chain_storage_version(),
			STORAGE_VERSION,
			"on-chain storage version should have been updated"
		);

		ensure!(
			!storage::unhashed::contains_prefixed_key(&pallet_prefix::<Pallet>()),
			"Pallet prefix still exists!"
		);

		Ok(())
	}
}

fn pallet_prefix<Pallet: PalletInfoAccess>() -> [u8; 16] {
	sp_io::hashing::twox_128(Pallet::name().as_bytes())
}
