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

use codec::{Decode, Encode};
use frame_support::{log, traits::Get, weights::Weight, Blake2_256};

#[cfg(feature = "try-runtime")]
use frame_support::ensure; // Not in prelude for try-runtime

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
struct Fee<Hash, Balance> {
	key: Hash,
	price: Balance,
}

#[frame_support::storage_alias]
type Fees<T: Config> = StorageMap<
	Pallet<T>,
	Blake2_256,
	<T as frame_system::Config>::Hash,
	Fee<<T as frame_system::Config>::Hash, BalanceOf<T>>,
>;

pub mod fee_balances {
	use super::*;

	#[cfg(feature = "try-runtime")]
	pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
		let count = Fees::<T>::iter_values().count();
		ensure!(count != 0, "Fees storage items not found!");
		log::info!(
				target: "runtime::fees::pre-migrate",
				"Pre Migrate check passed with count {:?}",
				count,
		);
		Ok(())
	}

	pub fn migrate<T: Config>() -> Weight {
		let _ = Fees::<T>::clear(u32::MAX, None);
		log::info!(target: "runtime::fees::migrate", "Done Migrating");
		T::DbWeight::get().reads_writes(1, 1)
	}

	#[cfg(feature = "try-runtime")]
	pub fn post_migrate<T: Config>() -> Result<(), &'static str> {
		ensure!(
			Fees::<T>::iter_values().count() == 0,
			"Fees storage should be empty!"
		);
		log::info!(target: "runtime::fees::post-migrate", "Post Migrate check passed");
		Ok(())
	}
}

#[cfg(test)]
#[cfg(feature = "try-runtime")]
mod test {
	use super::{fee_balances::*, Fee, Fees};
	use crate::mock::{new_test_ext, Test};
	use frame_support::assert_ok;
	use sp_core::H256;

	#[test]
	fn should_remove_all() {
		new_test_ext().execute_with(|| {
			let hash: H256 = [1; 32].into();
			Fees::<Test>::insert(
				hash,
				Fee {
					key: hash,
					price: 100,
				},
			);

			assert_ok!(pre_migrate::<Test>());
			assert!(post_migrate::<Test>().is_err());

			migrate::<Test>();

			assert_eq!(Fees::<Test>::iter_values().count(), 0);
			assert_ok!(post_migrate::<Test>());
			assert!(pre_migrate::<Test>().is_err());
		});
	}
}
