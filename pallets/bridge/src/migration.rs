// Copyright 2022 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
use super::*;

pub mod fix_pallet_account {
	use cfg_primitives::AccountId;
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::{log, weights::Weight};
	use sp_runtime::traits::{AccountIdConversion, Get, Zero};
	use sp_std::vec;

	use super::*; // Not in prelude for try-runtime

	const WRONG_INBOUND_ID: PalletId = PalletId(*b"cb/bridg");
	const WRONG_OUTBOUND_ID: PalletId = cfg_types::ids::BRIDGE_PALLET_ID;

	#[cfg(feature = "try-runtime")]
	pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
		Ok(())
	}

	pub fn migrate<T: Config>() -> Weight
	where
		<T as frame_system::Config>::AccountId: From<AccountId>,
	{
		let mut weight = Weight::from_ref_time(0);
		log::info!("pallet_bridge: initiating migration to move funds from wrong bridge accounts");

		let correct_bridge_account: T::AccountId =
			cfg_types::ids::CHAIN_BRIDGE_PALLET_ID.into_account_truncating();
		let wrong_accounts = vec![
			WRONG_INBOUND_ID.into_account_truncating(),
			WRONG_OUTBOUND_ID.into_account_truncating(),
		];

		wrong_accounts.iter().for_each(|x| {
			let balance = T::Currency::free_balance(&x);

			// Transfer the balance of the wrong account to the correct one if there's balance
			// to be moved; this works as a simple check to stop us from running this migration
			// more than once.
			if balance > Zero::zero() {
				log::info!(
					"pallet_bridge: will move balance from the wrong account {:?}",
					x
				);
				let res = T::Currency::transfer(&x, &correct_bridge_account, balance, AllowDeath);

				match res {
					Ok(_) => log::info!("pallet_bridge: balance migration succeeded"),
					Err(err) => {
						log::error!("pallet_bridge: balance migration failed with {:?}", err)
					}
				}
			}
			weight += T::DbWeight::get().reads_writes(2, 1);
		});

		weight
	}

	/// Ensure that the wrong accounts' balance is zero after the migration has been executed.
	#[cfg(feature = "try-runtime")]
	pub fn post_migrate<T: Config>() -> Result<(), &'static str> {
		let wrong_account_inbound = WRONG_INBOUND_ID.into_account_truncating();
		ensure!(
			T::Currency::free_balance(&wrong_account_inbound) == Zero::zero(),
			"Inbound account still has balance"
		);

		let wrong_account_outbound = WRONG_OUTBOUND_ID.into_account_truncating();
		ensure!(
			T::Currency::free_balance(&wrong_account_outbound) == Zero::zero(),
			"Outbound account still has balance"
		);

		Ok(())
	}
}
