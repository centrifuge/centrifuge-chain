use codec::{Decode, Encode};
use data::system_account::SYSTEM_ACCOUNT;
use frame_support::storage::unhashed;
use frame_support::weights::{constants::RocksDbWeight, Weight};
use pallet_migration_manager::traits::RuntimeUpgradeProvider;
use sp_std::vec::Vec;
use sp_version::RuntimeVersion;

mod data;

pub struct Migrator;

#[derive(Debug, Encode, Decode)]
pub struct Info {
	pub last_index: u64,
}

impl sp_std::default::Default for Info {
	fn default() -> Self {
		Info { last_index: 0 }
	}
}

impl RuntimeUpgradeProvider for Migrator {
	type Memo = Vec<u8>;
	type StateInfo = Info;

	fn info() -> (RuntimeVersion, Self::Memo) {
		(
			crate::VERSION,
			b"This Upgrade includes: Migration from all System.Account \
            storage values and keys into the storage of this chain."
				.as_ref()
				.to_vec(),
		)
	}

	fn upgrade_weight() -> Weight {
		Self::calc_weight()
	}

	fn next(usable: Weight, last_state: Option<Self::StateInfo>) -> (Weight, Self::StateInfo) {
		let pre_index: usize;
		let post_index: usize;

		if last_state.is_none() {
			let per_write = RocksDbWeight::get().writes(1);
			let num_writes = SYSTEM_ACCOUNT.len();

			let this_run = (usable / per_write) as usize;
			let mut index = 0usize;
			pre_index = 0usize;

			while index < this_run && index < num_writes {
				unhashed::put_raw(&SYSTEM_ACCOUNT[index].key, &SYSTEM_ACCOUNT[index].value);
				index += 1;
			}
			post_index = index;
		} else {
			let info = last_state.expect("StateInfo is some. qed.");

			let per_write = RocksDbWeight::get().writes(1);
			let num_writes = SYSTEM_ACCOUNT.len();

			let this_run = (usable / per_write) as usize;
			let mut index = info.last_index as usize;
			pre_index = info.last_index as usize;

			while index < this_run && index < num_writes {
				unhashed::put_raw(&SYSTEM_ACCOUNT[index].key, &SYSTEM_ACCOUNT[index].value);
				index += 1;
			}
			post_index = index;
		}

		if pre_index == post_index {
			(
				usable,
				Info {
					last_index: post_index as u64,
				},
			)
		} else {
			let used = RocksDbWeight::get().writes((post_index - 1) as u64);
			(
				used,
				Info {
					last_index: post_index as u64,
				},
			)
		}
	}
}

impl Migrator {
	fn calc_weight() -> Weight {
		RocksDbWeight::get().writes((SYSTEM_ACCOUNT.len() + 100) as u64)
	}
}
