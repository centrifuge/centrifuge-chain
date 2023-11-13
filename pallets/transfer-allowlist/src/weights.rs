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

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn add_transfer_allowance_no_existing_metadata() -> Weight;
	fn add_transfer_allowance_existing_metadata() -> Weight;
	fn add_allowance_delay_no_existing_metadata() -> Weight;
	fn add_allowance_delay_existing_metadata() -> Weight;
	fn toggle_allowance_delay_once_future_modifiable() -> Weight;
	fn update_allowance_delay() -> Weight;
	fn purge_allowance_delay_remaining_metadata() -> Weight;
	fn purge_allowance_delay_no_remaining_metadata() -> Weight;
	fn remove_transfer_allowance_missing_allowance() -> Weight;
	fn remove_transfer_allowance_delay_present() -> Weight;
	fn remove_transfer_allowance_no_delay() -> Weight;
	fn purge_transfer_allowance_no_remaining_metadata() -> Weight;
	fn purge_transfer_allowance_remaining_metadata() -> Weight;
}

/// Weights for pallet_transfer_allowlist using the Substrate node and
/// recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	// Storage: Fees FeeBalances (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	fn add_transfer_allowance_no_existing_metadata() -> Weight {
		// Minimum execution time: 40_000 nanoseconds.
		Weight::from_parts(41_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	// Storage: Fees FeeBalances (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	fn add_transfer_allowance_existing_metadata() -> Weight {
		// Minimum execution time: 43_000 nanoseconds.
		Weight::from_parts(43_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn add_allowance_delay_no_existing_metadata() -> Weight {
		// Minimum execution time: 18_000 nanoseconds.
		Weight::from_parts(18_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn add_allowance_delay_existing_metadata() -> Weight {
		// Minimum execution time: 19_000 nanoseconds.
		Weight::from_parts(20_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn toggle_allowance_delay_once_future_modifiable() -> Weight {
		// Minimum execution time: 20_000 nanoseconds.
		Weight::from_parts(20_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn update_allowance_delay() -> Weight {
		// Minimum execution time: 20_000 nanoseconds.
		Weight::from_parts(21_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn purge_allowance_delay_no_remaining_metadata() -> Weight {
		// Minimum execution time: 20_000 nanoseconds.
		Weight::from_parts(21_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn purge_allowance_delay_remaining_metadata() -> Weight {
		// Minimum execution time: 20_000 nanoseconds.
		Weight::from_parts(21_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	fn remove_transfer_allowance_missing_allowance() -> Weight {
		// Minimum execution time: 26_000 nanoseconds.
		Weight::from_parts(27_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	fn remove_transfer_allowance_delay_present() -> Weight {
		// Minimum execution time: 26_000 nanoseconds.
		Weight::from_parts(27_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	fn remove_transfer_allowance_no_delay() -> Weight {
		// Minimum execution time: 26_000 nanoseconds.
		Weight::from_parts(27_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	// Storage: Fees FeeBalances (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn purge_transfer_allowance_no_remaining_metadata() -> Weight {
		// Minimum execution time: 43_000 nanoseconds.
		Weight::from_parts(43_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}

	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	// Storage: Fees FeeBalances (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn purge_transfer_allowance_remaining_metadata() -> Weight {
		// Minimum execution time: 43_000 nanoseconds.
		Weight::from_parts(44_000_000, 0)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
}

impl WeightInfo for () {
	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	// Storage: Fees FeeBalances (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	fn add_transfer_allowance_no_existing_metadata() -> Weight {
		// Minimum execution time: 40_000 nanoseconds.
		Weight::from_parts(41_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(3))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	// Storage: Fees FeeBalances (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	fn add_transfer_allowance_existing_metadata() -> Weight {
		// Minimum execution time: 43_000 nanoseconds.
		Weight::from_parts(43_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(3))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn add_allowance_delay_no_existing_metadata() -> Weight {
		// Minimum execution time: 18_000 nanoseconds.
		Weight::from_parts(18_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn add_allowance_delay_existing_metadata() -> Weight {
		// Minimum execution time: 19_000 nanoseconds.
		Weight::from_parts(20_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn toggle_allowance_delay_once_future_modifiable() -> Weight {
		// Minimum execution time: 20_000 nanoseconds.
		Weight::from_parts(20_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn update_allowance_delay() -> Weight {
		// Minimum execution time: 20_000 nanoseconds.
		Weight::from_parts(21_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn purge_allowance_delay_no_remaining_metadata() -> Weight {
		// Minimum execution time: 20_000 nanoseconds.
		Weight::from_parts(21_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn purge_allowance_delay_remaining_metadata() -> Weight {
		// Minimum execution time: 20_000 nanoseconds.
		Weight::from_parts(21_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	fn remove_transfer_allowance_missing_allowance() -> Weight {
		// Minimum execution time: 26_000 nanoseconds.
		Weight::from_parts(27_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	fn remove_transfer_allowance_delay_present() -> Weight {
		// Minimum execution time: 26_000 nanoseconds.
		Weight::from_parts(27_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	fn remove_transfer_allowance_no_delay() -> Weight {
		// Minimum execution time: 26_000 nanoseconds.
		Weight::from_parts(27_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	// Storage: Fees FeeBalances (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn purge_transfer_allowance_no_remaining_metadata() -> Weight {
		// Minimum execution time: 43_000 nanoseconds.
		Weight::from_parts(43_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(3))
	}

	// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	// Storage: Fees FeeBalances (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	fn purge_transfer_allowance_remaining_metadata() -> Weight {
		// Minimum execution time: 43_000 nanoseconds.
		Weight::from_parts(44_000_000, 0)
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
}
