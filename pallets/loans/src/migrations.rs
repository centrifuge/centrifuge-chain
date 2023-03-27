use frame_support::{storage, traits::OnRuntimeUpgrade, weights::Weight};
use sp_std::vec::Vec;

use crate::*;

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for Migration<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		ensure!(
			ClosedLoans::<T>::iter_values().count() == 1,
			"There is not a closed loan"
		);

		Ok(Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		let loans_module = b"4c82a580ac33cceba8ed9766387f22b7";
		let _ = storage::unhashed::clear_prefix(loans_module, None, None);

		// Should be one/few elements per storage map in `pallet-loans`. The next number should be
		// enough.
		Weight::from_ref_time(200_000_000)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
		ensure!(
			ClosedLoans::<T>::iter_values().count() == 0,
			"There is still a closed loan"
		);
		Ok(())
	}
}
