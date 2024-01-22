// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::{AccountId, BlockNumber};
use cfg_types::{
	locations::Location,
	tokens::{CurrencyId, FilterCurrency},
};
use frame_support::{
	pallet_prelude::{NMapKey, OptionQuery},
	storage::types::{StorageDoubleMap, StorageNMap},
	traits::{Get, OnRuntimeUpgrade, StorageInstance},
	weights::Weight,
	Blake2_128Concat, Twox64Concat,
};
use pallet_transfer_allowlist::{AllowanceDetails, AllowanceMetadata};
#[cfg(feature = "try-runtime")]
use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "try-runtime")]
use sp_runtime::DispatchError;

const LOG_PREFIX: &str = "TransferAllowlist-CurrencyMigration: ";

struct DelayPrefix;
impl StorageInstance for DelayPrefix {
	const STORAGE_PREFIX: &'static str = "AccountCurrencyTransferCountDelay";

	fn pallet_prefix() -> &'static str {
		"TransferAllowList"
	}
}
type OldAccountCurrencyTransferCountDelay = StorageDoubleMap<
	DelayPrefix,
	Twox64Concat,
	AccountId,
	Twox64Concat,
	CurrencyId,
	AllowanceMetadata<BlockNumber>,
	OptionQuery,
>;

struct AllowancePrefix;
impl StorageInstance for AllowancePrefix {
	const STORAGE_PREFIX: &'static str = "AccountCurrencyTransferAllowance";

	fn pallet_prefix() -> &'static str {
		"TransferAllowList"
	}
}
type OldAccountCurrencyTransferAllowance = StorageNMap<
	AllowancePrefix,
	(
		NMapKey<Twox64Concat, AccountId>,
		NMapKey<Twox64Concat, CurrencyId>,
		NMapKey<Blake2_128Concat, Location>,
	),
	AllowanceDetails<BlockNumber>,
	OptionQuery,
>;

pub struct Migration<T>(sp_std::marker::PhantomData<T>);
impl<
		T: pallet_transfer_allowlist::Config<
			AccountId = AccountId,
			Location = Location,
			CurrencyId = FilterCurrency,
			BlockNumber = BlockNumber,
		>,
	> OnRuntimeUpgrade for Migration<T>
{
	fn on_runtime_upgrade() -> Weight {
		log::info!("{LOG_PREFIX} Migrating currency used started...");
		let mut counter = 0;
		OldAccountCurrencyTransferAllowance::translate::<AllowanceDetails<BlockNumber>, _>(
			|(account, currency_id, location), allowance| {
				pallet_transfer_allowlist::AccountCurrencyTransferAllowance::<T>::insert(
					(account, FilterCurrency::Specific(currency_id), location),
					allowance,
				);

				counter += 1;
				// We will remove the storage here, as we are inserting the new storage above
				None
			},
		);

		OldAccountCurrencyTransferCountDelay::translate::<AllowanceMetadata<BlockNumber>, _>(
			|account, currency_id, delay| {
				pallet_transfer_allowlist::AccountCurrencyTransferCountDelay::<T>::insert(
					account,
					FilterCurrency::Specific(currency_id),
					delay,
				);

				counter += 1;
				// We will remove the storage here, as we are inserting the new storage above
				None
			},
		);

		T::DbWeight::get().reads_writes(counter, counter.saturating_mul(2))
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, DispatchError> {
		log::info!("{LOG_PREFIX} PRE UPGRADE: Starting...");

		let count_allowance = OldAccountCurrencyTransferAllowance::iter().count() as u64;
		log::info!("{LOG_PREFIX} Counted {count_allowance} keys in old allowance storage.");

		let count_delay = OldAccountCurrencyTransferCountDelay::iter().count() as u64;
		log::info!("{LOG_PREFIX} Counted {count_delay} keys in old delay storage.");

		Ok((count_allowance, count_delay).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: sp_std::vec::Vec<u8>) -> Result<(), DispatchError> {
		log::info!("{LOG_PREFIX} POST UPGRADE: Starting...");
		let (count_allowance_pre, count_delay_pre): (u64, u64) =
			Decode::decode(&mut state.as_slice())
				.map_err(|_| DispatchError::Other("{LOG_PREFIX} Failed decoding state."))?;

		let count_allowance_after =
			pallet_transfer_allowlist::AccountCurrencyTransferCountDelay::<T>::iter().count()
				as u64;

		if count_allowance_after != count_allowance_pre {
			log::error!("{LOG_PREFIX} Delay migration failed. Got: {count_allowance_after}, Expected: {count_allowance_pre}");
		}

		let count_delay_after =
			pallet_transfer_allowlist::AccountCurrencyTransferCountDelay::<T>::iter().count()
				as u64;

		if count_delay_after != count_delay_pre {
			log::error!("{LOG_PREFIX} Delay migration failed. Got: {count_delay_after}, Expected: {count_delay_pre}");
		}

		log::info!("{LOG_PREFIX} POST UPGRADE: Finished.");

		Ok(())
	}
}
