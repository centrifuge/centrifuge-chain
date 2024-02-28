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

/// Simply bumps the storage version of a pallet
///
/// NOTE: Use with caution! Must ensure beforehand that a migration is not
/// necessary
pub struct Migration<P>(sp_std::marker::PhantomData<P>);
impl<P> OnRuntimeUpgrade for Migration<P>
where
	P: GetStorageVersion<CurrentStorageVersion = StorageVersion> + PalletInfoAccess,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!(
			"BumpStorageVersion: Increasing storage version of {:?} from {:?} to {:?}",
			P::name(),
			P::on_chain_storage_version(),
			P::current_storage_version()
		);
		P::current_storage_version().put::<P>();
		RocksDbWeight::get().writes(1)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::DispatchError> {
		assert!(
			P::on_chain_storage_version() < P::current_storage_version(),
			"Onchain {:?} vs current pallet version {:?}",
			P::on_chain_storage_version(),
			P::current_storage_version(),
		);
		Ok(sp_std::vec![])
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		assert_eq!(P::on_chain_storage_version(), P::current_storage_version());
		Ok(())
	}
}
