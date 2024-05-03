// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

//! Centrifuge Runtime-Common Migrations

pub mod increase_storage_version;
pub mod nuke;
pub mod precompile_account_codes;
pub mod technical_comittee;

pub mod utils {
	use frame_support::storage::unhashed;
	use sp_arithmetic::traits::Saturating;

	/// Iterates keys of storage and removes undecodable keys
	///
	/// WARNING: USE WITH EXTREME CAUTION! Ensure the cleanup can be performed
	/// beforehand!
	pub fn remove_undecodable_storage_keys<T: parity_scale_codec::Decode + Sized>(
		prefix: [u8; 32],
	) -> (u64, u64) {
		let mut previous_key = prefix.clone().to_vec();
		let mut reads: u64 = 1;
		let mut writes: u64 = 0;
		while let Some(next) =
			sp_io::storage::next_key(&previous_key).filter(|n| n.starts_with(&prefix))
		{
			reads.saturating_accrue(1);
			previous_key = next;
			let maybe_value = unhashed::get::<T>(&previous_key);
			match maybe_value {
				Some(_) => continue,
				None => {
					log::debug!(
						"Removing key which could not be decoded at {:?}",
						previous_key
					);
					unhashed::kill(&previous_key);
					writes.saturating_accrue(1);
					continue;
				}
			}
		}

		(reads, writes)
	}

	/// Iterates all keys of a storage and returns the count.
	///
	/// NOTE: Necessary because `Storage::<T>::iter().count()` does not account
	/// for keys which cannot be decoded.
	pub fn count_storage_keys(prefix: [u8; 32]) -> u32 {
		let mut previous_key = prefix.clone().to_vec();
		let mut n: u32 = 0;
		while let Some(next) =
			sp_io::storage::next_key(&previous_key).filter(|n| n.starts_with(&prefix))
		{
			n.saturating_accrue(1);
			previous_key = next;
		}

		n
	}
}
