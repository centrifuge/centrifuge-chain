use frame_support::{
	ensure, storage,
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
};
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

use crate::*;

/// This migration nukes all storages from the pallet individually.
pub struct NukeMigration<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for NukeMigration<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		let prefix: [u8; 16] = sp_io::hashing::twox_128(b"Loans");

		ensure!(
			util::contains_prefixed_key(&prefix),
			"Pallet loans prefix doesn't exists"
		);

		Ok(Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		let prefix: [u8; 16] = sp_io::hashing::twox_128(b"Loans");
		let result = storage::unhashed::clear_prefix(&prefix, None, None);

		log::debug!("Loans storage clearing migration successful");

		T::DbWeight::get().writes(result.unique.into())
			+ T::DbWeight::get().reads(result.loops.into())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
		let prefix: [u8; 16] = sp_io::hashing::twox_128(b"Loans");
		ensure!(
			!util::contains_prefixed_key(&prefix),
			"Pallet loans prefix still exists!"
		);

		Ok(())
	}
}

mod util {
	pub fn contains_prefixed_key(prefix: &[u8]) -> bool {
		// Implementation extracted from a newer version of `frame_support`.
		match sp_io::storage::next_key(prefix) {
			Some(key) => key.starts_with(prefix),
			None => false,
		}
	}
}
