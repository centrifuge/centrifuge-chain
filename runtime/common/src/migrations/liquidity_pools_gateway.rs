// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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

use cfg_types::domain_address::Domain;
#[cfg(feature = "try-runtime")]
use frame_support::pallet_prelude::{Decode, Encode};
use frame_support::{
	pallet_prelude::ValueQuery,
	storage_alias,
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
	Blake2_128Concat,
};
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

pub mod clear_outbound_nonce {
	use super::*;

	#[storage_alias]
	pub type OutboundMessageNonceStore<T: pallet_liquidity_pools_gateway::Config> =
		StorageValue<pallet_liquidity_pools_gateway::Pallet<T>, u64, ValueQuery>;

	#[storage_alias]
	pub type OutboundMessageQueue<T: pallet_liquidity_pools_gateway::Config> = StorageMap<
		pallet_liquidity_pools_gateway::Pallet<T>,
		Blake2_128Concat,
		u64,
		(
			Domain,
			<T as frame_system::Config>::AccountId,
			pallet_liquidity_pools::Message,
		),
	>;

	#[storage_alias]
	pub type FailedOutboundMessages<T: pallet_liquidity_pools_gateway::Config> = StorageMap<
		pallet_liquidity_pools_gateway::Pallet<T>,
		Blake2_128Concat,
		u64,
		(
			Domain,
			<T as frame_system::Config>::AccountId,
			pallet_liquidity_pools::Message,
			DispatchError,
		),
	>;

	const LOG_PREFIX: &str = "LPGatewayClearNonce";

	pub struct Migration<T>(PhantomData<T>);

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: pallet_liquidity_pools_gateway::Config + frame_system::Config,
	{
		fn on_runtime_upgrade() -> Weight {
			let mut weight = Weight::zero();
			OutboundMessageNonceStore::<T>::kill();

			for (nonce, entry) in OutboundMessageQueue::<T>::iter() {
				log::warn!("{LOG_PREFIX}: Found outbound message:\nnonce:{nonce}\nentry:{entry:?}");
				weight.saturating_accrue(T::DbWeight::get().reads(1));
			}

			for (nonce, entry) in FailedOutboundMessages::<T>::iter() {
				log::warn!(
					"{LOG_PREFIX}: Found failed outbound message:\nnonce:{nonce}\nentry:{entry:?}"
				);
				weight.saturating_accrue(T::DbWeight::get().reads(1));
			}

			log::info!("{LOG_PREFIX}: Migration done!");

			weight
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			// Extra check to confirm that the storage alias is correct.
			assert_eq!(
				OutboundMessageQueue::<T>::iter_keys().count(),
				0,
				"{LOG_PREFIX}: OutboundMessageQueue should be empty!"
			);

			assert_eq!(
				FailedOutboundMessages::<T>::iter_keys().count(),
				0,
				"{LOG_PREFIX}: FailedOutboundMessages should be empty!"
			);

			log::info!("{LOG_PREFIX}: Pre checks done!");

			Ok(Vec::new())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			assert_eq!(
				OutboundMessageNonceStore::<T>::get(),
				0,
				"{LOG_PREFIX}: OutboundMessageNonceStore should be 0!"
			);

			assert_eq!(
				OutboundMessageQueue::<T>::iter_keys().count(),
				0,
				"{LOG_PREFIX}: OutboundMessageQueue should be empty!"
			);

			assert_eq!(
				FailedOutboundMessages::<T>::iter_keys().count(),
				0,
				"{LOG_PREFIX}: FailedOutboundMessages should be empty!"
			);

			log::info!("{LOG_PREFIX}: Post checks done!");

			Ok(())
		}
	}
}

pub mod clear_deprecated_domain_router_entries {
	use frame_system::pallet_prelude::OriginFor;
	use sp_arithmetic::traits::SaturatedConversion;

	use super::*;
	pub struct Migration<T>(PhantomData<T>);

	const LOG_PREFIX: &str = "LPGatewayClearDomainRouters";

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: pallet_liquidity_pools_gateway::Config
			+ pallet_xcm_transactor::Config
			+ pallet_ethereum_transaction::Config
			+ pallet_evm::Config
			+ frame_system::Config,
		T::AccountId: AsRef<[u8; 32]>,
		OriginFor<T>:
			From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		v0::DomainRouter<T>: sp_std::fmt::Debug,
		<T as pallet_liquidity_pools_gateway::Config>::Router:
			From<liquidity_pools_gateway_routers::DomainRouter<T>>,
	{
		fn on_runtime_upgrade() -> Weight {
			let items = v0::DomainRouters::<T>::iter_keys().count().saturated_into();

			pallet_liquidity_pools_gateway::DomainRouters::<T>::translate::<v0::DomainRouter<T>, _>(
				|key, old| {
					log::debug!("{LOG_PREFIX}: Inspecting key {key:?} with value\n{old:?}");
					match old {
						v0::DomainRouter::AxelarEVM(router) => Some(
							liquidity_pools_gateway_routers::DomainRouter::<T>::AxelarEVM(router)
								.into(),
						),
						// Remove other entries
						router => {
							log::info!("{LOG_PREFIX} : Removing entry {router:?}!");
							None
						}
					}
				},
			);

			log::info!(
				"{LOG_PREFIX} ON_RUNTIME_UPGRADE: Migration done with {items:?} items in total!"
			);

			T::DbWeight::get().reads_writes(items, items)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			let count_total = v0::DomainRouters::<T>::iter_keys().count();
			let count_axelar_evm_router: u64 = v0::DomainRouters::<T>::iter()
				.filter(|(domain, router)| {
					log::debug!(
						"{LOG_PREFIX} PRE: Inspecting key {domain:?} with value\n{router:?}"
					);
					matches!(router, v0::DomainRouter::AxelarEVM(_))
				})
				.count()
				.saturated_into();
			log::info!(
				"{LOG_PREFIX} PRE: Found {count_axelar_evm_router:?} values of
		total {count_total:?} to migrate!"
			);
			log::info!(
				"{LOG_PREFIX} PRE: Checks
		done!"
			);
			Ok(count_axelar_evm_router.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			let pre_count: u64 = Decode::decode(&mut pre_state.as_slice())
				.expect("pre_upgrade provides a valid state; qed");
			let post_count: u64 = pallet_liquidity_pools_gateway::DomainRouters::<T>::iter_keys()
				.count()
				.saturated_into();
			assert_eq!(
				pre_count, post_count,
				"{LOG_PREFIX} POST: Mismatching number of domain routers after migration!"
			);

			log::info!("{LOG_PREFIX} POST: Checks done!");

			Ok(())
		}
	}

	mod v0 {
		use frame_support::{
			pallet_prelude::{Decode, Encode, MaxEncodedLen, OptionQuery, TypeInfo},
			traits::ConstU32,
			BoundedVec,
		};
		use liquidity_pools_gateway_routers::AxelarEVMRouter;
		use sp_core::H160;
		use staging_xcm::VersionedLocation;

		use super::*;

		pub const MAX_AXELAR_EVM_CHAIN_SIZE: u32 = 16;

		#[storage_alias]
		pub type DomainRouters<T: pallet_liquidity_pools_gateway::Config> = StorageMap<
			pallet_liquidity_pools_gateway::Pallet<T>,
			Blake2_128Concat,
			Domain,
			DomainRouter<T>,
			OptionQuery,
		>;

		#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
		pub enum DomainRouter<T>
		where
			T: pallet_xcm_transactor::Config
				+ pallet_ethereum_transaction::Config
				+ pallet_evm::Config,
			T::AccountId: AsRef<[u8; 32]>,
			OriginFor<T>:
				From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		{
			EthereumXCM(EthereumXCMRouter<T>),
			AxelarEVM(AxelarEVMRouter<T>),
			AxelarXCM(AxelarXCMRouter<T>),
		}

		#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
		pub struct AxelarXCMRouter<T: pallet_xcm_transactor::Config> {
			pub router: XCMRouter<T>,
			pub axelar_target_chain: BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>,
			pub axelar_target_contract: H160,
		}

		#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
		pub struct EthereumXCMRouter<T: pallet_xcm_transactor::Config> {
			pub router: XCMRouter<T>,
		}

		#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
		pub struct XCMRouter<T: pallet_xcm_transactor::Config> {
			pub xcm_domain: XcmDomain<T::CurrencyId>,
		}

		#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
		pub struct XcmDomain<CurrencyId> {
			pub location: sp_std::boxed::Box<VersionedLocation>,
			pub ethereum_xcm_transact_call_index:
				BoundedVec<u8, ConstU32<{ xcm_primitives::MAX_ETHEREUM_XCM_INPUT_SIZE }>>,
			pub contract_address: H160,
			pub max_gas_limit: u64,
			pub transact_required_weight_at_most: Weight,
			pub overall_weight: Weight,
			pub fee_currency: CurrencyId,
			pub fee_amount: u128,
		}
	}
}
