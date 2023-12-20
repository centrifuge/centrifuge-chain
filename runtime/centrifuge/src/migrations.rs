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

pub type UpgradeCentrifuge1024 = burn_unburned::Migration<super::Runtime>;

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

mod burn_unburned {
	const LOG_PREFIX: &str = "BurnUnburnedMigration: ";
	const LP_ETH_USDC: CurrencyId = CurrencyId::ForeignAsset(100_001);
	const ETH_DOMAIN: Domain = Domain::EVM(1);

	use cfg_types::{domain_address::Domain, tokens::CurrencyId};
	use frame_support::traits::{
		fungibles::Mutate,
		tokens::{Fortitude, Precision},
		OnRuntimeUpgrade,
	};
	use pallet_order_book::weights::Weight;
	use sp_runtime::traits::{Convert, Get};

	pub struct Migration<T>
	where
		T: orml_tokens::Config + frame_system::Config,
	{
		_phantom: sp_std::marker::PhantomData<T>,
	}

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: orml_tokens::Config<CurrencyId = CurrencyId> + frame_system::Config,
	{
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
			use sp_runtime::traits::Zero;

			let pre_data = orml_tokens::Accounts::<T>::get(
				<Domain as Convert<_, T::AccountId>>::convert(ETH_DOMAIN),
				LP_ETH_USDC,
			);

			if !pre_data.frozen.is_zero() || !pre_data.reserved.is_zero() {
				log::error!(
					"{LOG_PREFIX} AccountData of Ethereum domain account has non free balances..."
				);
			}

			log::info!(
				"{LOG_PREFIX} AccountData of Ethereum domain account has free balance of: {:?}",
				pre_data.free
			);

			Ok(sp_std::vec::Vec::new())
		}

		fn on_runtime_upgrade() -> Weight {
			let data = orml_tokens::Accounts::<T>::get(
				<Domain as Convert<_, T::AccountId>>::convert(ETH_DOMAIN),
				LP_ETH_USDC,
			);

			if let Err(e) = orml_tokens::Pallet::<T>::burn_from(
				LP_ETH_USDC,
				&<Domain as Convert<_, T::AccountId>>::convert(ETH_DOMAIN),
				data.free,
				Precision::Exact,
				Fortitude::Force,
			) {
				log::error!(
					"{LOG_PREFIX} Burning from Ethereum domain account failed with: {:?}. Migration failed...",
					e
				);
			} else {
				log::info!(
					"{LOG_PREFIX} Successfully burned {:?} LP_ETH_USDC from Ethereum domain account",
					data.free
				);
			}

			T::DbWeight::get().reads(1)
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			use sp_runtime::traits::Zero;

			let post_data = orml_tokens::Accounts::<T>::get(
				<Domain as Convert<_, T::AccountId>>::convert(ETH_DOMAIN),
				LP_ETH_USDC,
			);

			if !post_data.free.is_zero()
				|| !post_data.frozen.is_zero()
				|| !post_data.reserved.is_zero()
			{
				log::error!(
					"{LOG_PREFIX} AccountData of Ethereum domain account SHOULD be zero. Migration failed."
				);
			} else {
				log::info!("{LOG_PREFIX} Migration successfully finished.")
			}

			Ok(())
		}
	}
}
