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

use frame_support::{
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
};
#[cfg(feature = "try-runtime")]
use sp_runtime::DispatchError;

use crate::evm::precompile::{utils::initialize_accounts, H160Addresses};

pub struct Migration<T>(sp_std::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for Migration<T>
where
	T: pallet_evm::Config,
	<T as pallet_evm::Config>::PrecompilesType: H160Addresses,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!("precompile::AccountCodes: Inserting precompile account codes");

		let (reads, writes) = initialize_accounts::<T>();

		log::info!(
			"precompile::AccountCodes: Added new {} account codes",
			writes
		);

		T::DbWeight::get().reads_writes(writes, reads)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, DispatchError> {
		Ok(sp_std::vec::Vec::<u8>::new())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), DispatchError> {
		Ok(())
	}
}
