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
pub type UpgradeCentrifuge1028 = migrate_anemoy_external_prices::Migration<super::Runtime>;

mod migrate_anemoy_external_prices {
	use cfg_primitives::PoolId;
	use cfg_traits::data::DataRegistry;
	use cfg_types::oracles::OracleKey;
	use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
	use pallet_loans::{entities::pricing::ActivePricing, WeightInfo};

	const LOG_PREFIX: &str = "MigrateAnemoyPrices:";
	const ANEMOY_POOL_ID: PoolId = 4139607887;
	/// Simply bumps the storage version of a pallet
	///
	/// NOTE: Use with caution! Must ensure beforehand that a migration is not
	/// necessary
	pub struct Migration<R>(sp_std::marker::PhantomData<R>);
	impl<R> OnRuntimeUpgrade for Migration<R>
	where
		R: pallet_loans::Config<PoolId = PoolId, PriceId = OracleKey>
			+ pallet_oracle_collection::Config<CollectionId = PoolId, OracleKey = OracleKey>,
	{
		fn on_runtime_upgrade() -> Weight {
			log::info!("{LOG_PREFIX}: STARTING Migrating Anemoy Price Ids.");
			let active_loans = pallet_loans::ActiveLoans::<R>::get(ANEMOY_POOL_ID);
			active_loans.clone().into_iter().for_each(|(_, loan)| {
				if let ActivePricing::External(pricing) = loan.pricing() {
					match pallet_oracle_collection::Pallet::<R>::register_id(
						&pricing.price_id(),
						&ANEMOY_POOL_ID,
					) {
						Ok(_) => {
							log::info!("{LOG_PREFIX}: Registered PriceId: {:?}", pricing.price_id())
						}
						Err(e) => log::info!(
							"{LOG_PREFIX}: Failed to register PriceId: {:?}, with error: {:?}.",
							pricing.price_id(),
							e
						),
					}
				}
			});

			log::info!("{LOG_PREFIX}: FINISHED Migrating Anemoy Price Ids.");
			<R as pallet_loans::Config>::WeightInfo::create()
				.saturating_mul(active_loans.len() as u64)
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
