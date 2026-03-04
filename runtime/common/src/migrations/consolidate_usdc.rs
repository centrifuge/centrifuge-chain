// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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

use cfg_types::tokens::CurrencyId;
use frame_support::{
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
};
use orml_tokens::AccountData;
use sp_arithmetic::traits::Saturating;
use sp_runtime::AccountId32;
#[cfg(feature = "try-runtime")]
use {frame_support::pallet_prelude::*, sp_std::vec::Vec};

const LOG_PREFIX: &str = "ConsolidateUSDC";
const USDC_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(6);
const DESTINATION: [u8; 32] =
	hex_literal::hex!("684f4dc6a026ea82a6cb36de4330a1a44428bbe243fb7f26ccf6227b0d0ef054");

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for Migration<T>
where
	T: frame_system::Config<AccountId = AccountId32> + orml_tokens::Config<CurrencyId = CurrencyId>,
	T::Balance: From<u128> + Into<u128>,
{
	fn on_runtime_upgrade() -> Weight {
		let dest = AccountId32::from(DESTINATION);
		let mut total_moved: u128 = 0;
		let mut accounts_drained: u64 = 0;
		let mut reads: u64 = 0;
		let mut writes: u64 = 0;

		// Collect all USDC holders (except destination) with their balances
		let holders: sp_std::vec::Vec<(AccountId32, AccountData<T::Balance>)> =
			orml_tokens::Accounts::<T>::iter()
				.filter_map(|(account, currency, data)| {
					reads.saturating_accrue(1);
					if currency == USDC_CURRENCY_ID && account != dest {
						Some((account, data))
					} else {
						None
					}
				})
				.collect();

		for (account, data) in holders {
			let free: u128 = data.free.into();
			let reserved: u128 = data.reserved.into();
			let amount = free.saturating_add(reserved);

			total_moved.saturating_accrue(amount);
			accounts_drained.saturating_accrue(1);

			// Clear the source account's USDC balance
			orml_tokens::Accounts::<T>::remove(&account, USDC_CURRENCY_ID);
			writes.saturating_accrue(1);

			// Also clear any locks and reserves for this account+currency
			orml_tokens::Locks::<T>::remove(&account, USDC_CURRENCY_ID);
			orml_tokens::Reserves::<T>::remove(&account, USDC_CURRENCY_ID);
			writes.saturating_accrue(2);

			log::info!(
				"{LOG_PREFIX}: Drained {amount} USDC (free: {free}, reserved: {reserved}) from {account:?}",
			);
		}

		if total_moved > 0 {
			// Credit all drained USDC to the destination as free balance
			orml_tokens::Accounts::<T>::mutate(&dest, USDC_CURRENCY_ID, |data| {
				let existing_free: u128 = data.free.into();
				let existing_reserved: u128 = data.reserved.into();
				data.free = existing_free
					.saturating_add(existing_reserved)
					.saturating_add(total_moved)
					.into();
				data.reserved = 0u128.into();
				data.frozen = 0u128.into();
			});
			reads.saturating_accrue(1);
			writes.saturating_accrue(1);
		}

		log::info!(
			"{LOG_PREFIX}: Migration done! Moved {total_moved} USDC from {accounts_drained} accounts to {dest:?}",
		);

		T::DbWeight::get().reads_writes(reads, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		let total: u128 = orml_tokens::Accounts::<T>::iter()
			.filter(|(_, currency, _)| *currency == USDC_CURRENCY_ID)
			.map(|(_, _, data)| {
				let free: u128 = data.free.into();
				let reserved: u128 = data.reserved.into();
				free.saturating_add(reserved)
			})
			.fold(0u128, |acc, x| acc.saturating_add(x));

		let holder_count = orml_tokens::Accounts::<T>::iter()
			.filter(|(_, currency, _)| *currency == USDC_CURRENCY_ID)
			.count() as u64;

		log::info!("{LOG_PREFIX}: Pre-upgrade: {holder_count} USDC holders with total {total}");

		Ok((total, holder_count).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		let (pre_total, pre_count): (u128, u64) =
			Decode::decode(&mut pre_state.as_slice()).expect("pre_upgrade provides valid state");

		let remaining_holders: u64 = orml_tokens::Accounts::<T>::iter()
			.filter(|(_, currency, _)| *currency == USDC_CURRENCY_ID)
			.count() as u64;

		if pre_count == 0 {
			assert_eq!(
				remaining_holders, 0,
				"No USDC holders expected when pre-migration had none"
			);
			log::info!("{LOG_PREFIX}: Post-upgrade: No USDC holders found, nothing to do");
			return Ok(());
		}

		let dest = AccountId32::from(DESTINATION);
		let dest_data = orml_tokens::Accounts::<T>::get(&dest, USDC_CURRENCY_ID);
		let dest_balance: u128 = dest_data.free.into();
		let dest_reserved: u128 = dest_data.reserved.into();

		// All USDC should now be in destination's free balance
		assert_eq!(
			dest_balance,
			pre_total,
			"Destination free balance ({dest_balance}) should equal pre-migration total ({pre_total})"
		);
		assert_eq!(
			dest_reserved, 0,
			"Destination should have no reserved balance"
		);

		// Only the destination should hold USDC now
		assert_eq!(
			remaining_holders, 1,
			"Only destination should hold USDC, found {remaining_holders}"
		);

		log::info!("{LOG_PREFIX}: Post-upgrade: All {pre_total} USDC consolidated to {dest:?}");

		Ok(())
	}
}
