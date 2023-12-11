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
	fn remove_transfer_allowance_delay_present() -> Weight;
	fn remove_transfer_allowance_no_delay() -> Weight;
	fn purge_transfer_allowance_no_remaining_metadata() -> Weight;
	fn purge_transfer_allowance_remaining_metadata() -> Weight;
}

/// Weights for pallet_transfer_allowlist using the Substrate node and
/// recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	/// Storage: Fees FeeBalances (r:1 w:0)
	/// Proof: Fees FeeBalances (max_values: None, max_size: Some(48), added:
	/// 2523, mode: MaxEncodedLen) Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added:
	/// 2603, mode: MaxEncodedLen) Storage: Balances Holds (r:1 w:1)
	/// Proof: Balances Holds (max_values: None, max_size: Some(209), added:
	/// 2684, mode: MaxEncodedLen)
	fn add_transfer_allowance_no_existing_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `537`
		//  Estimated: `3674`
		// Minimum execution time: 88_084_000 picoseconds.
		Weight::from_parts(89_187_000, 0)
			.saturating_add(Weight::from_parts(0, 3674))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	/// Storage: Fees FeeBalances (r:1 w:0)
	/// Proof: Fees FeeBalances (max_values: None, max_size: Some(48), added:
	/// 2523, mode: MaxEncodedLen) Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added:
	/// 2603, mode: MaxEncodedLen) Storage: Balances Holds (r:1 w:1)
	/// Proof: Balances Holds (max_values: None, max_size: Some(209), added:
	/// 2684, mode: MaxEncodedLen)
	fn add_transfer_allowance_existing_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `630`
		//  Estimated: `3674`
		// Minimum execution time: 90_499_000 picoseconds.
		Weight::from_parts(91_682_000, 0)
			.saturating_add(Weight::from_parts(0, 3674))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn add_allowance_delay_no_existing_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `213`
		//  Estimated: `3556`
		// Minimum execution time: 20_037_000 picoseconds.
		Weight::from_parts(20_819_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn add_allowance_delay_existing_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `336`
		//  Estimated: `3556`
		// Minimum execution time: 22_913_000 picoseconds.
		Weight::from_parts(23_655_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn toggle_allowance_delay_once_future_modifiable() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `306`
		//  Estimated: `3556`
		// Minimum execution time: 22_973_000 picoseconds.
		Weight::from_parts(23_624_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn update_allowance_delay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `310`
		//  Estimated: `3556`
		// Minimum execution time: 23_153_000 picoseconds.
		Weight::from_parts(23_554_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn purge_allowance_delay_no_remaining_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `310`
		//  Estimated: `3556`
		// Minimum execution time: 22_642_000 picoseconds.
		Weight::from_parts(23_514_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn purge_allowance_delay_remaining_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `344`
		//  Estimated: `3556`
		// Minimum execution time: 24_074_000 picoseconds.
		Weight::from_parts(24_555_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	fn remove_transfer_allowance_delay_present() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `438`
		//  Estimated: `3596`
		// Minimum execution time: 34_765_000 picoseconds.
		Weight::from_parts(35_746_000, 0)
			.saturating_add(Weight::from_parts(0, 3596))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	fn remove_transfer_allowance_no_delay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `434`
		//  Estimated: `3596`
		// Minimum execution time: 34_605_000 picoseconds.
		Weight::from_parts(35_526_000, 0)
			.saturating_add(Weight::from_parts(0, 3596))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	/// Storage: Fees FeeBalances (r:1 w:0)
	/// Proof: Fees FeeBalances (max_values: None, max_size: Some(48), added:
	/// 2523, mode: MaxEncodedLen) Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added:
	/// 2603, mode: MaxEncodedLen) Storage: Balances Holds (r:1 w:1)
	/// Proof: Balances Holds (max_values: None, max_size: Some(209), added:
	/// 2684, mode: MaxEncodedLen) Storage: TransferAllowList
	/// AccountCurrencyTransferCountDelay (r:1 w:1) Proof: TransferAllowList
	/// AccountCurrencyTransferCountDelay (max_values: None, max_size: Some(91),
	/// added: 2566, mode: MaxEncodedLen)
	fn purge_transfer_allowance_no_remaining_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `811`
		//  Estimated: `3674`
		// Minimum execution time: 81_633_000 picoseconds.
		Weight::from_parts(84_177_000, 0)
			.saturating_add(Weight::from_parts(0, 3674))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	/// Storage: Fees FeeBalances (r:1 w:0)
	/// Proof: Fees FeeBalances (max_values: None, max_size: Some(48), added:
	/// 2523, mode: MaxEncodedLen) Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added:
	/// 2603, mode: MaxEncodedLen) Storage: Balances Holds (r:1 w:1)
	/// Proof: Balances Holds (max_values: None, max_size: Some(209), added:
	/// 2684, mode: MaxEncodedLen) Storage: TransferAllowList
	/// AccountCurrencyTransferCountDelay (r:1 w:1) Proof: TransferAllowList
	/// AccountCurrencyTransferCountDelay (max_values: None, max_size: Some(91),
	/// added: 2566, mode: MaxEncodedLen)
	fn purge_transfer_allowance_remaining_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `850`
		//  Estimated: `3674`
		// Minimum execution time: 82_524_000 picoseconds.
		Weight::from_parts(83_476_000, 0)
			.saturating_add(Weight::from_parts(0, 3674))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}
}

impl WeightInfo for () {
	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	/// Storage: Fees FeeBalances (r:1 w:0)
	/// Proof: Fees FeeBalances (max_values: None, max_size: Some(48), added:
	/// 2523, mode: MaxEncodedLen) Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added:
	/// 2603, mode: MaxEncodedLen) Storage: Balances Holds (r:1 w:1)
	/// Proof: Balances Holds (max_values: None, max_size: Some(209), added:
	/// 2684, mode: MaxEncodedLen)
	fn add_transfer_allowance_no_existing_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `537`
		//  Estimated: `3674`
		// Minimum execution time: 88_084_000 picoseconds.
		Weight::from_parts(89_187_000, 0)
			.saturating_add(Weight::from_parts(0, 3674))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	/// Storage: Fees FeeBalances (r:1 w:0)
	/// Proof: Fees FeeBalances (max_values: None, max_size: Some(48), added:
	/// 2523, mode: MaxEncodedLen) Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added:
	/// 2603, mode: MaxEncodedLen) Storage: Balances Holds (r:1 w:1)
	/// Proof: Balances Holds (max_values: None, max_size: Some(209), added:
	/// 2684, mode: MaxEncodedLen)
	fn add_transfer_allowance_existing_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `630`
		//  Estimated: `3674`
		// Minimum execution time: 90_499_000 picoseconds.
		Weight::from_parts(91_682_000, 0)
			.saturating_add(Weight::from_parts(0, 3674))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn add_allowance_delay_no_existing_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `213`
		//  Estimated: `3556`
		// Minimum execution time: 20_037_000 picoseconds.
		Weight::from_parts(20_819_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn add_allowance_delay_existing_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `336`
		//  Estimated: `3556`
		// Minimum execution time: 22_913_000 picoseconds.
		Weight::from_parts(23_655_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn toggle_allowance_delay_once_future_modifiable() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `306`
		//  Estimated: `3556`
		// Minimum execution time: 22_973_000 picoseconds.
		Weight::from_parts(23_624_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn update_allowance_delay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `310`
		//  Estimated: `3556`
		// Minimum execution time: 23_153_000 picoseconds.
		Weight::from_parts(23_554_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn purge_allowance_delay_no_remaining_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `310`
		//  Estimated: `3556`
		// Minimum execution time: 22_642_000 picoseconds.
		Weight::from_parts(23_514_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	fn purge_allowance_delay_remaining_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `344`
		//  Estimated: `3556`
		// Minimum execution time: 24_074_000 picoseconds.
		Weight::from_parts(24_555_000, 0)
			.saturating_add(Weight::from_parts(0, 3556))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	fn remove_transfer_allowance_delay_present() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `438`
		//  Estimated: `3596`
		// Minimum execution time: 34_765_000 picoseconds.
		Weight::from_parts(35_746_000, 0)
			.saturating_add(Weight::from_parts(0, 3596))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferCountDelay (r:1 w:0)
	/// Proof: TransferAllowList AccountCurrencyTransferCountDelay (max_values:
	/// None, max_size: Some(91), added: 2566, mode: MaxEncodedLen)
	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	fn remove_transfer_allowance_no_delay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `434`
		//  Estimated: `3596`
		// Minimum execution time: 34_605_000 picoseconds.
		Weight::from_parts(35_526_000, 0)
			.saturating_add(Weight::from_parts(0, 3596))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	/// Storage: Fees FeeBalances (r:1 w:0)
	/// Proof: Fees FeeBalances (max_values: None, max_size: Some(48), added:
	/// 2523, mode: MaxEncodedLen) Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added:
	/// 2603, mode: MaxEncodedLen) Storage: Balances Holds (r:1 w:1)
	/// Proof: Balances Holds (max_values: None, max_size: Some(209), added:
	/// 2684, mode: MaxEncodedLen) Storage: TransferAllowList
	/// AccountCurrencyTransferCountDelay (r:1 w:1) Proof: TransferAllowList
	/// AccountCurrencyTransferCountDelay (max_values: None, max_size: Some(91),
	/// added: 2566, mode: MaxEncodedLen)
	fn purge_transfer_allowance_no_remaining_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `811`
		//  Estimated: `3674`
		// Minimum execution time: 81_633_000 picoseconds.
		Weight::from_parts(84_177_000, 0)
			.saturating_add(Weight::from_parts(0, 3674))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}

	/// Storage: TransferAllowList AccountCurrencyTransferAllowance (r:1 w:1)
	/// Proof: TransferAllowList AccountCurrencyTransferAllowance (max_values:
	/// None, max_size: Some(131), added: 2606, mode: MaxEncodedLen)
	/// Storage: Fees FeeBalances (r:1 w:0)
	/// Proof: Fees FeeBalances (max_values: None, max_size: Some(48), added:
	/// 2523, mode: MaxEncodedLen) Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added:
	/// 2603, mode: MaxEncodedLen) Storage: Balances Holds (r:1 w:1)
	/// Proof: Balances Holds (max_values: None, max_size: Some(209), added:
	/// 2684, mode: MaxEncodedLen) Storage: TransferAllowList
	/// AccountCurrencyTransferCountDelay (r:1 w:1) Proof: TransferAllowList
	/// AccountCurrencyTransferCountDelay (max_values: None, max_size: Some(91),
	/// added: 2566, mode: MaxEncodedLen)
	fn purge_transfer_allowance_remaining_metadata() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `850`
		//  Estimated: `3674`
		// Minimum execution time: 82_524_000 picoseconds.
		Weight::from_parts(83_476_000, 0)
			.saturating_add(Weight::from_parts(0, 3674))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}
}
