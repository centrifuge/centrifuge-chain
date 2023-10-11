// Copyright 2019-2021 Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

use std::sync::Arc;

use cfg_primitives::{Block, BlockNumber};
use cumulus_client_cli::CollatorOptions;
use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
use cumulus_client_consensus_common::ParachainBlockImport as TParachainBlockImport;
use cumulus_primitives_core::ParaId;
use fc_db::Backend as FrontierBackend;
use sc_executor::WasmExecutor;
use sc_service::{Configuration, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::TelemetryHandle;

use crate::rpc::{
	self,
	anchors::{AnchorApiServer, Anchors},
	pools::{Pools, PoolsApiServer},
	rewards::{Rewards, RewardsApiServer},
};

pub(crate) mod evm;
use evm::EthConfiguration;

#[cfg(not(feature = "runtime-benchmarks"))]
type HostFunctions = sp_io::SubstrateHostFunctions;

#[cfg(feature = "runtime-benchmarks")]
type HostFunctions = (
	sp_io::SubstrateHostFunctions,
	frame_benchmarking::benchmarking::HostFunctions,
);

type FullClient<RuntimeApi> = TFullClient<Block, RuntimeApi, WasmExecutor<HostFunctions>>;

type FullBackend = TFullBackend<Block>;

type ParachainBlockImport<RuntimeApi> =
	TParachainBlockImport<Block, Arc<FullClient<RuntimeApi>>, FullBackend>;

// Native Altair executor instance.
pub struct AltairRuntimeExecutor;

impl sc_executor::NativeExecutionDispatch for AltairRuntimeExecutor {
	/// Only enable the benchmarking host functions when we actually want to
	/// benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		altair_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		altair_runtime::native_version()
	}
}

// Native Centrifuge executor instance.
pub struct CentrifugeRuntimeExecutor;

impl sc_executor::NativeExecutionDispatch for CentrifugeRuntimeExecutor {
	/// Only enable the benchmarking host functions when we actually want to
	/// benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		centrifuge_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		centrifuge_runtime::native_version()
	}
}

// Native Development executor instance.
pub struct DevelopmentRuntimeExecutor;

impl sc_executor::NativeExecutionDispatch for DevelopmentRuntimeExecutor {
	/// Only enable the benchmarking host functions when we actually want to
	/// benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		development_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		development_runtime::native_version()
	}
}

/// Build the import queue for the "altair" runtime.
#[allow(clippy::type_complexity)]
pub fn build_altair_import_queue(
	client: Arc<FullClient<altair_runtime::RuntimeApi>>,
	block_import: ParachainBlockImport<altair_runtime::RuntimeApi>,
	config: &Configuration,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	frontier_backend: FrontierBackend<Block>,
	first_evm_block: BlockNumber,
) -> Result<
	sc_consensus::DefaultImportQueue<Block, FullClient<altair_runtime::RuntimeApi>>,
	sc_service::Error,
> {
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;
	let block_import = evm::BlockImport::new(
		block_import,
		first_evm_block,
		client.clone(),
		Arc::new(frontier_backend),
	);

	cumulus_client_consensus_aura::import_queue::<
		sp_consensus_aura::sr25519::AuthorityPair,
		_,
		_,
		_,
		_,
		_,
	>(cumulus_client_consensus_aura::ImportQueueParams {
		block_import,
		client,
		create_inherent_data_providers: move |_, _| async move {
			let time = sp_timestamp::InherentDataProvider::from_system_time();

			let slot =
				sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*time,
					slot_duration,
				);

			Ok((slot, time))
		},
		registry: config.prometheus_registry(),
		spawner: &task_manager.spawn_essential_handle(),
		telemetry,
	})
	.map_err(Into::into)
}

/// Start an altair parachain node.
pub async fn start_altair_node(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	eth_config: EthConfiguration,
	collator_options: CollatorOptions,
	id: ParaId,
	first_evm_block: BlockNumber,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<altair_runtime::RuntimeApi>>)> {
	let is_authority = parachain_config.role.is_authority();
	evm::start_node_impl::<altair_runtime::RuntimeApi, AltairRuntimeExecutor, _, _, _>(
		parachain_config,
		polkadot_config,
		eth_config,
		collator_options,
		id,
		first_evm_block,
		move |client,
		      pool,
		      deny_unsafe,
		      subscription_task_executor,
		      network,
		      frontier_backend,
		      filter_pool,
		      fee_history_cache,
		      overrides,
		      block_data_cache| {
			let mut module = rpc::create_full(client.clone(), pool.clone(), deny_unsafe)?;
			module
				.merge(Anchors::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			module
				.merge(Pools::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			let eth_deps = rpc::evm::Deps {
				client,
				pool: pool.clone(),
				graph: pool.pool().clone(),
				converter: Some(development_runtime::TransactionConverter),
				is_authority,
				enable_dev_signer: eth_config.enable_dev_signer,
				network,
				frontier_backend: match frontier_backend.clone() {
					fc_db::Backend::KeyValue(b) => Arc::new(b),
					#[cfg(feature = "sql")]
					fc_db::Backend::Sql(b) => Arc::new(b),
				},
				overrides,
				block_data_cache,
				filter_pool,
				max_past_logs: eth_config.max_past_logs,
				fee_history_cache,
				fee_history_cache_limit: eth_config.fee_history_limit,
				execute_gas_limit_multiplier: eth_config.execute_gas_limit_multiplier,
				forced_parent_hashes: None,
			};
			let module = rpc::evm::create(module, eth_deps, subscription_task_executor, Arc::new(Default::default()))?;
			Ok(module)
		},
		build_altair_import_queue,
		|client,
		 block_import,
		 prometheus_registry,
		 telemetry,
		 task_manager,
		 relay_chain_interface,
		 transaction_pool,
		 sync_oracle,
		 keystore,
		 force_authoring| {
			let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

			let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
				task_manager.spawn_handle(),
				client.clone(),
				transaction_pool,
				prometheus_registry,
				telemetry.clone(),
			);

			Ok(AuraConsensus::build::<
				sp_consensus_aura::sr25519::AuthorityPair,
				_,
				_,
				_,
				_,
				_,
				_,
			>(BuildAuraConsensusParams {
				proposer_factory,
				create_inherent_data_providers: move |_, (relay_parent, validation_data)| {
					let relay_chain_interface = relay_chain_interface.clone();
					async move {
						let parachain_inherent =
							cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
							relay_parent,
							&relay_chain_interface,
							&validation_data,
							id,
						).await;

						let time = sp_timestamp::InherentDataProvider::from_system_time();

						let slot =
							sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
								*time,
								slot_duration,
							);

						let parachain_inherent = parachain_inherent.ok_or_else(|| {
							Box::<dyn std::error::Error + Send + Sync>::from(
								"Failed to create parachain inherent",
							)
						})?;
						Ok((slot, time, parachain_inherent))
					}
				},
				block_import,
				para_client: client,
				backoff_authoring_blocks: Option::<()>::None,
				sync_oracle,
				keystore,
				force_authoring,
				slot_duration,
				telemetry,
				// We got around 500ms for proposing
				block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
				// And a maximum of 750ms if slots are skipped
				max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
			}))
		},
	)
	.await
}

/// Build the import queue for the "centrifuge" runtime.
#[allow(clippy::type_complexity)]
pub fn build_centrifuge_import_queue(
	client: Arc<FullClient<centrifuge_runtime::RuntimeApi>>,
	block_import: ParachainBlockImport<centrifuge_runtime::RuntimeApi>,
	config: &Configuration,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	frontier_backend: FrontierBackend<Block>,
	first_evm_block: BlockNumber,
) -> Result<
	sc_consensus::DefaultImportQueue<Block, FullClient<centrifuge_runtime::RuntimeApi>>,
	sc_service::Error,
> {
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;
	let block_import = evm::BlockImport::new(
		block_import,
		first_evm_block,
		client.clone(),
		Arc::new(frontier_backend),
	);

	cumulus_client_consensus_aura::import_queue::<
		sp_consensus_aura::sr25519::AuthorityPair,
		_,
		_,
		_,
		_,
		_,
	>(cumulus_client_consensus_aura::ImportQueueParams {
		block_import,
		client,
		create_inherent_data_providers: move |_, _| async move {
			let time = sp_timestamp::InherentDataProvider::from_system_time();

			let slot =
				sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*time,
					slot_duration,
				);

			Ok((slot, time))
		},
		registry: config.prometheus_registry(),
		spawner: &task_manager.spawn_essential_handle(),
		telemetry,
	})
	.map_err(Into::into)
}

/// Start a centrifuge parachain node.
pub async fn start_centrifuge_node(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	eth_config: EthConfiguration,
	collator_options: CollatorOptions,
	id: ParaId,
	first_evm_block: BlockNumber,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<centrifuge_runtime::RuntimeApi>>)> {
	let is_authority = parachain_config.role.is_authority();
	evm::start_node_impl::<centrifuge_runtime::RuntimeApi, CentrifugeRuntimeExecutor, _, _, _>(
		parachain_config,
		polkadot_config,
		eth_config,
		collator_options,
		id,
		first_evm_block,
		move |client,
		      pool,
		      deny_unsafe,
		      subscription_task_executor,
		      network,
		      frontier_backend,
		      filter_pool,
		      fee_history_cache,
		      overrides,
		      block_data_cache| {
			let mut module = rpc::create_full(client.clone(), pool.clone(), deny_unsafe)?;
			module
				.merge(Anchors::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			module
				.merge(Pools::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			let eth_deps = rpc::evm::Deps {
				client,
				pool: pool.clone(),
				graph: pool.pool().clone(),
				converter: Some(development_runtime::TransactionConverter),
				is_authority,
				enable_dev_signer: eth_config.enable_dev_signer,
				network,
				// nuno
				frontier_backend: match frontier_backend.clone() {
					fc_db::Backend::KeyValue(b) => Arc::new(b),
					#[cfg(feature = "sql")]
					fc_db::Backend::Sql(b) => Arc::new(b),
				},
				overrides,
				block_data_cache,
				filter_pool,
				max_past_logs: eth_config.max_past_logs,
				fee_history_cache,
				fee_history_cache_limit: eth_config.fee_history_limit,
				execute_gas_limit_multiplier: eth_config.execute_gas_limit_multiplier,
				forced_parent_hashes: None,
			};
			let module = rpc::evm::create(module, eth_deps, subscription_task_executor, Arc::new(Default::default()))?;
			Ok(module)
		},
		build_centrifuge_import_queue,
		|client,
		 block_import,
		 prometheus_registry,
		 telemetry,
		 task_manager,
		 relay_chain_interface,
		 transaction_pool,
		 sync_oracle,
		 keystore,
		 force_authoring| {
			let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

			let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
				task_manager.spawn_handle(),
				client.clone(),
				transaction_pool,
				prometheus_registry,
				telemetry.clone(),
			);

			Ok(AuraConsensus::build::<
				sp_consensus_aura::sr25519::AuthorityPair,
				_,
				_,
				_,
				_,
				_,
				_,
			>(BuildAuraConsensusParams {
				proposer_factory,
				create_inherent_data_providers: move |_, (relay_parent, validation_data)| {
					let relay_chain_interface = relay_chain_interface.clone();
					async move {
						let parachain_inherent =
							cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
								relay_parent,
								&relay_chain_interface,
								&validation_data,
								id,
							).await;

						let time = sp_timestamp::InherentDataProvider::from_system_time();

						let slot =
							sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
								*time,
								slot_duration,
							);

						let parachain_inherent = parachain_inherent.ok_or_else(|| {
							Box::<dyn std::error::Error + Send + Sync>::from(
								"Failed to create parachain inherent",
							)
						})?;
						Ok((slot, time, parachain_inherent))
					}
				},
				block_import,
				para_client: client,
				backoff_authoring_blocks: Option::<()>::None,
				sync_oracle,
				keystore,
				force_authoring,
				slot_duration,
				// We got around 500ms for proposing
				block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
				// And a maximum of 750ms if slots are skipped
				max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
				telemetry,
			}))
		},
	)
	.await
}

/// Build the import queue for the "development" runtime.
#[allow(clippy::type_complexity)]
pub fn build_development_import_queue(
	client: Arc<FullClient<development_runtime::RuntimeApi>>,
	block_import: ParachainBlockImport<development_runtime::RuntimeApi>,
	config: &Configuration,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	frontier_backend: FrontierBackend<Block>,
	first_evm_block: BlockNumber,
) -> Result<
	sc_consensus::DefaultImportQueue<Block, FullClient<development_runtime::RuntimeApi>>,
	sc_service::Error,
> {
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;
	let block_import = evm::BlockImport::new(
		block_import,
		first_evm_block,
		client.clone(),
		Arc::new(frontier_backend),
	);

	cumulus_client_consensus_aura::import_queue::<
		sp_consensus_aura::sr25519::AuthorityPair,
		_,
		_,
		_,
		_,
		_,
	>(cumulus_client_consensus_aura::ImportQueueParams {
		block_import,
		client,
		create_inherent_data_providers: move |_, _| async move {
			let time = sp_timestamp::InherentDataProvider::from_system_time();

			let slot =
				sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*time,
					slot_duration,
				);

			Ok((slot, time))
		},
		registry: config.prometheus_registry(),
		spawner: &task_manager.spawn_essential_handle(),
		telemetry,
	})
	.map_err(Into::into)
}

/// Start a development parachain node.
pub async fn start_development_node(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	eth_config: EthConfiguration,
	collator_options: CollatorOptions,
	id: ParaId,
	first_evm_block: BlockNumber,
) -> sc_service::error::Result<(
	TaskManager,
	Arc<FullClient<development_runtime::RuntimeApi>>,
)> {
	let is_authority = parachain_config.role.is_authority();

	evm::start_node_impl::<development_runtime::RuntimeApi, DevelopmentRuntimeExecutor, _, _, _>(
		parachain_config,
		polkadot_config,
		eth_config,
		collator_options,
		id,
		first_evm_block,
		move |client,
		      pool,
		      deny_unsafe,
		      subscription_task_executor,
		      network,
		      frontier_backend,
		      filter_pool,
		      fee_history_cache,
		      overrides,
		      block_data_cache| {
			let mut module = rpc::create_full(client.clone(), pool.clone(), deny_unsafe)?;
			module
				.merge(Anchors::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			module
				.merge(Pools::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			module
				.merge(Rewards::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			let eth_deps = rpc::evm::Deps {
				client,
				pool: pool.clone(),
				graph: pool.pool().clone(),
				converter: Some(development_runtime::TransactionConverter),
				is_authority,
				enable_dev_signer: eth_config.enable_dev_signer,
				network,
				//nuno
				frontier_backend: match frontier_backend.clone() {
					fc_db::Backend::KeyValue(b) => Arc::new(b),
					#[cfg(feature = "sql")]
					fc_db::Backend::Sql(b) => Arc::new(b),
				},
				overrides,
				block_data_cache,
				filter_pool,
				max_past_logs: eth_config.max_past_logs,
				fee_history_cache,
				fee_history_cache_limit: eth_config.fee_history_limit,
				execute_gas_limit_multiplier: eth_config.execute_gas_limit_multiplier,
				forced_parent_hashes: None,
			};
			// nuno pass pubsub_notification_sinks
			let module = rpc::evm::create(module, eth_deps, subscription_task_executor, Arc::new(Default::default()))?;
			Ok(module)
		},
		build_development_import_queue,
		|client,
		 block_import,
		 prometheus_registry,
		 telemetry,
		 task_manager,
		 relay_chain_interface,
		 transaction_pool,
		 sync_oracle,
		 keystore,
		 force_authoring| {
			let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

			let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
				task_manager.spawn_handle(),
				client.clone(),
				transaction_pool,
				prometheus_registry,
				telemetry.clone(),
			);

			Ok(AuraConsensus::build::<
				sp_consensus_aura::sr25519::AuthorityPair,
				_,
				_,
				_,
				_,
				_,
				_,
			>(BuildAuraConsensusParams {
				proposer_factory,
				create_inherent_data_providers: move |_, (relay_parent, validation_data)| {
					let relay_chain_interface = relay_chain_interface.clone();
					async move {
						let parachain_inherent =
							cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
								relay_parent,
								&relay_chain_interface,
								&validation_data,
								id,
							).await;

						let time = sp_timestamp::InherentDataProvider::from_system_time();

						let slot =
							sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
								*time,
								slot_duration,
							);

						let parachain_inherent = parachain_inherent.ok_or_else(|| {
							Box::<dyn std::error::Error + Send + Sync>::from(
								"Failed to create parachain inherent",
							)
						})?;
						Ok((slot, time, parachain_inherent))
					}
				},
				block_import,
				para_client: client,
				backoff_authoring_blocks: Option::<()>::None,
				sync_oracle,
				keystore,
				force_authoring,
				slot_duration,
				// We got around 500ms for proposing
				block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
				// And a maximum of 750ms if slots are skipped
				max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
				telemetry,
			}))
		},
	)
	.await
}
