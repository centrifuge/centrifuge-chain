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

use frame_support::traits::{Get, OnRuntimeUpgrade};
use pallet_order_book::weights::Weight;
use sp_io::{storage::clear_prefix, KillStorageResult};
use sp_std::vec::Vec;

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

const REMOVAL_LIMIT: u32 = 10u32;
const LOG_PREFIX: &str = "LiquidityPoolsGatewayV1";

impl<T> OnRuntimeUpgrade for Migration<T>
where
	T: frame_system::Config,
{
	fn on_runtime_upgrade() -> Weight {
		let mut weight = Weight::zero();

		match clear_prefix(&get_storage_prefix(), Some(REMOVAL_LIMIT)) {
			KillStorageResult::AllRemoved(n) => {
				log::info!("{LOG_PREFIX}: Removed {n} ForeignInvestmentInfo V1 keys");
				weight.saturating_accrue(T::DbWeight::get().writes(n.into()));
			}
			KillStorageResult::SomeRemaining(n) => {
				log::warn!("{LOG_PREFIX}: There are {n} remaining ForeignInvestmentInfo V1 keys!");
				weight.saturating_accrue(T::DbWeight::get().writes(REMOVAL_LIMIT.into()));
			}
		}

		log::info!("{LOG_PREFIX}: Migration done!");

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		log::info!("{LOG_PREFIX}: Pre checks done!");

		Ok(Vec::new())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		assert!(
			!sp_io::storage::exists(&get_storage_prefix()),
			"{LOG_PREFIX}: ForeignInvestmentV0 keys remaining!"
		);

		log::info!("{LOG_PREFIX}: Post checks done!");

		Ok(())
	}
}

/// Returns final storage key prefix of `ForeignInvestmentInfo` in v0
fn get_storage_prefix() -> Vec<u8> {
	hex_literal::hex!("464aed913919bab92f79f3c7b79d28f7efbac15e93f37811895e260605cdc487").to_vec()
}
