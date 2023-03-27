use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use sp_std::vec::Vec;

use crate::*;

/// This migration nukes all storages from the pallet individually.
pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for Migration<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		// The current state from altair when this migration should be done has only one element
		// in each store.
		ensure!(
			PoolToLoanNftClass::<T>::iter_values().count() == 1,
			"Err PoolToLoanNftClass"
		);
		ensure!(
			LoanNftClassToPool::<T>::iter_values().count() == 1,
			"Err LoanNftClassToPool"
		);
		ensure!(
			NextLoanId::<T>::iter_values().count() == 1,
			"Err NextLoanId"
		);
		ensure!(Loan::<T>::iter_values().count() == 1, "Err Loan");
		ensure!(
			ActiveLoans::<T>::iter_values().count() == 1,
			"Err ActiveLoans"
		);
		ensure!(
			ClosedLoans::<T>::iter_values().count() == 1,
			"Err ClosedLoans"
		);
		ensure!(PoolNAV::<T>::iter_values().count() == 1, "Err PoolNAV");
		ensure!(
			PoolWriteOffGroups::<T>::iter_values().count() == 1,
			"Err PoolWriteOffGroups "
		);

		Ok(Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		let _ = PoolToLoanNftClass::<T>::clear(1, None);
		let _ = LoanNftClassToPool::<T>::clear(1, None);
		let _ = NextLoanId::<T>::clear(1, None);
		let _ = Loan::<T>::clear(1, None);
		let _ = ActiveLoans::<T>::clear(1, None);
		let _ = ClosedLoans::<T>::clear(1, None);
		let _ = PoolNAV::<T>::clear(1, None);
		let _ = PoolWriteOffGroups::<T>::clear(1, None);

		T::DbWeight::get().writes(8)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
		ensure!(
			PoolToLoanNftClass::<T>::iter_values().count() == 0,
			"Exists PoolToLoanNftClass"
		);
		ensure!(
			LoanNftClassToPool::<T>::iter_values().count() == 0,
			"Exists LoanNftClassToPool"
		);
		ensure!(
			NextLoanId::<T>::iter_values().count() == 0,
			"Exists NextLoanId"
		);
		ensure!(Loan::<T>::iter_values().count() == 0, "Exists Loan");
		ensure!(
			ActiveLoans::<T>::iter_values().count() == 0,
			"Exists ActiveLoans"
		);
		ensure!(
			ClosedLoans::<T>::iter_values().count() == 0,
			"Exists ClosedLoans"
		);
		ensure!(PoolNAV::<T>::iter_values().count() == 0, "Exists PoolNAV");
		ensure!(
			PoolWriteOffGroups::<T>::iter_values().count() == 0,
			"Exists PoolWriteOffGroups"
		);

		Ok(())
	}
}
