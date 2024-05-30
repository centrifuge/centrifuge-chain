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

use frame_support::pallet_prelude::ValueQuery;
use frame_support::{storage_alias, Blake2_128Concat, BoundedVec, Parameter};

use cfg_primitives::{AccountId, Balance};
use frame_support::traits::{ConstU32, Get, Len, OnRuntimeUpgrade, VariantCountOf};
use pallet_balances::IdAmount;
use pallet_order_book::weights::Weight;
use pallet_transfer_allowlist::HoldReason;
#[cfg(feature = "try-runtime")]
use parity_scale_codec::{Decode, Encode};
use parity_scale_codec::{FullCodec, FullEncode};
use sp_runtime::traits::Member;
use sp_runtime::SaturatedConversion;
#[cfg(feature = "try-runtime")]
use sp_runtime::Saturating;
#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;
use sp_std::vec;
use sp_std::vec::Vec;

const LOG_PREFIX: &str = "MigrateTransferAllowList HoldReason:";

pub struct MigrateTransferAllowListHolds<T, RuntimeHoldReason>(
	sp_std::marker::PhantomData<(T, RuntimeHoldReason)>,
);

type OldHolds = BoundedVec<IdAmount<(), Balance>, ConstU32<10>>;
type NewHolds<RuntimeHoldReason> =
	BoundedVec<IdAmount<RuntimeHoldReason, Balance>, VariantCountOf<RuntimeHoldReason>>;

#[storage_alias]
pub type Holds<T: pallet_balances::Config> =
	StorageMap<pallet_balances::Pallet<T>, Blake2_128Concat, AccountId, OldHolds, ValueQuery>;

impl<T, RuntimeHoldReason> OnRuntimeUpgrade for MigrateTransferAllowListHolds<T, RuntimeHoldReason>
where
	T: pallet_balances::Config<Balance = Balance, RuntimeHoldReason = RuntimeHoldReason>
		+ pallet_transfer_allowlist::Config
		+ frame_system::Config<AccountId = AccountId>,
	<T as pallet_balances::Config>::RuntimeHoldReason: From<pallet_transfer_allowlist::HoldReason>,
	RuntimeHoldReason: frame_support::traits::VariantCount
		+ FullCodec
		+ FullEncode
		+ Parameter
		+ Member
		+ sp_std::fmt::Debug,
{
	fn on_runtime_upgrade() -> Weight {
		let transfer_allowlist_accounts: Vec<AccountId> =
			pallet_transfer_allowlist::AccountCurrencyTransferAllowance::<T>::iter_keys()
				.map(|(a, _, _)| a)
				.collect();
		let mut weight =
			T::DbWeight::get().reads(transfer_allowlist_accounts.len().saturated_into());

		pallet_balances::Holds::<T>::translate::<OldHolds, _>(|who, holds| {
			if Self::account_can_be_migrated(&who, &transfer_allowlist_accounts) {
				log::info!("{LOG_PREFIX} Migrating hold for account {who:?}");
				let id_amount = IdAmount::<RuntimeHoldReason, Balance> {
					id: HoldReason::TransferAllowance.into(),
					// Non-emptiness ensured above
					amount: holds[0].amount,
				};
				weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));

				Some(NewHolds::truncate_from(vec![id_amount]))
			} else {
				None
			}
		});

		log::info!(
			"{LOG_PREFIX} migrated {:?} accounts",
			transfer_allowlist_accounts.len()
		);

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
		let transfer_allowlist_accounts =
			pallet_transfer_allowlist::AccountCurrencyTransferAllowance::<T>::iter_keys()
				.map(|(a, _, _)| a)
				.collect();

		let mut n = 0u64;
		for (acc, _) in Holds::<T>::iter() {
			assert!(Self::account_can_be_migrated(
				&acc,
				&transfer_allowlist_accounts
			));
			n.saturating_accrue(1);
		}

		log::info!("{LOG_PREFIX} pre checks done");
		Ok(n.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(pre_state: Vec<u8>) -> Result<(), TryRuntimeError> {
		let count_pre: u64 = Decode::decode(&mut pre_state.as_slice())
			.expect("pre_upgrade provides a valid state; qed");

		let holds_post = pallet_balances::Holds::<T>::iter();
		let count_post: u64 = holds_post.count().saturated_into();
		assert_eq!(count_pre, count_post);

		for (_, hold) in pallet_balances::Holds::<T>::iter() {
			assert_eq!(hold.len(), 1);
			assert_eq!(hold[0].id, HoldReason::TransferAllowance.into());
		}

		log::info!("{LOG_PREFIX} post checks done");
		Ok(())
	}
}

impl<T, RuntimeHoldReason> MigrateTransferAllowListHolds<T, RuntimeHoldReason>
where
	T: pallet_balances::Config
		+ pallet_transfer_allowlist::Config
		+ frame_system::Config<AccountId = AccountId>,
{
	fn account_can_be_migrated(who: &AccountId, whitelist: &Vec<AccountId>) -> bool {
		if !whitelist.iter().any(|a| a == who) {
			log::warn!("{LOG_PREFIX} Account {who:?} is skipped due to missing AccountCurrencyTransferAllowance storage entry");
			return false;
		}

		match Holds::<T>::get(who) {
			holds if holds.len() == 1 => holds.into_inner()[0].id == (),
			_ => {
				log::warn!("{LOG_PREFIX} Account {who:?} does not meet Hold storage assumptions");
				false
			}
		}
	}
}
