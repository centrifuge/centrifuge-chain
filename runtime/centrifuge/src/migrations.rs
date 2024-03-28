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

/// The migration set for Centrifuge @ Polkadot.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeCentrifuge1027 = migrate_anemoy_external_prices::Migration<super::Runtime>;

mod migrate_anemoy_external_prices {
	use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

	const LOG_PREFIX: &str = "MigrateAnemoyPrices:";

	/// Simply bumps the storage version of a pallet
	///
	/// NOTE: Use with caution! Must ensure beforehand that a migration is not
	/// necessary
	pub struct Migration<R>(sp_std::marker::PhantomData<R>);
	impl<R> OnRuntimeUpgrade for Migration<R> {
		fn on_runtime_upgrade() -> Weight {
			Weight::zero()
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::DispatchError> {
			Ok(sp_std::vec![])
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
			Ok(())
		}
	}
}
