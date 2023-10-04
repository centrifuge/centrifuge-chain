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
use crate::{Runtime, Weight};

pub type UpgradeCentrifuge1022 = (
	anemoy_pool::Migration<crate::Runtime, 1021>,
	add_wrapped_usdc_variants::Migration<crate::Runtime, 1021>,
	// Sets account codes for all precompiles
	runtime_common::migrations::precompile_account_codes::Migration<crate::Runtime, 1022>,
);

/// Migrate the Anemoy Pool's currency from LpEthUSC to Circle's USDC,
/// native on Polkadot's AssetHub.
mod anemoy_pool {
	use cfg_primitives::PoolId;
	use cfg_traits::PoolInspect;
	use cfg_types::tokens::usdc::{CURRENCY_ID_DOT_NATIVE, CURRENCY_ID_LP_ETH};
	#[cfg(feature = "try-runtime")]
	use codec::{Decode, Encode};
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::traits::{fungibles::Inspect, OnRuntimeUpgrade};
	#[cfg(feature = "try-runtime")]
	use pallet_pool_system::PoolDetailsOf;
	use sp_std::vec;
	#[cfg(feature = "try-runtime")]
	use sp_std::vec::Vec;

	use super::*;
	use crate::PoolSystem;

	const ANEMOY_POOL_ID: PoolId = 4_139_607_887;

	pub struct Migration<T, const BEFORE_VERSION: u32>(sp_std::marker::PhantomData<T>);

	impl<T: frame_system::Config, const BEFORE_VERSION: u32> OnRuntimeUpgrade
		for Migration<T, BEFORE_VERSION>
	{
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			let pool_details: PoolDetailsOf<Runtime> =
				PoolSystem::pool(ANEMOY_POOL_ID).ok_or("Could not find Anemoy Pool")?;

			ensure!(
				pool_details.currency == CURRENCY_ID_LP_ETH,
				"anemoy_pool::Migration: pre_upgrade failing as Anemoy's currency should be LpEthUSDC"
			);

			Ok(pool_details.encode())
		}

		fn on_runtime_upgrade() -> Weight {
			let last_version = frame_system::LastRuntimeUpgrade::<T>::get()
				.map(|v| v.spec_version.0)
				.unwrap_or(<T::Version as frame_support::traits::Get<_>>::get().spec_version);

			if last_version >= BEFORE_VERSION {
				log::warn!(
					"anemoy_pool::Migration: NOT execution since current version higher than BEFORE_VERSION"
				);
				return Weight::zero();
			}

			let (sanity_checks, weight) = verify_sanity_checks();
			if !sanity_checks {
				log::error!("anemoy_pool::Migration: Sanity checks FAILED");
				return weight;
			}

			pallet_pool_system::Pool::<Runtime>::mutate(ANEMOY_POOL_ID, |maybe_details| {
				if let Some(details) = maybe_details {
					details.currency = CURRENCY_ID_DOT_NATIVE;
					log::info!("anemoy_pool::Migration: currency set to USDC ✓");
				} else {
					log::warn!("anemoy_pool::Migration: Pool details empty, skipping migration");
				}
			});

			weight.saturating_add(
				<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1),
			)
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(old_state: Vec<u8>) -> Result<(), &'static str> {
			let mut old_pool_details = PoolDetailsOf::<Runtime>::decode(&mut old_state.as_ref())
				.map_err(|_| "Error decoding pre-upgrade state")?;

			let pool_details: PoolDetailsOf<Runtime> =
				PoolSystem::pool(ANEMOY_POOL_ID).ok_or("Could not find Anemoy Pool")?;

			// Ensure the currency set to USDC is the only mutation performed
			old_pool_details.currency = CURRENCY_ID_DOT_NATIVE;
			ensure!(
				old_pool_details == pool_details,
				"Corrupted migration: Only the currency of the Anemoy pool should have changed"
			);

			log::info!("anemoy_pool::Migration: post_upgrade succeeded ✓");
			Ok(())
		}
	}

	fn verify_sanity_checks() -> (bool, Weight) {
		let res =
			crate::Tokens::balance(
				CURRENCY_ID_LP_ETH,
				&<PoolSystem as PoolInspect<_, _>>::account_for(ANEMOY_POOL_ID),
			) == 0 && pallet_investments::ActiveInvestOrders::<Runtime>::iter_keys()
				.filter(|investment| investment.pool_id == ANEMOY_POOL_ID)
				.count() == 0 && pallet_investments::ActiveRedeemOrders::<Runtime>::iter_keys()
				.filter(|investment| investment.pool_id == ANEMOY_POOL_ID)
				.count() == 0 && pallet_investments::InvestOrders::<Runtime>::iter_keys()
				.filter(|(_, investment)| investment.pool_id == ANEMOY_POOL_ID)
				.count() == 0 && pallet_investments::RedeemOrders::<Runtime>::iter_keys()
				.filter(|(_, investment)| investment.pool_id == ANEMOY_POOL_ID)
				.count() == 0;

		let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(
			vec![
				1, // pool account balance read
				pallet_investments::ActiveInvestOrders::<Runtime>::iter_keys().count(),
				pallet_investments::ActiveRedeemOrders::<Runtime>::iter_keys().count(),
				pallet_investments::InvestOrders::<Runtime>::iter_keys().count(),
				pallet_investments::RedeemOrders::<Runtime>::iter_keys().count(),
			]
			.iter()
			.fold(0u64, |acc, x| acc.saturating_add(*x as u64)),
		);

		(res, weight)
	}
}

/// Add more LP wrapped USDC variants to the asset registry as well as
/// bidirectional trading pairs to and from DOT native USDC for these.
pub mod add_wrapped_usdc_variants {
	#[cfg(feature = "try-runtime")]
	use cfg_traits::TokenSwaps;
	use cfg_types::tokens::{
		usdc::{
			lp_wrapped_usdc_metadata, CHAIN_ID_ARBITRUM_MAINNET, CHAIN_ID_BASE_MAINNET,
			CHAIN_ID_CELO_MAINNET, CONTRACT_ARBITRUM, CONTRACT_BASE, CONTRACT_CELO,
			CURRENCY_ID_DOT_NATIVE, CURRENCY_ID_LP_ARB, CURRENCY_ID_LP_BASE, CURRENCY_ID_LP_CELO,
			CURRENCY_ID_LP_ETH, MIN_SWAP_ORDER_AMOUNT,
		},
		CurrencyId, CustomMetadata,
	};
	use frame_support::traits::OnRuntimeUpgrade;
	use orml_asset_registry::AssetMetadata;
	use sp_runtime::SaturatedConversion;
	use sp_std::{vec, vec::Vec};

	use super::*;
	#[cfg(feature = "try-runtime")]
	use crate::OrderBook;
	use crate::{liquidity_pools::LiquidityPoolsPalletIndex, Balance, OrmlAssetRegistry, Runtime};

	pub struct Migration<T, const BEFORE_VERSION: u32>(sp_std::marker::PhantomData<T>);

	impl<T: frame_system::Config, const BEFORE_VERSION: u32> OnRuntimeUpgrade
		for Migration<T, BEFORE_VERSION>
	{
		fn on_runtime_upgrade() -> Weight {
			let last_version = frame_system::LastRuntimeUpgrade::<T>::get()
				.map(|v| v.spec_version.0)
				.unwrap_or(<T::Version as frame_support::traits::Get<_>>::get().spec_version);

			if last_version >= BEFORE_VERSION {
				log::warn!(
					"add_wrapped_usdc_variants::Migration: NOT execution since current version higher than BEFORE_VERSION"
				);
				return Weight::zero();
			}

			// Register assets
			for (currency_id, metadata) in Self::get_unregistered_metadata().into_iter() {
				log::debug!("Registering asset {:?}", currency_id);
				OrmlAssetRegistry::do_register_asset_without_asset_processor(metadata, currency_id)
					.map_err(|e| {
						log::error!(
							"Failed to register asset {:?} due to error {:?}",
							currency_id,
							e
						);
					})
					// Add trading pairs if asset was registered successfully
					.map(|_| {
						log::debug!(
							"Adding bidirectional USDC trading pair for asset {:?}",
							currency_id
						);
						pallet_order_book::TradingPair::<Runtime>::insert(
							CURRENCY_ID_DOT_NATIVE,
							currency_id,
							MIN_SWAP_ORDER_AMOUNT,
						);
						pallet_order_book::TradingPair::<Runtime>::insert(
							currency_id,
							CURRENCY_ID_DOT_NATIVE,
							MIN_SWAP_ORDER_AMOUNT,
						);
					})
					.ok();
			}
			// Add trading pair for already registered LpEthUsdc
			pallet_order_book::TradingPair::<Runtime>::insert(
				CURRENCY_ID_DOT_NATIVE,
				CURRENCY_ID_LP_ETH,
				MIN_SWAP_ORDER_AMOUNT,
			);
			pallet_order_book::TradingPair::<Runtime>::insert(
				CURRENCY_ID_LP_ETH,
				CURRENCY_ID_DOT_NATIVE,
				MIN_SWAP_ORDER_AMOUNT,
			);

			log::info!("add_wrapped_usdc_variants::Migration: on_runtime_upgrade succeeded ✓");

			// 2 writes for registering, 2 writes for adding trading pair
			let new_assets: u64 = Self::get_unregistered_ids().len().saturated_into();
			<Runtime as frame_system::Config>::DbWeight::get()
				.reads_writes(1, new_assets.saturating_mul(4).saturating_add(2))
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			assert!(
				Self::get_unregistered_ids()
					.into_iter()
					.all(|currency_id| OrmlAssetRegistry::metadata(currency_id).is_none()),
				"At least one of new the wrapped USDC variants is already registered"
			);

			log::info!("add_wrapped_usdc_variants::Migration: pre_upgrade succeeded ✓");
			Ok(Vec::new())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), &'static str> {
			assert!(
				Self::get_unregistered_ids()
					.into_iter()
					.all(|currency_id| OrmlAssetRegistry::metadata(currency_id).is_some()),
				"At least one of new the wrapped USDC variants was not registered"
			);

			assert!(
                Self::get_tradeable_ids()
                    .into_iter()
                    .all(|wrapped_usdc_id| {
                        OrderBook::valid_pair(CURRENCY_ID_DOT_NATIVE, wrapped_usdc_id)
                    }),
                "At least one of the wrapped USDC variants is not enabled as trading pair into DOT native USDC"
            );

			assert!(
                Self::get_tradeable_ids()
                    .into_iter()
                    .all(|wrapped_usdc_id| {
                        OrderBook::valid_pair(wrapped_usdc_id, CURRENCY_ID_DOT_NATIVE)
                    }),
                "At least one of the wrapped USDC variants is not enabled as trading pair from DOT native USDC"
            );

			log::info!("add_wrapped_usdc_variants::Migration: post_upgrade succeeded ✓");
			Ok(())
		}
	}

	impl<T, const BEFORE_VERSION: u32> Migration<T, BEFORE_VERSION> {
		fn get_unregistered_ids() -> Vec<CurrencyId> {
			vec![CURRENCY_ID_LP_BASE, CURRENCY_ID_LP_ARB, CURRENCY_ID_LP_CELO]
		}

		#[cfg(feature = "try-runtime")]
		fn get_tradeable_ids() -> Vec<CurrencyId> {
			vec![
				CURRENCY_ID_LP_ETH,
				CURRENCY_ID_LP_BASE,
				CURRENCY_ID_LP_ARB,
				CURRENCY_ID_LP_CELO,
			]
		}

		fn get_unregistered_metadata() -> Vec<(CurrencyId, AssetMetadata<Balance, CustomMetadata>)>
		{
			vec![
				(
					CURRENCY_ID_LP_BASE,
					lp_wrapped_usdc_metadata(
						"LP Base Wrapped USDC".as_bytes().to_vec(),
						"LpBaseUSDC".as_bytes().to_vec(),
						LiquidityPoolsPalletIndex::get(),
						CHAIN_ID_BASE_MAINNET,
						CONTRACT_BASE,
						true,
					),
				),
				(
					CURRENCY_ID_LP_ARB,
					lp_wrapped_usdc_metadata(
						"LP Arbitrum Wrapped USDC".as_bytes().to_vec(),
						"LpArbUSDC".as_bytes().to_vec(),
						LiquidityPoolsPalletIndex::get(),
						CHAIN_ID_ARBITRUM_MAINNET,
						CONTRACT_ARBITRUM,
						true,
					),
				),
				(
					CURRENCY_ID_LP_CELO,
					lp_wrapped_usdc_metadata(
						"LP Celo Wrapped USDC".as_bytes().to_vec(),
						"LpCeloUSDC".as_bytes().to_vec(),
						LiquidityPoolsPalletIndex::get(),
						CHAIN_ID_CELO_MAINNET,
						CONTRACT_CELO,
						true,
					),
				),
			]
		}
	}
}
