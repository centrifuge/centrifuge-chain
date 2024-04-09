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

use frame_support::{
	traits::{GetStorageVersion, OnRuntimeUpgrade, PalletInfoAccess, StorageVersion},
	weights::{constants::RocksDbWeight, Weight},
};
use sp_runtime::traits::Zero;

const LOG_PREFIX: &str = "BumpStorageVersion:";

/// Simply bumps the storage version of a pallet
///
/// NOTE: Use with caution! Must ensure beforehand that a migration is not
/// necessary
pub struct Migration<P, const FROM_VERSION: u16, const TO_VERSION: u16>(
	sp_std::marker::PhantomData<P>,
);
impl<P, const FROM_VERSION: u16, const TO_VERSION: u16> OnRuntimeUpgrade
	for Migration<P, FROM_VERSION, TO_VERSION>
where
	P: GetStorageVersion<CurrentStorageVersion = StorageVersion> + PalletInfoAccess,
{
	fn on_runtime_upgrade() -> Weight {
		if P::on_chain_storage_version() == FROM_VERSION
			&& P::current_storage_version() == TO_VERSION
		{
			log::info!(
				"{LOG_PREFIX} Increasing storage version of {:?} from {:?} to {:?}",
				P::name(),
				P::on_chain_storage_version(),
				P::current_storage_version()
			);
			P::current_storage_version().put::<P>();
			RocksDbWeight::get().writes(1)
		} else {
			log::error!(
				"{LOG_PREFIX} Mismatching versions. Wanted to upgrade from \
			{FROM_VERSION} to {TO_VERSION} but would instead upgrade from {:?} to {:?}",
				P::on_chain_storage_version(),
				P::current_storage_version()
			);
			Zero::zero()
		}
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::DispatchError> {
		assert_eq!(
			P::on_chain_storage_version(),
			FROM_VERSION,
			"Unexpected onchain version: Expected {FROM_VERSION:?}, received {:?}",
			P::on_chain_storage_version(),
		);
		assert_eq!(
			P::current_storage_version(),
			TO_VERSION,
			"Unexpected upgrade version: Expected {TO_VERSION:?}, latest {:?}",
			P::on_chain_storage_version(),
		);
		Ok(sp_std::vec![])
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		assert_eq!(P::on_chain_storage_version(), P::current_storage_version());
		Ok(())
	}
}

/// Simply bumps the storage version of a pallet.
/// Similar to the above but it does not check the current version is TO_VERSION
///
/// NOTE: Use with extreme caution! Must ensure beforehand that a migration is
/// not necessary
pub struct ForceMigration<P, const FROM_VERSION: u16, const TO_VERSION: u16>(
	sp_std::marker::PhantomData<P>,
);
impl<P, const FROM_VERSION: u16, const TO_VERSION: u16> OnRuntimeUpgrade
	for ForceMigration<P, FROM_VERSION, TO_VERSION>
where
	P: GetStorageVersion<CurrentStorageVersion = StorageVersion> + PalletInfoAccess,
{
	fn on_runtime_upgrade() -> Weight {
		if P::on_chain_storage_version() == FROM_VERSION {
			log::warn!("Double-check you really want this migration!!!!",);
			log::info!(
				"{LOG_PREFIX} Increasing storage version of {:?} from {:?} to {TO_VERSION:?}",
				P::name(),
				P::on_chain_storage_version(),
			);
			StorageVersion::new(TO_VERSION).put::<P>();

			RocksDbWeight::get().writes(1)
		} else {
			log::error!(
				"{LOG_PREFIX} Mismatching versions. Wanted to upgrade from \
			{FROM_VERSION} but on-chain version is {:?}",
				P::on_chain_storage_version(),
			);
			Zero::zero()
		}
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::DispatchError> {
		assert_eq!(
			P::on_chain_storage_version(),
			FROM_VERSION,
			"Unexpected onchain version: Expected {FROM_VERSION:?}, received {:?}",
			P::on_chain_storage_version(),
		);
		Ok(sp_std::vec![])
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		assert_eq!(P::on_chain_storage_version(), TO_VERSION);
		Ok(())
	}
}
