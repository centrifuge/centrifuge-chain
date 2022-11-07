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
	use frame_support::{log, weights::Weight};
	use sp_core::crypto::Ss58Codec;
	use sp_runtime::traits::AccountIdConversion;
	use sp_std::{vec, vec::Vec};

	use super::*;

	pub fn migrate<T: Config>() -> Weight
	where
		<T as frame_system::Config>::AccountId: From<AccountId>,
	{
		log::info!("pallet_bridge: fix pallet account");

		let wrong_accounts: Vec<T::AccountId> = vec![
			"4dpEcgqFmYGRSkRhTzVZjTG4uKoiB9VBB4Qi33LH73WsWXa4",
			"4dpEcgqFor2TJw9uWSjx2JpjkNmTic2UjJAK1j9fRtcTUoRu",
		]
		.iter()
		.map(|x| {
			AccountId::from_string(x)
				.expect("Account conversion should work")
				.into()
		})
		.collect::<Vec<_>>();

		let correct_bridge_account: T::AccountId =
			cfg_types::ids::CHAIN_BRIDGE_PALLET_ID.into_account_truncating();

		wrong_accounts.iter().for_each(|x| {
			let balance = T::Currency::free_balance(&x);
			// Transfers the balance of the bad account to the correct one
			T::Currency::transfer(
				&x,
				&correct_bridge_account,
				balance,
				AllowDeath,
			)
				.expect("TODO(nuno)");
		});

		Weight::from_ref_time(0)
	}
}
