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

pub type UpgradeCentrifuge1024 = (burn_unburned::Migration<super::Runtime>);

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
	use sp_std::vec::Vec;
	const LP_ETH_USDC: CurrencyId = CurrencyId::ForeignAsset(100_001);
	const ETH_DOMAIN: Domain = Domain::EVM(1);

	use cfg_types::{domain_address::Domain, tokens::CurrencyId};
	use frame_support::traits::{
		fungibles::Mutate,
		tokens::{Fortitude, Precision},
		OnRuntimeUpgrade,
	};
	use pallet_order_book::weights::Weight;
	use sp_runtime::{
		traits::{Convert, Get, Zero},
		TryRuntimeError,
	};

	pub struct Migration<T>
	where
		T: orml_tokens::Config + frame_system::Config,
		<T as orml_tokens::Config>::CurrencyId: From<CurrencyId>,
	{
		_phantom: sp_std::marker::PhantomData<T>,
	}

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: orml_tokens::Config + frame_system::Config,
		<T as orml_tokens::Config>::CurrencyId: From<CurrencyId>,
	{
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
			let pre_data = orml_tokens::Accounts::<T>::get(
				<Domain as Convert<_, T::AccountId>>::convert(ETH_DOMAIN),
				T::CurrencyId::from(LP_ETH_USDC),
			);

			if !pre_data.frozen.is_zero() && !pre_data.reserved.is_zero() {
				log::error!("AccountData of Ethereum domain account has non free balances...");
			}

			log::info!(
				"AccountData of Ethereum domain account has free balance of: {:?}",
				pre_data.free
			);

			Ok(Vec::new())
		}

		fn on_runtime_upgrade() -> Weight {
			let data = orml_tokens::Accounts::<T>::get(
				<Domain as Convert<_, T::AccountId>>::convert(ETH_DOMAIN),
				T::CurrencyId::from(LP_ETH_USDC),
			);

			if let Err(e) = orml_tokens::Pallet::<T>::burn_from(
				T::CurrencyId::from(LP_ETH_USDC),
				&<Domain as Convert<_, T::AccountId>>::convert(ETH_DOMAIN),
				data.free,
				Precision::Exact,
				Fortitude::Force,
			) {
				log::error!(
					"Burning from Ethereum domain account failed with: {:?}. Migration failed...",
					e
				);
			}

			T::DbWeight::get().reads(1)
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: Vec<u8>) -> Result<(), TryRuntimeError> {
			let post_data = orml_tokens::Accounts::<T>::get(
				<Domain as Convert<_, T::AccountId>>::convert(ETH_DOMAIN),
				T::CurrencyId::from(LP_ETH_USDC),
			);

			if !post_data.free.is_zero()
				&& !post_data.frozen.is_zero()
				&& !post_data.reserved.is_zero()
			{
				log::error!(
					"AccountDatat of Ethereum domain account SHOULD be zero. Migration failed."
				);
			}

			Ok(())
		}
	}
}
