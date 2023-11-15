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

use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use sp_runtime::DispatchError;

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

pub struct Migration<T: pallet_balances::Config>(sp_std::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for Migration<T>
where
	T: pallet_balances::Config,
{
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
		// CHECKING DECODING OLD DATASTRUCTURE WITH NEW LAYOUT WORKS:
		// * Fetch storage from chain with NEW data structure
		// * Check if fetched accounts matches on-chain storage entries

		// TODO:
		// * Research whether orml-pallets datastructure also changed
		// * Research whether we need to migrate locks in orml (maybe first check if
		//   there exist any. ^^

		let accounts = frame_system::Account::<T>::full_storage_key;

		Ok(Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		// WE CAN NOT MIGRATE. THIS CODE IS JUST FOR CHECKING IF WE NEED ANYTHING
		// BESIDES THE LAZY MIGRATION FROM PARITY
		// See: https://kflabs.slack.com/archives/C05TBFRBL15/p1699956615421249
		Weight::from_parts(0, 0)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), DispatchError> {
		Ok(())
	}
}
