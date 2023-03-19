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

use std::{
	collections::BTreeMap,
	path::PathBuf,
	sync::{Arc, Mutex},
	time::Duration,
};

use cfg_primitives::{Block, Hash};
use cumulus_client_cli::CollatorOptions;
use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
use cumulus_client_consensus_common::{
	ParachainBlockImport as TParachainBlockImport, ParachainConsensus,
};
use cumulus_client_network::BlockAnnounceValidator;
use cumulus_client_service::{
	build_relay_chain_interface, prepare_node_config, start_collator, start_full_node,
	StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_interface::{RelayChainError, RelayChainInterface};
use fc_db::Backend as FrontierBackend;
use fc_mapping_sync::{MappingSyncWorker, SyncStrategy};
use fc_rpc::{EthTask, OverrideHandle};
use fc_rpc_core::types::{FeeHistoryCache, FilterPool};
use futures::{future, StreamExt};
use sc_cli::SubstrateCli;
use sc_client_api::BlockchainEvents;
use sc_consensus::ImportQueue;
use sc_executor::WasmExecutor;
use sc_network::{NetworkBlock, NetworkService};
use sc_rpc::SubscriptionTaskExecutor;
use sc_rpc_api::DenyUnsafe;
use sc_service::{
	BasePath, Configuration, PartialComponents, TFullBackend, TFullClient, TaskManager,
};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sp_api::ConstructRuntimeApi;
use sp_core::U256;
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::traits::BlakeTwo256;
use substrate_prometheus_endpoint::Registry;

use crate::{
	cli::Cli,
	rpc::{
		self,
		anchors::{AnchorApiServer, Anchors},
		pools::{Pools, PoolsApiServer},
		rewards::{Rewards, RewardsApiServer},
	},
};

mod evm;

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
	/// Only enable the benchmarking host functions when we actually want to benchmark.
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
	/// Only enable the benchmarking host functions when we actually want to benchmark.
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
	/// Only enable the benchmarking host functions when we actually want to benchmark.
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

fn db_config_dir(config: &Configuration) -> PathBuf {
	config
		.base_path
		.as_ref()
		.map(|base_path| base_path.config_dir(config.chain_spec.id()))
		.unwrap_or_else(|| {
			BasePath::from_project("", "", &Cli::executable_name())
				.config_dir(config.chain_spec.id())
		})
}

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
#[allow(clippy::type_complexity)]
pub fn new_partial<RuntimeApi, BIQ>(
	config: &Configuration,
	build_import_queue: BIQ,
) -> Result<
	PartialComponents<
		FullClient<RuntimeApi>,
		FullBackend,
		(),
		sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi>>,
		sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>,
		(
			ParachainBlockImport<RuntimeApi>,
			Option<Telemetry>,
			Option<TelemetryWorkerHandle>,
			Arc<FrontierBackend<Block>>,
			FilterPool,
			FeeHistoryCache,
		),
	>,
	sc_service::Error,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::Metadata<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_block_builder::BlockBuilder<Block>,
	sc_client_api::StateBackendFor<FullBackend, Block>: sp_api::StateBackend<BlakeTwo256>,
	BIQ: FnOnce(
		Arc<FullClient<RuntimeApi>>,
		ParachainBlockImport<RuntimeApi>,
		&Configuration,
		Option<TelemetryHandle>,
		&TaskManager,
		Arc<FrontierBackend<Block>>,
	) -> Result<
		sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi>>,
		sc_service::Error,
	>,
{
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = sc_executor::WasmExecutor::<HostFunctions>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		None,
		config.runtime_cache_size,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager
			.spawn_handle()
			.spawn("telemetry", None, worker.run());
		telemetry
	});

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let block_import = ParachainBlockImport::new(client.clone(), backend.clone());

	// TODO: is it bad if we create these when we're not in a runtime that uses EVM?
	let frontier_backend = Arc::new(FrontierBackend::open(
		Arc::clone(&client),
		&config.database,
		&db_config_dir(config),
	)?);

	let import_queue = build_import_queue(
		client.clone(),
		block_import.clone(),
		config,
		telemetry.as_ref().map(|telemetry| telemetry.handle()),
		&task_manager,
		frontier_backend.clone(),
	)?;

	let filter_pool: FilterPool = Arc::new(Mutex::new(BTreeMap::new()));
	let fee_history_cache: FeeHistoryCache = Arc::new(Mutex::new(BTreeMap::new()));

	let params = PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		select_chain: (),
		other: (
			block_import,
			telemetry,
			telemetry_worker_handle,
			frontier_backend,
			filter_pool,
			fee_history_cache,
		),
	};

	Ok(params)
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the runtime api.
#[allow(clippy::too_many_arguments)]
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl<RuntimeApi, Executor, RB, EXT, EB, BIQ, BIC, TS>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	collator_options: CollatorOptions,
	id: ParaId,
	ext_builder: EB,
	rpc_ext_builder: RB,
	build_import_queue: BIQ,
	build_consensus: BIC,
	task_ext_spawner: TS,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi>>)>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::Metadata<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ cumulus_primitives_core::CollectCollationInfo<Block>,
	sc_client_api::StateBackendFor<FullBackend, Block>: sp_api::StateBackend<BlakeTwo256>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
	EXT: Clone + 'static,
	EB: FnOnce(
		Arc<FullClient<RuntimeApi>>,
		Option<substrate_prometheus_endpoint::Registry>,
		&mut TaskManager,
	) -> EXT,
	RB: Fn(
			Arc<FullClient<RuntimeApi>>,
			Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>>,
			DenyUnsafe,
			SubscriptionTaskExecutor,
			Arc<NetworkService<Block, Hash>>,
			Arc<FrontierBackend<Block>>,
			FilterPool,
			FeeHistoryCache,
			EXT,
		) -> Result<rpc::RpcExtension, sc_service::Error>
		+ 'static,
	BIQ: FnOnce(
		Arc<FullClient<RuntimeApi>>,
		ParachainBlockImport<RuntimeApi>,
		&Configuration,
		Option<TelemetryHandle>,
		&TaskManager,
		Arc<FrontierBackend<Block>>,
	) -> Result<
		sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi>>,
		sc_service::Error,
	>,
	BIC: FnOnce(
		Arc<FullClient<RuntimeApi>>,
		ParachainBlockImport<RuntimeApi>,
		Option<&Registry>,
		Option<TelemetryHandle>,
		&TaskManager,
		Arc<dyn RelayChainInterface>,
		Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>>,
		Arc<NetworkService<Block, Hash>>,
		SyncCryptoStorePtr,
		bool,
	) -> Result<Box<dyn ParachainConsensus<Block>>, sc_service::Error>,
	TS: FnOnce(
		&TaskManager,
		Arc<FullClient<RuntimeApi>>,
		Arc<TFullBackend<Block>>,
		Arc<FrontierBackend<Block>>,
		FeeHistoryCache,
		FilterPool,
		EXT,
	),
{
	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial::<RuntimeApi, BIQ>(&parachain_config, build_import_queue)?;
	let (
		block_import,
		mut telemetry,
		telemetry_worker_handle,
		frontier_backend,
		filter_pool,
		fee_history_cache,
	) = params.other;

	let client = params.client.clone();
	let backend = params.backend.clone();
	let mut task_manager = params.task_manager;

	let (relay_chain_interface, collator_key) = build_relay_chain_interface(
		polkadot_config,
		&parachain_config,
		telemetry_worker_handle,
		&mut task_manager,
		collator_options.clone(),
		None,
	)
	.await
	.map_err(|e| match e {
		RelayChainError::ServiceError(polkadot_service::Error::Sub(x)) => x,
		s => s.to_string().into(),
	})?;
	let block_announce_validator = BlockAnnounceValidator::new(relay_chain_interface.clone(), id);

	let force_authoring = parachain_config.force_authoring;
	let validator = parachain_config.role.is_authority();
	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();
	let (network, system_rpc_tx, tx_handler_controller, start_network) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &parachain_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue: params.import_queue,
			block_announce_validator_builder: Some(Box::new(|_| {
				Box::new(block_announce_validator)
			})),
			warp_sync: None,
		})?;

	let rpc_client = client.clone();
	let pool = transaction_pool.clone();

	let ext = ext_builder(
		rpc_client.clone(),
		prometheus_registry.clone(),
		&mut task_manager,
	);
	let rpc_builder = {
		let network = network.clone();
		let ext = ext.clone();
		let frontier_backend = frontier_backend.clone();
		let fee_history_cache = fee_history_cache.clone();
		let filter_pool = filter_pool.clone();
		move |deny, subscription_task_executor| {
			rpc_ext_builder(
				rpc_client.clone(),
				pool.clone(),
				deny,
				subscription_task_executor,
				network.clone(),
				frontier_backend.clone(),
				filter_pool.clone(),
				fee_history_cache.clone(),
				ext.clone(),
			)
		}
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_builder: Box::new(rpc_builder),
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend: backend.clone(),
		network: network.clone(),
		system_rpc_tx,
		tx_handler_controller,
		telemetry: telemetry.as_mut(),
	})?;

	task_ext_spawner(
		&task_manager,
		client.clone(),
		backend.clone(),
		frontier_backend.clone(),
		fee_history_cache.clone(),
		filter_pool.clone(),
		ext,
	);

	let announce_block = {
		let network = network.clone();
		Arc::new(move |hash, data| network.announce_block(hash, data))
	};

	let relay_chain_slot_duration = Duration::from_secs(6);

	let _overseer_handle = relay_chain_interface
		.overseer_handle()
		.map_err(|e| sc_service::Error::Application(Box::new(e)))?;

	if validator {
		let parachain_consensus = build_consensus(
			client.clone(),
			block_import,
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface.clone(),
			transaction_pool,
			network,
			params.keystore_container.sync_keystore(),
			force_authoring,
		)?;

		let spawner = task_manager.spawn_handle();

		let params = StartCollatorParams {
			para_id: id,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			relay_chain_interface,
			spawner,
			parachain_consensus,
			import_queue: import_queue_service,
			collator_key: collator_key.ok_or_else(|| {
				sc_service::error::Error::Other("Collator Key is None".to_string())
			})?,
			relay_chain_slot_duration,
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id: id,
			relay_chain_interface,
			relay_chain_slot_duration,
			import_queue: import_queue_service,
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}

/// Build the import queue for the "altair" runtime.
#[allow(clippy::type_complexity)]
pub fn build_altair_import_queue(
	client: Arc<FullClient<altair_runtime::RuntimeApi>>,
	block_import: ParachainBlockImport<altair_runtime::RuntimeApi>,
	config: &Configuration,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	_: Arc<FrontierBackend<Block>>,
) -> Result<
	sc_consensus::DefaultImportQueue<Block, FullClient<altair_runtime::RuntimeApi>>,
	sc_service::Error,
> {
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

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
	collator_options: CollatorOptions,
	id: ParaId,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<altair_runtime::RuntimeApi>>)> {
	start_node_impl::<altair_runtime::RuntimeApi, AltairRuntimeExecutor, _, _, _, _, _, _>(
		parachain_config,
		polkadot_config,
		collator_options,
		id,
		|_, _, _| {},
		|client, pool, deny_unsafe, _, _, _, _, _, _| {
			let mut module = rpc::create_full(client.clone(), pool, deny_unsafe)?;
			module
				.merge(Anchors::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			module
				.merge(Pools::new(client).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
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
		|_, _, _, _, _, _, _| {},
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
	_: Arc<FrontierBackend<Block>>,
) -> Result<
	sc_consensus::DefaultImportQueue<Block, FullClient<centrifuge_runtime::RuntimeApi>>,
	sc_service::Error,
> {
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

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
	collator_options: CollatorOptions,
	id: ParaId,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<centrifuge_runtime::RuntimeApi>>)> {
	start_node_impl::<centrifuge_runtime::RuntimeApi, CentrifugeRuntimeExecutor, _, _, _, _, _, _>(
		parachain_config,
		polkadot_config,
		collator_options,
		id,
		|_, _, _| {},
		|client, pool, deny_unsafe, _, _, _, _, _, _| {
			let mut module = rpc::create_full(client.clone(), pool, deny_unsafe)?;
			module
				.merge(Anchors::new(client.clone()).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
			module
				.merge(Pools::new(client).into_rpc())
				.map_err(|e| sc_service::Error::Application(e.into()))?;
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
		|_, _, _, _, _, _, _| {},
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
	frontier_backend: Arc<FrontierBackend<Block>>,
) -> Result<
	sc_consensus::DefaultImportQueue<Block, FullClient<development_runtime::RuntimeApi>>,
	sc_service::Error,
> {
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

	let block_import = evm::BlockImport::new(block_import, client.clone(), frontier_backend);

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
			let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(1)); // TODO: cli.target_gas_price

			Ok((slot, time, dynamic_fee))
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
	collator_options: CollatorOptions,
	id: ParaId,
) -> sc_service::error::Result<(
	TaskManager,
	Arc<FullClient<development_runtime::RuntimeApi>>,
)> {
	let is_authority = parachain_config.role.is_authority();
	start_node_impl::<development_runtime::RuntimeApi, DevelopmentRuntimeExecutor, _, _, _, _, _, _>(
		parachain_config,
		polkadot_config,
		collator_options,
		id,
		|client, prometheus_registry, task_manager| {
			let overrides = rpc::eth::overrides_handle(client);
			let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
				task_manager.spawn_handle(),
				overrides.clone(),
				50, // eth_config.eth_log_block_cache,
				50, // eth_config.eth_statuses_cache,
				prometheus_registry,
			));
			(overrides, block_data_cache)
		},
		move |client,
		      pool,
		      deny_unsafe,
		      subscription_task_executor,
		      network,
		      frontier_backend,
		      filter_pool,
		      fee_history_cache,
		      (overrides, block_data_cache)| {
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
			let eth_deps = rpc::eth::Deps {
				client,
				pool: pool.clone(),
				graph: pool.pool().clone(),
				converter: Some(development_runtime::TransactionConverter),
				is_authority,
				enable_dev_signer: false, // eth_config.enable_dev_signer,
				network,
				frontier_backend,
				overrides,
				block_data_cache,
				filter_pool,
				max_past_logs: 10000, // eth_config.max_past_logs,
				fee_history_cache,
				fee_history_cache_limit: 2048,    // eth_config.fee_history_limit,
				execute_gas_limit_multiplier: 10, // eth_config.execute_gas_limit_multiplier,
			};
			let module = rpc::eth::create(module, eth_deps, subscription_task_executor)?;
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
		|task_manager, client, backend, frontier_backend, fee_history_cache, filter_pool, (overrides, _)| spawn_frontier_tasks::<development_runtime::RuntimeApi, DevelopmentRuntimeExecutor>(task_manager, client, backend, frontier_backend, filter_pool, overrides, fee_history_cache),
	)
	.await
}

fn spawn_frontier_tasks<RuntimeApi, Executor>(
	task_manager: &TaskManager,
	client: Arc<FullClient<RuntimeApi>>,
	backend: Arc<TFullBackend<Block>>,
	frontier_backend: Arc<FrontierBackend<Block>>,
	filter_pool: FilterPool,
	overrides: Arc<OverrideHandle<Block>>,
	fee_history_cache: FeeHistoryCache,
) where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::Metadata<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_api::ApiExt<
			Block,
			StateBackend = sc_client_api::StateBackendFor<TFullBackend<Block>, Block>,
		> + sp_offchain::OffchainWorkerApi<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ cumulus_primitives_core::CollectCollationInfo<Block>
		+ fp_rpc::EthereumRuntimeRPCApi<Block>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
{
	task_manager.spawn_essential_handle().spawn(
		"frontier-mapping-sync-worker",
		None,
		MappingSyncWorker::new(
			client.import_notification_stream(),
			Duration::new(6, 0),
			client.clone(),
			backend,
			frontier_backend,
			3,
			0,
			SyncStrategy::Parachain,
		)
		.for_each(|()| future::ready(())),
	);

	// Spawn Frontier EthFilterApi maintenance task.
	// Each filter is allowed to stay in the pool for 100 blocks.
	const FILTER_RETAIN_THRESHOLD: u64 = 100;
	task_manager.spawn_essential_handle().spawn(
		"frontier-filter-pool",
		None,
		EthTask::filter_pool_task(client.clone(), filter_pool, FILTER_RETAIN_THRESHOLD),
	);

	// Spawn Frontier FeeHistory cache maintenance task.
	task_manager.spawn_essential_handle().spawn(
		"frontier-fee-history",
		None,
		EthTask::fee_history_task(
			client,
			overrides,
			fee_history_cache,
			2048, // TODO: fee_history_cache_limit
		),
	);
}
