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

use frame_support::traits::{
	fungibles::{Inspect, Mutate},
	Get, OnRuntimeUpgrade,
};
use sp_runtime::traits::AccountIdConversion;
use sp_std::marker::PhantomData;

use crate::{BalanceOf, Config};

pub struct FundExistentialDeposit<T, I, Currency, ED>(PhantomData<(T, I, Currency, ED)>);

fn pallet_is_funded<T, I, Currency, ED>() -> bool
where
	T: Config<I>,
	I: 'static,
	Currency: Get<<T as Config<I>>::CurrencyId>,
	ED: Get<BalanceOf<T, I>>,
{
	<<T as Config<I>>::Currency as Inspect<T::AccountId>>::balance(
		Currency::get(),
		&<T as Config<I>>::PalletId::get().into_account_truncating(),
	) >= ED::get()
}

impl<T, I, Currency, ED> OnRuntimeUpgrade for FundExistentialDeposit<T, I, Currency, ED>
where
	T: frame_system::Config + Config<I>,
	I: 'static,
	Currency: Get<<T as Config<I>>::CurrencyId>,
	ED: Get<BalanceOf<T, I>>,
{
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		assert!(!pallet_is_funded::<T, I, Currency, ED>());

		log::info!("💶 Rewards: Pre funding ED checks successful");
		Ok(vec![])
	}

	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		if !pallet_is_funded::<T, I, Currency, ED>() {
			log::info!("💶 Rewards: Initiating ED funding to sovereign pallet account");
			T::Currency::mint_into(
				Currency::get(),
				&T::PalletId::get().into_account_truncating(),
				ED::get(),
			)
			.map_err(|_| log::error!("💶 Rewards: Failed to mint ED for sovereign pallet account",))
			.ok();

			T::DbWeight::get().reads_writes(1, 1)
		} else {
			log::info!(
				"💶 Rewards: ED funding for sovereign pallet account not required anymore. 
                This probably should be removed"
			);
			T::DbWeight::get().reads_writes(1, 0)
		}
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_pre_state: Vec<u8>) -> Result<(), &'static str> {
		assert!(pallet_is_funded::<T, I, Currency, ED>());

		log::info!("💶 Rewards: Post funding ED checks successful");
		Ok(())
	}
}
