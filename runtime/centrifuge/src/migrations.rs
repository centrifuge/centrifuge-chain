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

use cfg_primitives::{Balance, PoolId};
use cfg_types::tokens::{
	usdc::{
		CURRENCY_ID_AXELAR, CURRENCY_ID_DOT_NATIVE, CURRENCY_ID_LOCAL, CURRENCY_ID_LP_ARB,
		CURRENCY_ID_LP_BASE, CURRENCY_ID_LP_CELO, CURRENCY_ID_LP_ETH, LOCAL_ASSET_ID,
	},
	CurrencyId, LocalAssetId,
};

frame_support::parameter_types! {
	pub const ClaimsPalletName: &'static str = "Claims";
	pub const MigrationPalletName: &'static str = "Migration";
	pub const UsdcVariants: [CurrencyId; 6] = [CURRENCY_ID_DOT_NATIVE, CURRENCY_ID_AXELAR, CURRENCY_ID_LP_ETH, CURRENCY_ID_LP_BASE, CURRENCY_ID_LP_ARB, CURRENCY_ID_LP_CELO];
	pub const LocalAssetIdUsdc: LocalAssetId = LOCAL_ASSET_ID;
	pub const LocalCurrencyIdUsdc: CurrencyId = CURRENCY_ID_LOCAL;
	pub const PoolIdAnemoy: PoolId = 4_139_607_887;
	pub const PoolCurrencyAnemoy: CurrencyId = CURRENCY_ID_DOT_NATIVE;
	pub const UsdcDot: CurrencyId = CURRENCY_ID_DOT_NATIVE;
	pub const UsdcEth: CurrencyId = CURRENCY_ID_LP_ETH;
	pub const UsdcBase: CurrencyId = CURRENCY_ID_LP_BASE;
	pub const UsdcArb: CurrencyId = CURRENCY_ID_LP_ARB;
	pub const UsdcCelo: CurrencyId = CURRENCY_ID_LP_CELO;
	pub const MinOrderAmount: Balance = 10u128.pow(6);
	pub const AnnualTreasuryInflationPercent: u32 = 3;
}

pub type UpgradeCentrifuge1025 = (
	// Burns tokens from other domains that are falsly not burned when they were transferred back
	// to their domain
	burn_unburned::Migration<super::Runtime>,
	runtime_common::migrations::epoch_execution::Migration<super::Runtime>,
	// Migrates the currency used in `pallet-transfer-allowlist` from our global currency to a
	// special filter currency enum
	runtime_common::migrations::transfer_allowlist_currency::Migration<super::Runtime>,
	// Removes tinlake reward claims pallet
	runtime_common::migrations::nuke::KillPallet<ClaimsPalletName, crate::RocksDbWeight>,
	// Register LocalUSDC
	runtime_common::migrations::local_currency::register::Migration<
		super::Runtime,
		LocalCurrencyIdUsdc,
	>,
	// Init local representation for all assets
	runtime_common::migrations::local_currency::translate_metadata::Migration<
		super::Runtime,
		UsdcVariants,
		LocalAssetIdUsdc,
	>,
	// Switch pool currency from Polkadot USDC to Local USDC
	runtime_common::migrations::local_currency::migrate_pool_currency::Migration<
		super::Runtime,
		PoolIdAnemoy,
		PoolCurrencyAnemoy,
		LocalCurrencyIdUsdc,
	>,
	// Removes unused migration pallet
	runtime_common::migrations::nuke::KillPallet<MigrationPalletName, crate::RocksDbWeight>,
	// Sets account codes for all precompiles
	runtime_common::migrations::precompile_account_codes::Migration<crate::Runtime>,
	// Bumps storage version from 0 to 1
	runtime_common::migrations::nuke::ResetPallet<crate::OrderBook, crate::RocksDbWeight, 0>,
	// Apply relative treasury inflation
	pallet_block_rewards::migrations::v2::RelativeTreasuryInflationMigration<
		crate::Runtime,
		AnnualTreasuryInflationPercent,
	>,
);

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
