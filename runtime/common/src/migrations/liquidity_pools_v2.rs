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
use pallet_axelar_router::AxelarId;
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

use crate::routing::RouterId;

const LOG_PREFIX: &str = "LpV2";

fn maybe_router_id(domain: Domain) -> Option<RouterId> {
	match domain.get_evm_chain_id() {
		None => {
			log::error!(
				"{LOG_PREFIX}: Failed derive chain_id from domain {domain:?} because it's not EVM"
			);
			None
		}
		Some(id) => Some(RouterId::Axelar(AxelarId::Evm(id))),
	}
}

pub mod kill_relayer_list {
	use frame_support::traits::OnRuntimeUpgrade;
	#[cfg(feature = "try-runtime")]
	use frame_support::{dispatch::DispatchResult, storage::with_storage_layer};
	#[cfg(feature = "try-runtime")]
	use sp_arithmetic::traits::Zero;
	#[cfg(feature = "try-runtime")]
	use sp_std::vec;

	use super::{types::v0, *};
	use crate::migrations::nuke::storage_clean_res_log;

	const LOG_PREFIX: &str = "ClearRelayerList";

	pub struct Migration<T>(PhantomData<T>);

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: pallet_liquidity_pools_gateway::Config,
	{
		fn on_runtime_upgrade() -> Weight {
			let res = v0::RelayerList::<T>::clear(20, None);
			storage_clean_res_log(&res, "RelayerList", LOG_PREFIX);

			log::info!("{LOG_PREFIX}: Migration done!");

			T::DbWeight::get().reads_writes(res.loops.into(), res.unique.into())
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			let mut cleared: bool = false;

			// Need to rollback in order to be NOOP
			let _ = with_storage_layer(|| -> DispatchResult {
				cleared = v0::RelayerList::<T>::clear(20, None).maybe_cursor.is_none();
				Err(DispatchError::Other("Reverting on purpose"))
			});
			assert!(cleared);

			log::info!("{LOG_PREFIX}: Pre checks done!");

			Ok(vec![])
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			assert!(v0::RelayerList::<T>::iter_keys().count().is_zero());
			log::info!("{LOG_PREFIX}: Post checks done!");

			Ok(())
		}
	}
}

pub mod v2_update_message_queue {
	use pallet_liquidity_pools::Message;
	use pallet_liquidity_pools_gateway::message::GatewayMessage;
	use sp_arithmetic::traits::SaturatedConversion;

	use super::{types::v2, *};

	const LOG_PREFIX: &str = "InitMessageQueue";

	pub struct Migration<T>(PhantomData<T>);

	impl<T> OnRuntimeUpgrade for v2_update_message_queue::Migration<T>
	where
		T: pallet_liquidity_pools_gateway::Config<Message = Message>
			+ frame_system::Config
			+ pallet_liquidity_pools_gateway_queue::Config<
				MessageNonce = u64,
				Message = GatewayMessage<Message, RouterId>,
			>,
	{
		fn on_runtime_upgrade() -> Weight {
			let items: u64 = v2::MessageQueue::<T>::iter_keys()
				.count()
				.saturating_add(v2::FailedMessageQueue::<T>::iter_keys().count())
				.saturated_into();

			pallet_liquidity_pools_gateway_queue::MessageQueue::<T>::translate_values::<
				v2::GatewayMessage<T::AccountId, Message>,
				_,
			>(|message| message.migrate());

			pallet_liquidity_pools_gateway_queue::FailedMessageQueue::<T>::translate_values::<
				(v2::GatewayMessage<T::AccountId, Message>, DispatchError),
				_,
			>(|(message, error)| message.migrate().map(|m| (m, error)));

			log::info!("{LOG_PREFIX}: Migration done with {items} in total!");

			T::DbWeight::get().reads_writes(items, items)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			assert!(
				v2::MessageQueue::<T>::iter_values().all(|message| match message {
					v2::GatewayMessage::<T::AccountId, Message>::Inbound { domain, .. } =>
						maybe_router_id(domain.domain()).is_some(),
					v2::GatewayMessage::<T::AccountId, Message>::Outbound {
						destination, ..
					} => maybe_router_id(destination).is_some(),
				})
			);
			assert!(v2::FailedMessageQueue::<T>::iter_values()
				.into_iter()
				.all(|(message, _)| match message {
					v2::GatewayMessage::<T::AccountId, Message>::Inbound { domain, .. } =>
						maybe_router_id(domain.domain()).is_some(),
					v2::GatewayMessage::<T::AccountId, Message>::Outbound {
						destination, ..
					} => maybe_router_id(destination).is_some(),
				}));

			let pending_v2: u64 = v2::MessageQueue::<T>::iter_keys().count().saturated_into();
			let failed_v2: u64 = v2::FailedMessageQueue::<T>::iter_keys()
				.count()
				.saturated_into();

			let pending_v3: u64 =
				pallet_liquidity_pools_gateway_queue::MessageQueue::<T>::iter_keys()
					.count()
					.saturated_into();
			let failed_v3: u64 =
				pallet_liquidity_pools_gateway_queue::FailedMessageQueue::<T>::iter_keys()
					.count()
					.saturated_into();

			log::info!(
				"{LOG_PREFIX}: Pre checks done with {pending_v2} items in v2::MessageQueue, {pending_v3} in v3::MessageQueue and {failed_v2} items in FailedMessageQueue and {failed_v3} in v3::FailedMessageQueue!"
			);

			Ok((
				pending_v2.saturating_add(pending_v3),
				failed_v2.saturating_add(failed_v3),
			)
				.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			let (pre_pending, pre_failed): (u64, u64) = Decode::decode(&mut pre_state.as_slice())
				.expect("pre_upgrade provides a valid state; qed");

			let pending: u64 = pallet_liquidity_pools_gateway_queue::MessageQueue::<T>::iter_keys()
				.count()
				.saturated_into();
			let failed: u64 =
				pallet_liquidity_pools_gateway_queue::FailedMessageQueue::<T>::iter_keys()
					.count()
					.saturated_into();

			assert_eq!(
                pre_pending, pending,
                "{LOG_PREFIX} POST: Mismatching number of pending messages in queue after migration!"
            );
			assert_eq!(
                pre_failed, failed,
                "{LOG_PREFIX} POST: Mismatching number of failed messages in queue after migration!"
            );

			log::info!("{LOG_PREFIX}: Post checks done!");

			Ok(())
		}
	}
}

pub mod v0_init_message_queue {
	use pallet_liquidity_pools::Message;
	use pallet_liquidity_pools_gateway::message::GatewayMessage;
	use sp_arithmetic::traits::{SaturatedConversion, Saturating};

	use super::{
		types::v0::{FailedOutboundMessages, OutboundMessageNonceStore, OutboundMessageQueue},
		*,
	};

	const LOG_PREFIX: &str = "InitMessageQueue";

	pub struct Migration<T>(PhantomData<T>);

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: pallet_liquidity_pools_gateway::Config<Message = Message>
			+ frame_system::Config
			+ pallet_liquidity_pools_gateway_queue::Config<
				MessageNonce = u64,
				Message = GatewayMessage<Message, RouterId>,
			>,
	{
		fn on_runtime_upgrade() -> Weight {
			let mut reads = 0u64;
			let mut writes = 0u64;
			OutboundMessageNonceStore::<T>::kill();

			pallet_liquidity_pools_gateway_queue::MessageNonceStore::<T>::put(0u64);
			Self::migrate_message_queue(&mut reads, &mut writes);
			Self::migrate_failed_message_queue(&mut reads, &mut writes);
			log::info!("{LOG_PREFIX}: Migration done with {reads} reads and {writes} writes!");

			// Add weight from killing and setting nonce store
			T::DbWeight::get().reads_writes(reads.saturating_add(1), writes.saturating_add(2))
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			assert!(OutboundMessageQueue::<T>::iter_values()
				.into_iter()
				.all(|(domain, _, _)| maybe_router_id(domain).is_some()));
			assert!(FailedOutboundMessages::<T>::iter_values()
				.into_iter()
				.all(|(domain, _, _, _)| maybe_router_id(domain).is_some()));

			let pending: u64 = OutboundMessageQueue::<T>::iter_keys()
				.count()
				.saturated_into();
			let failed: u64 = FailedOutboundMessages::<T>::iter_keys()
				.count()
				.saturated_into();

			log::info!("{LOG_PREFIX}: Pre checks done with {pending} items in OutboundMessageQueue and {failed} items in FailedOutboundMessages!");

			Ok((pending, failed).encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			let (pre_pending, pre_failed): (u64, u64) = Decode::decode(&mut pre_state.as_slice())
				.expect("pre_upgrade provides a valid state; qed");

			let pending: u64 = pallet_liquidity_pools_gateway_queue::MessageQueue::<T>::iter_keys()
				.count()
				.saturated_into();
			let failed: u64 =
				pallet_liquidity_pools_gateway_queue::FailedMessageQueue::<T>::iter_keys()
					.count()
					.saturated_into();

			assert_eq!(
                pre_pending, pending,
                "{LOG_PREFIX} POST: Mismatching number of pending messages in queue after migration!"
            );
			assert_eq!(
                pre_failed, failed,
                "{LOG_PREFIX} POST: Mismatching number of failed messages in queue after migration!"
            );

			log::info!("{LOG_PREFIX}: Post checks done!");

			Ok(())
		}
	}

	impl<T> Migration<T>
	where
		T: pallet_liquidity_pools_gateway::Config<Message = Message>
			+ frame_system::Config
			+ pallet_liquidity_pools_gateway_queue::Config<
				MessageNonce = u64,
				Message = GatewayMessage<Message, RouterId>,
			>,
	{
		fn migrate_message_queue(reads: &mut u64, writes: &mut u64) {
			let n: u64 = FailedOutboundMessages::<T>::iter_keys()
				.count()
				.saturated_into();
			log::info!("{LOG_PREFIX}: Initiating migration of {n} outbound messages");
			reads.saturating_accrue(n);

			for (nonce, (domain, _, message)) in OutboundMessageQueue::<T>::iter().drain() {
				// Should never be none since target are outbound messages
				if let Some(router_id) = maybe_router_id(domain) {
					log::info!(
						"{LOG_PREFIX}: Migrating outbound domain {domain:?} message {message:?}"
					);

					pallet_liquidity_pools_gateway_queue::MessageQueue::<T>::insert(
						nonce,
						GatewayMessage::Outbound { message, router_id },
					);

					writes.saturating_accrue(2);
				} else {
					writes.saturating_accrue(1);
					continue;
				}
			}
		}

		fn migrate_failed_message_queue(reads: &mut u64, writes: &mut u64) {
			let n: u64 = FailedOutboundMessages::<T>::iter_keys()
				.count()
				.saturated_into();
			log::info!("{LOG_PREFIX}: Initiating migration of {n} failed outbound messages");
			reads.saturating_accrue(n);

			for (nonce, (domain, _, message, err)) in FailedOutboundMessages::<T>::iter().drain() {
				if let Some(router_id) = maybe_router_id(domain) {
					log::info!(
						"{LOG_PREFIX}: Migrating failed outbound domain {domain:?} message {message:?}"
					);

					// Should never be none since target are outbound messages
					pallet_liquidity_pools_gateway_queue::FailedMessageQueue::<T>::insert(
						nonce,
						(GatewayMessage::Outbound { message, router_id }, err),
					);
					writes.saturating_accrue(2);
				} else {
					writes.saturating_accrue(1);
					continue;
				}
			}
		}
	}
}

pub mod init_axelar_router {
	use cfg_types::EVMChainId;
	#[cfg(feature = "try-runtime")]
	use frame_support::storage::transactional;
	use frame_support::{
		dispatch::DispatchResult,
		traits::{GetStorageVersion, OriginTrait, StorageVersion},
	};
	use frame_system::pallet_prelude::OriginFor;
	#[cfg(feature = "try-runtime")]
	use sp_arithmetic::traits::SaturatedConversion;
	use sp_arithmetic::traits::Saturating;
	use sp_std::boxed::Box;

	use super::{
		types::{v0, v2},
		*,
	};
	pub struct Migration<T>(PhantomData<T>);

	const LOG_PREFIX: &str = "DomainRoutersToAxelarConfig";

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: pallet_liquidity_pools_gateway::Config
			+ pallet_liquidity_pools_gateway_queue::Config
			+ pallet_xcm_transactor::Config
			+ pallet_ethereum_transaction::Config
			+ pallet_axelar_router::Config
			+ pallet_evm::Config
			+ frame_system::Config,
		T::AccountId: AsRef<[u8; 32]>,
		OriginFor<T>:
			From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		v0::DomainRouter<T>: sp_std::fmt::Debug,
		v2::DomainRouter<T>: sp_std::fmt::Debug,
	{
		fn on_runtime_upgrade() -> Weight {
			let (reads, writes) = Self::migrate_domain_routers().unwrap_or_default();
			log::info!(
				"{LOG_PREFIX} ON_RUNTIME_UPGRADE: Migration done with {writes:?} updated domains!"
			);

			T::DbWeight::get().reads_writes(reads, writes.saturating_add(1))
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			let mut writes = 0u64;

			// Need to rollback in order to be NOOP
			let _ = transactional::with_storage_layer(|| -> DispatchResult {
				let (r, w) = Self::migrate_domain_routers()?;
				log::info!("{LOG_PREFIX} PRE Migration counts {w:?} updated domains and {} removed domains", r.saturating_sub(w));
				writes = w;
				Err(DispatchError::Other("Reverting on purpose"))
			});

			log::info!("{LOG_PREFIX} PRE: Checks done with {writes} domains!");

			Ok(writes.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			let pre_count: u64 = Decode::decode(&mut pre_state.as_slice())
				.expect("pre_upgrade provides a valid state; qed");
			let post_count: u64 = pallet_axelar_router::Configuration::<T>::iter_keys()
				.count()
				.saturated_into();
			assert_eq!(
                pre_count, post_count,
                "{LOG_PREFIX} POST: Mismatching number of configured domains in axelar router after migration!"
            );
			assert_eq!(
				pallet_axelar_router::Configuration::<T>::iter_keys().count(),
				pallet_axelar_router::ChainNameById::<T>::iter_keys().count(),
			);

			log::info!("{LOG_PREFIX} POST: Checks done!");

			Ok(())
		}
	}

	impl<T> Migration<T>
	where
		T: pallet_liquidity_pools_gateway::Config
			+ pallet_liquidity_pools_gateway_queue::Config
			+ pallet_xcm_transactor::Config
			+ pallet_ethereum_transaction::Config
			+ pallet_axelar_router::Config
			+ pallet_evm::Config
			+ frame_system::Config,
		T::AccountId: AsRef<[u8; 32]>,
		OriginFor<T>:
			From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		v0::DomainRouter<T>: sp_std::fmt::Debug,
		v2::DomainRouter<T>: sp_std::fmt::Debug,
	{
		fn migrate_domain_routers() -> Result<(u64, u64), DispatchError> {
			match pallet_liquidity_pools_gateway::Pallet::<T>::on_chain_storage_version() {
				// Altair + Centrifuge chains
				zero if zero == StorageVersion::new(0) => Self::v0_migrate_domain_routers(),
				// Dev + Demo chains
				two if two == StorageVersion::new(2) => Self::v2_migrate_domain_routers(),
				z => {
					log::error!("Unexpected storage version {z:?}, must be either 0 or 2. Skipping domain router migration");
					Err(DispatchError::Other("Unexpected storage version"))
				}
			}
		}

		fn v0_migrate_domain_routers() -> Result<(u64, u64), DispatchError> {
			let mut reads: u64 = 0;
			let mut writes: u64 = 0;
			for (domain, val) in v0::DomainRouters::<T>::iter() {
				log::debug!("{LOG_PREFIX}: Inspecting key {domain:?} with value\n{val:?}");
				reads.saturating_accrue(1);

				let chain_id = match domain.get_evm_chain_id() {
					None => {
						log::info!("{LOG_PREFIX}: Skipping domain {domain:?} because it's not EVM");
						continue;
					}
					Some(id) => id,
				};

				match val {
					v0::DomainRouter::AxelarEVM(axelar) => {
						Self::migrate_axelar_evm(axelar, chain_id, &domain)?;
						writes.saturating_accrue(1);
					}
					v0::DomainRouter::EthereumXCM(_) => {
						log::info!(
							"{LOG_PREFIX} Removing v0::EthereumXCM router for domain {domain:?}"
						);
					}
					v0::DomainRouter::AxelarXCM(_) => {
						log::info!(
							"{LOG_PREFIX} Removing v0::AxelarXCM router for domain {domain:?}"
						);
					}
				}
			}

			Ok((reads, writes))
		}

		fn v2_migrate_domain_routers() -> Result<(u64, u64), DispatchError> {
			let mut reads: u64 = 0;
			let mut writes: u64 = 0;
			for (domain, domain_router) in v2::DomainRouters::<T>::iter() {
				log::debug!(
					"{LOG_PREFIX}: Inspecting key {domain:?} with domain router\n{domain_router:?}"
				);
				reads.saturating_accrue(1);

				let chain_id = match domain.get_evm_chain_id() {
					None => {
						log::info!("{LOG_PREFIX}: Skipping domain {domain:?} because it's not EVM");
						continue;
					}
					Some(id) => id,
				};
				Self::migrate_axelar_evm(domain_router.into(), chain_id, &domain)?;
				writes.saturating_accrue(1);
			}

			Ok((reads, writes))
		}

		fn migrate_axelar_evm(
			router: v0::AxelarEVMRouter<T>,
			chain_id: EVMChainId,
			domain: &Domain,
		) -> DispatchResult {
			// Read v0::gateway::AllowList storage => addr1
			// Read v0::axelar-gateway-precompile::GatewayContract => addr 2

			pallet_axelar_router::Pallet::<T>::set_config(
				T::RuntimeOrigin::root(),
				router.evm_chain.clone(),
				Box::new(router.migrate_to_domain_config(chain_id /* , addr1, addr2 */)),
			)
			.map_err(|e| {
				log::error!(
					"{LOG_PREFIX}: Failed to set axelar config for {domain:?} due to error \n{e:?}"
				);
				e
			})?;

			Ok(())
		}
	}
}

mod types {
	use super::*;

	pub(crate) mod v2 {
		use cfg_types::domain_address::{Domain, DomainAddress};
		use frame_support::{
			pallet_prelude::{Decode, Encode, MaxEncodedLen, OptionQuery, TypeInfo},
			storage_alias, Blake2_128Concat,
		};
		use frame_system::pallet_prelude::OriginFor;
		use pallet_liquidity_pools::Message;
		use sp_runtime::DispatchError;

		use super::v0::AxelarEVMRouter;
		use crate::{migrations::liquidity_pools_v2::maybe_router_id, routing::RouterId};

		#[storage_alias]
		pub type MessageQueue<T: pallet_liquidity_pools_gateway::Config> = StorageMap<
			pallet_liquidity_pools_gateway::Pallet<T>,
			Blake2_128Concat,
			u64,
			GatewayMessage<<T as frame_system::Config>::AccountId, Message>,
			OptionQuery,
		>;

		#[storage_alias]
		pub type FailedMessageQueue<T: pallet_liquidity_pools_gateway::Config> = StorageMap<
			pallet_liquidity_pools_gateway::Pallet<T>,
			Blake2_128Concat,
			u64,
			(
				GatewayMessage<<T as frame_system::Config>::AccountId, Message>,
				DispatchError,
			),
			OptionQuery,
		>;

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
			T: pallet_ethereum_transaction::Config + pallet_evm::Config,
			T::AccountId: AsRef<[u8; 32]>,
			OriginFor<T>:
				From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		{
			AxelarEVM(AxelarEVMRouter<T>),
		}

		impl<T> From<DomainRouter<T>> for AxelarEVMRouter<T>
		where
			T: pallet_ethereum_transaction::Config + pallet_evm::Config,
			T::AccountId: AsRef<[u8; 32]>,
			OriginFor<T>:
				From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		{
			fn from(value: DomainRouter<T>) -> Self {
				match value {
					DomainRouter::AxelarEVM(t) => t,
				}
			}
		}

		#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
		pub enum GatewayMessage<AccountId, Message> {
			Inbound {
				domain_address: DomainAddress,
				message: Message,
			},
			Outbound {
				sender: AccountId,
				destination: Domain,
				message: Message,
			},
		}

		impl<AccountId, Message> GatewayMessage<AccountId, Message> {
			pub(crate) fn migrate(
				self,
			) -> Option<pallet_liquidity_pools_gateway::message::GatewayMessage<Message, RouterId>>
			{
				match self {
					GatewayMessage::Inbound {
						domain_address,
						message,
					} => maybe_router_id(domain_address.domain()).map(|router_id| {
						pallet_liquidity_pools_gateway::message::GatewayMessage::<
                            Message,
                            RouterId,
                        >::Inbound {
                            domain: domain_address.domain(),
                            message,
                            router_id,
                        }
					}),
					GatewayMessage::Outbound {
						message,
						destination,
						..
					} => maybe_router_id(destination).map(|router_id| {
						pallet_liquidity_pools_gateway::message::GatewayMessage::<
                            Message,
                            RouterId,
                        >::Outbound {
                            message,
                            router_id,
                        }
					}),
				}
			}
		}
	}

	pub(crate) mod v0 {
		use cfg_types::{domain_address::DomainAddress, EVMChainId};
		use frame_support::{
			pallet_prelude::{Decode, Encode, MaxEncodedLen, OptionQuery, TypeInfo},
			traits::ConstU32,
			BoundedVec,
		};
		use frame_system::pallet_prelude::OriginFor;
		use pallet_axelar_router::{AxelarConfig, DomainConfig, EvmConfig, FeeValues};
		use pallet_liquidity_pools::Message;
		use sp_core::{H160, H256};
		use staging_xcm::VersionedLocation;

		use super::*;

		pub const MAX_AXELAR_EVM_CHAIN_SIZE: u32 = 16;

		#[storage_alias]
		pub type OutboundMessageNonceStore<T: pallet_liquidity_pools_gateway::Config> =
			StorageValue<pallet_liquidity_pools_gateway::Pallet<T>, u64, ValueQuery>;

		#[storage_alias]
		pub type OutboundMessageQueue<T: pallet_liquidity_pools_gateway::Config> = StorageMap<
			pallet_liquidity_pools_gateway::Pallet<T>,
			Blake2_128Concat,
			u64,
			(Domain, <T as frame_system::Config>::AccountId, Message),
		>;

		#[storage_alias]
		pub type FailedOutboundMessages<T: pallet_liquidity_pools_gateway::Config> = StorageMap<
			pallet_liquidity_pools_gateway::Pallet<T>,
			Blake2_128Concat,
			u64,
			(
				Domain,
				<T as frame_system::Config>::AccountId,
				Message,
				DispatchError,
			),
		>;

		#[storage_alias]
		pub type RelayerList<T: pallet_liquidity_pools_gateway::Config> = StorageDoubleMap<
			pallet_liquidity_pools_gateway::Pallet<T>,
			Blake2_128Concat,
			Domain,
			Blake2_128Concat,
			DomainAddress,
			(),
		>;

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

		#[storage_alias]
		pub type GatewayContract<T: pallet_axelar_router::Config> =
			StorageValue<pallet_axelar_router::Pallet<T>, H160, ValueQuery>;

		#[storage_alias]
		pub type Allowlist<T: pallet_liquidity_pools_gateway::Config> = StorageDoubleMap<
			pallet_liquidity_pools_gateway::Pallet<T>,
			Blake2_128Concat,
			Domain,
			Blake2_128Concat,
			DomainAddress,
			(),
		>;

		#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
		pub struct AxelarEVMRouter<T>
		where
			T: pallet_ethereum_transaction::Config + pallet_evm::Config,
			OriginFor<T>:
				From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		{
			pub router: EVMRouter<T>,
			pub evm_chain: BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>,
			pub liquidity_pools_contract_address: H160,
		}

		impl<T> AxelarEVMRouter<T>
		where
			T: pallet_ethereum_transaction::Config + pallet_evm::Config,
			OriginFor<T>:
				From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		{
			pub(crate) fn migrate_to_domain_config(&self, chain_id: EVMChainId) -> AxelarConfig {
				AxelarConfig {
					app_contract_address: todo!("previous value in gateway::AllowList storage"),
					inbound_contract_address: todo!(
						"previous axelar-gateway-precompile::GatewayContract storage",
					),
					outbound_contract_address: self.liquidity_pools_contract_address,
					domain: DomainConfig::Evm(EvmConfig {
						chain_id: chain_id,
						outbound_fee_values: self.router.evm_domain.fee_values.clone(),
					}),
				};
			}
		}

		#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
		pub struct EVMRouter<T>
		where
			T: pallet_ethereum_transaction::Config + pallet_evm::Config,
			OriginFor<T>:
				From<pallet_ethereum::Origin> + Into<Result<pallet_ethereum::Origin, OriginFor<T>>>,
		{
			pub evm_domain: EVMDomain,
			pub _marker: PhantomData<T>,
		}

		#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
		pub struct EVMDomain {
			/// The address of the contract deployed in our EVM.
			pub target_contract_address: H160,

			/// The `BlakeTwo256` hash of the target contract code.
			///
			/// This is used during router initialization to ensure that the
			/// correct contract code is used.
			pub target_contract_hash: H256,

			/// The values used when executing the EVM call.
			pub fee_values: FeeValues,
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
