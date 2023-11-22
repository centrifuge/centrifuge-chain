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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::TypeInfo, storage::unhashed, traits::OnRuntimeUpgrade, weights::Weight, RuntimeDebug,
	StoragePrefixedMap,
};
use frame_system::AccountInfo;
use pallet_balances::AccountData;
use sp_arithmetic::traits::Zero;
use sp_core::crypto::AccountId32;
pub use sp_core::sr25519;
#[cfg(feature = "try-runtime")]
use sp_runtime::DispatchError;
#[cfg(feature = "try-runtime")]
use sp_std::{prelude::Vec, vec};

/// All balance information for an account.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct OldAccountData<Balance> {
	/// Non-reserved part of the balance. There may still be restrictions on
	/// this, but it is the total pool what may in principle be transferred,
	/// reserved and used for tipping.
	///
	/// This is the only balance that matters in terms of most operations on
	/// tokens. It alone is used to determine the balance when in the contract
	/// execution environment.
	pub free: Balance,
	/// Balance which is reserved and may not be used at all.
	///
	/// This can still get slashed, but gets slashed last of all.
	///
	/// This balance is a 'reserve' balance that other subsystems use in order
	/// to set aside tokens that are still 'owned' by the account holder, but
	/// which are suspendable. This includes named reserve and unnamed reserve.
	pub reserved: Balance,
	/// The amount that `free` may not drop below when withdrawing for *anything
	/// except transaction fee payment*.
	pub misc_frozen: Balance,
	/// The amount that `free` may not drop below when withdrawing specifically
	/// for transaction fee payment.
	pub fee_frozen: Balance,
}

pub type OldAccountInfoOf<T> = AccountInfo<
	<T as frame_system::Config>::Index,
	OldAccountData<<T as pallet_balances::Config>::Balance>,
>;

pub type NewAccountData<Balance> = pallet_balances::AccountData<Balance>;

pub type NewAccountInfoOf<T> = AccountInfo<
	<T as frame_system::Config>::Index,
	NewAccountData<<T as pallet_balances::Config>::Balance>,
>;

pub struct Migration<T: pallet_balances::Config + frame_system::Config>(
	sp_std::marker::PhantomData<T>,
);

impl<T> OnRuntimeUpgrade for Migration<T>
where
	T: frame_system::Config<AccountId = AccountId32, Index = u32>
		+ pallet_balances::Config<Balance = u128>,
	NewAccountInfoOf<T>:
		codec::EncodeLike<AccountInfo<u32, <T as frame_system::Config>::AccountData>>,
{
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
		check_account_storage::<T>(true);

		Ok(vec![])
	}

	fn on_runtime_upgrade() -> Weight {
		// !!! REMOVE TEST ACCOUNT DATA !!!
		//
		// TEST ACCOUNT DATA - START
		let test_account_data = get_test_account_data::<T>();

		for (account_id, account_data) in test_account_data {
			log::info!(
				"Balances Migration - Processing account id - {}",
				hex::encode(account_id.clone())
			);

			frame_system::Account::<T>::insert(account_id, account_data);
		}

		// TEST ACCOUNT DATA - END

		// WE CAN NOT MIGRATE. THIS CODE IS JUST FOR CHECKING IF WE NEED ANYTHING
		// BESIDES THE LAZY MIGRATION FROM PARITY
		// See: https://kflabs.slack.com/archives/C05TBFRBL15/p1699956615421249
		Weight::from_parts(0, 0)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), DispatchError> {
		check_account_storage::<T>(false);

		Ok(())
	}
}

/// The difference between the old and new account data structures
/// is the last field of the struct, which is:
//
/// * Old - `free_frozen` - T::Balance
/// * New - `flags` - u128
//
/// During this check, we confirm that the old or the new AccountInfo can
/// be successfully decoded given the raw data found in storage, and
/// we add specific checks for each version:
//
/// * Old - confirm that `fee_frozen` is zero, as it shouldn't have been used so
///   far
/// * New - confirm that `flags` does not have the new logic flag set because
///   this requires calling
///   `pallet_balances::Pallet::<T>::upgrade_account(origin, &who)` which can be
///   done by a script
fn check_account_storage<T: frame_system::Config + pallet_balances::Config>(is_old: bool) {
	let account_prefix = frame_system::Account::<T>::final_prefix();

	let mut total_count = 0;
	let mut total_reserved = 0;
	let mut total_frozen = 0;
	let mut total_misc_frozen = 0;
	let mut total_flags = 0;

	let prefix = account_prefix.to_vec();
	let mut previous_key = prefix.clone();

	while let Some(next) = sp_io::storage::next_key(&previous_key) {
		if !next.starts_with(&prefix) {
			break;
		}

		previous_key = next;

		log::info!(
			"Balances Migration - Processing account key - {}",
			hex::encode(previous_key.clone())
		);

		if is_old {
			if let Some(info) = unhashed::get::<OldAccountInfoOf<T>>(&previous_key) {
				if !info.data.reserved.is_zero() {
					total_reserved += 1;
				}
				if !info.data.fee_frozen.is_zero() {
					total_frozen += 1;
				}
				if !info.data.misc_frozen.is_zero() {
					total_misc_frozen += 1;
				}
			} else {
				log::error!("Balances Migration - Error decoding OLD AccountInfo")
			}
		} else {
			if let Some(info) = unhashed::get::<NewAccountInfoOf<T>>(&previous_key) {
				if !info.data.reserved.is_zero() {
					total_reserved += 1;
				}
				if !info.data.frozen.is_zero() {
					total_frozen += 1;
				}
				if info.data.flags.is_new_logic() {
					log::warn!("Balances Migration - New account data with new logic flag enabled");
					total_flags += 1;
				}
			} else {
				log::error!("Balances Migration - Error decoding NEW AccountInfo")
			}
		}
		total_count += 1;
	}

	log::info!("Balances Migration - Total accounts - {}", total_count);
	log::info!(
		"Balances Migration - Total accounts with reserved balances - {}",
		total_reserved
	);
	log::info!(
		"Balances Migration - Total accounts with fee frozen balances - {}",
		total_frozen
	);
	if is_old {
		log::info!(
			"Balances Migration - Total accounts with misc frozen balances - {}",
			total_misc_frozen
		);
	} else {
		log::info!(
			"Balances Migration - Total accounts with flags set to new logic - {}",
			total_flags
		);
	}
}

/// MUST NOT BE PART OF RELEASE!
///
/// Adds five dummy accounts with reserved and or frozen fee.
///
/// NOTE: Unfortunately, flags needs to be initialized with the default
/// (correct) value.
#[cfg(feature = "try-runtime")]
fn get_test_account_data<T>() -> Vec<(T::AccountId, NewAccountInfoOf<T>)>
where
	T: frame_system::Config<AccountId = AccountId32, Index = u32>
		+ pallet_balances::Config<Balance = u128>,
	NewAccountInfoOf<T>:
		codec::EncodeLike<AccountInfo<u32, <T as frame_system::Config>::AccountData>>,
{
	vec![
		(
			[1u8; 32].into(),
			NewAccountInfoOf::<T> {
				nonce: 0,
				consumers: 0,
				providers: 1,
				sufficients: 0,
				data: AccountData::<T::Balance> {
					free: 1_000_000_000_000,
					reserved: 0,
					frozen: 1_000_000_000_000,
					flags: Default::default(),
				},
			},
		),
		(
			[2u8; 32].into(),
			NewAccountInfoOf::<T> {
				nonce: 0,
				consumers: 0,
				providers: 1,
				sufficients: 0,
				data: AccountData::<T::Balance> {
					free: 1_000_000_000_000,
					reserved: 1_000_000_000_000,
					frozen: 0,
					flags: Default::default(),
				},
			},
		),
		(
			[3u8; 32].into(),
			NewAccountInfoOf::<T> {
				nonce: 0,
				consumers: 0,
				providers: 1,
				sufficients: 0,
				data: AccountData::<T::Balance> {
					free: 1_000_000_000_000,
					reserved: 1_000_000_000_000,
					frozen: 1_000_000_000_000,
					flags: Default::default(),
				},
			},
		),
		(
			[4u8; 32].into(),
			NewAccountInfoOf::<T> {
				nonce: 0,
				consumers: 0,
				providers: 1,
				sufficients: 0,
				data: AccountData::<T::Balance> {
					free: 3_000_000_000_000,
					reserved: 2_000_000_000_000,
					frozen: 1_000_000_000_000,
					flags: Default::default(),
				},
			},
		),
		(
			[5u8; 32].into(),
			NewAccountInfoOf::<T> {
				nonce: 1,
				consumers: 2,
				providers: 3,
				sufficients: 4,
				data: AccountData::<T::Balance> {
					free: 1_000_000_000_000,
					reserved: 1_000_000_000_000,
					frozen: 1_000_000_000_000,
					flags: Default::default(),
				},
			},
		),
	]
}
