#[cfg(feature = "try-runtime")]
use frame_support::ensure;
use frame_support::{
	pallet_prelude::ValueQuery,
	storage, storage_alias,
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
	Blake2_128Concat,
};
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

use crate::*;

mod old {
	use super::*;

	/// This storage comes from the previous pallet loans.
	/// It is used as an indicator that the previous pallet loans still exists.
	/// If this storage is not found, the nuking process is aborted.
	#[storage_alias]
	pub(crate) type NextLoanId<T: Config> =
		StorageMap<Pallet<T>, Blake2_128Concat, <T as Config>::PoolId, u128, ValueQuery>;
}

/// This migration nukes all storages from the pallet individually.
pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for Migration<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		ensure!(
			contains_prefixed_key(&loan_prefix()),
			"Pallet loans prefix doesn't exists"
		);

		ensure!(
			old::NextLoanId::<T>::iter_values().count() == 1,
			"Pallet loans contains doesn't contain old data"
		);

		Ok(Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		let old_values = old::NextLoanId::<T>::iter_values().count();
		if old_values > 0 {
			let result = storage::unhashed::clear_prefix(&loan_prefix(), None, None);

			match result.maybe_cursor {
				None => log::info!("Loans: storage cleared successful"),
				Some(_) => log::error!("Loans: storage not totally cleared"),
			}

			T::DbWeight::get().writes(result.unique.into())
				+ T::DbWeight::get().reads(result.loops.into())
		} else {
			log::warn!("Loans: storage was already clear. This migration can be removed.");

			T::DbWeight::get().reads(old_values as u64)
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
		ensure!(
			!contains_prefixed_key(&loan_prefix()),
			"Pallet loans prefix still exists!"
		);

		ensure!(
			old::NextLoanId::<T>::iter_values().count() == 0,
			"Pallet loans still contains old data"
		);

		Ok(())
	}
}

fn loan_prefix() -> [u8; 16] {
	sp_io::hashing::twox_128(b"Loans")
}

#[cfg(feature = "try-runtime")]
fn contains_prefixed_key(prefix: &[u8]) -> bool {
	// Implementation extracted from a newer version of `frame_support`.
	match sp_io::storage::next_key(prefix) {
		Some(key) => key.starts_with(prefix),
		None => false,
	}
}
