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

use std::{
	collections::BTreeMap,
	marker::PhantomData,
	path::PathBuf,
	sync::{Arc, Mutex},
	time::Duration,
};

use cfg_primitives::{Block, BlockNumber, Hash};
use cumulus_client_cli::CollatorOptions;
use cumulus_client_consensus_common::ParachainBlockImportMarker;
use cumulus_client_service::{
	build_network, build_relay_chain_interface, prepare_node_config, start_relay_chain_tasks,
	BuildNetworkParams, CollatorSybilResistance, DARecoveryProfile, StartRelayChainTasksParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_interface::RelayChainInterface;
use fc_consensus::Error;
use fc_db::Backend as FrontierBackend;
use fc_mapping_sync::{kv::MappingSyncWorker, SyncStrategy};
use fc_rpc::{EthBlockDataCacheTask, EthTask, OverrideHandle};
use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use fp_consensus::ensure_log;
use fp_rpc::EthereumRuntimeRPCApi;
use frame_benchmarking_cli::SUBSTRATE_REFERENCE_HARDWARE;
use futures::{future, StreamExt};
use sc_client_api::{backend::AuxStore, BlockOf, BlockchainEvents};
use sc_consensus::{
	BlockCheckParams, BlockImport as BlockImportT, BlockImportParams, ImportQueue, ImportResult,
};
use sc_network::{NetworkBlock, NetworkService};
use sc_network_sync::SyncingService;
use sc_rpc::SubscriptionTaskExecutor;
use sc_rpc_api::DenyUnsafe;
use sc_service::{Configuration, PartialComponents, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sp_api::{ConstructRuntimeApi, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::HeaderBackend;
use sp_consensus::Error as ConsensusError;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Header as HeaderT};

use super::{
	rpc, start_consensus, FullBackend, FullClient, ParachainBlockImport, RuntimeApiCollection,
};

/// The ethereum-compatibility configuration used to run a node.
#[derive(Clone, Copy, Debug, clap::Parser)]
pub struct EthConfiguration {
	/// Maximum number of logs in a query.
	#[clap(long, default_value = "10000")]
	pub max_past_logs: u32,

	/// Maximum fee history cache size.
	#[clap(long, default_value = "2048")]
	pub fee_history_limit: u64,

	#[clap(long)]
	pub enable_dev_signer: bool,

	/// The dynamic-fee pallet target gas price set by block author
	#[arg(long, default_value = "1")]
	pub target_gas_price: u64,

	/// Maximum allowed gas limit will be `block.gas_limit *
	/// execute_gas_limit_multiplier` when using eth_call/eth_estimateGas.
	#[clap(long, default_value = "10")]
	pub execute_gas_limit_multiplier: u64,

	/// Size in bytes of the LRU cache for block data.
	#[clap(long, default_value = "50")]
	pub eth_log_block_cache: usize,

	/// Size in bytes of the LRU cache for transactions statuses data.
	#[clap(long, default_value = "50")]
	pub eth_statuses_cache: usize,
}

type BlockNumberOf<B> = <<B as BlockT>::Header as HeaderT>::Number;

/// A re-implementation of the Frontier block importer, which can be
/// configured to only start filter for ethereum blocks after a
/// certain point.
#[derive(Clone)]
pub struct BlockImport<B: BlockT, I, C> {
	inner: I,
	first_evm_block: BlockNumberOf<B>,
	_client: Arc<C>,
	_backend: Arc<fc_db::Backend<B>>,
	_marker: PhantomData<B>,
}

impl<B, I, C> BlockImport<B, I, C>
where
	B: BlockT,
	I: BlockImportT<B> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + BlockOf,
{
	pub fn new(
		inner: I,
		first_evm_block: BlockNumberOf<B>,
		client: Arc<C>,
		backend: Arc<fc_db::Backend<B>>,
	) -> Self {
		Self {
			inner,
			first_evm_block,
			_client: client,
			_backend: backend,
			_marker: PhantomData,
		}
	}
}

#[async_trait::async_trait]
impl<B, I, C> BlockImportT<B> for BlockImport<B, I, C>
where
	B: BlockT,
	<B::Header as HeaderT>::Number: PartialOrd,
	I: BlockImportT<B> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + BlockOf,
	C::Api: EthereumRuntimeRPCApi<B>,
	C::Api: BlockBuilderApi<B>,
{
	type Error = ConsensusError;

	async fn check_block(
		&mut self,
		block: BlockCheckParams<B>,
	) -> Result<ImportResult, Self::Error> {
		self.inner.check_block(block).await.map_err(Into::into)
	}

	async fn import_block(
		&mut self,
		block: BlockImportParams<B>,
	) -> Result<ImportResult, Self::Error> {
		// Validate that there is one and exactly one frontier log,
		// but only on blocks created after frontier was enabled.
		if *block.header.number() >= self.first_evm_block {
			ensure_log(block.header.digest()).map_err(Error::from)?;
		}
		self.inner.import_block(block).await.map_err(Into::into)
	}
}

impl<B: BlockT, I, C> ParachainBlockImportMarker for BlockImport<B, I, C> {}

fn db_config_dir(config: &Configuration) -> PathBuf {
	config.base_path.config_dir(config.chain_spec.id())
}

/// Assembly of PartialComponents (enough to run chain ops subcommands)
///
/// NOTE: Based on Polkadot SDK
pub type Service<RuntimeApi> = PartialComponents<
	FullClient<RuntimeApi>,
	FullBackend,
	(),
	sc_consensus::DefaultImportQueue<Block>,
	sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>,
	(
		ParachainBlockImport<RuntimeApi>,
		Option<Telemetry>,
		Option<TelemetryWorkerHandle>,
		FrontierBackend<Block>,
		FilterPool,
		FeeHistoryCache,
	),
>;

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the
/// builder in order to be able to perform chain operations.
#[allow(clippy::type_complexity)]
pub fn new_partial<RuntimeApi, BIQ>(
	config: &Configuration,
	first_evm_block: BlockNumber,
	build_import_queue: BIQ,
) -> Result<Service<RuntimeApi>, sc_service::Error>
where
	RuntimeApi:
		ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection,
	sc_client_api::StateBackendFor<FullBackend, Block>: sc_client_api::StateBackend<BlakeTwo256>,
	BIQ: FnOnce(
		Arc<FullClient<RuntimeApi>>,
		ParachainBlockImport<RuntimeApi>,
		&Configuration,
		Option<TelemetryHandle>,
		&TaskManager,
		FrontierBackend<Block>,
		BlockNumber,
	) -> Result<sc_consensus::DefaultImportQueue<Block>, sc_service::Error>,
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

	let heap_pages =
		config
			.default_heap_pages
			.map_or(sc_executor::DEFAULT_HEAP_ALLOC_STRATEGY, |h| {
				sc_executor::HeapAllocStrategy::Static {
					extra_pages: h as _,
				}
			});

	let executor = sc_executor::WasmExecutor::builder()
		.with_execution_method(config.wasm_method)
		.with_onchain_heap_alloc_strategy(heap_pages)
		.with_offchain_heap_alloc_strategy(heap_pages)
		.with_max_runtime_instances(config.max_runtime_instances)
		.with_runtime_cache_size(config.runtime_cache_size)
		.build();

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

	let frontier_backend = FrontierBackend::KeyValue(fc_db::kv::Backend::open(
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
		first_evm_block,
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

/// Start a node with the given parachain `Configuration` and relay chain
/// `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the
/// runtime api.
#[allow(clippy::too_many_arguments)]
#[sc_tracing::logging::prefix_logs_with("🌀Parachain")]
pub(crate) async fn start_node_impl<RuntimeApi, RB, BIQ>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	eth_config: EthConfiguration,
	collator_options: CollatorOptions,
	para_id: ParaId,
	hwbench: Option<sc_sysinfo::HwBench>,
	first_evm_block: BlockNumber,
	rpc_ext_builder: RB,
	build_import_queue: BIQ,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi>>)>
where
	RuntimeApi:
		ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection,
	sc_client_api::StateBackendFor<FullBackend, Block>: sc_client_api::StateBackend<BlakeTwo256>,
	RB: Fn(
			Arc<FullClient<RuntimeApi>>,
			Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>>,
			DenyUnsafe,
			SubscriptionTaskExecutor,
			Arc<NetworkService<Block, Hash>>,
			Arc<SyncingService<Block>>,
			FrontierBackend<Block>,
			FilterPool,
			FeeHistoryCache,
			Arc<OverrideHandle<Block>>,
			Arc<EthBlockDataCacheTask<Block>>,
		) -> Result<rpc::RpcExtension, sc_service::Error>
		+ 'static,
	BIQ: FnOnce(
		Arc<FullClient<RuntimeApi>>,
		ParachainBlockImport<RuntimeApi>,
		&Configuration,
		Option<TelemetryHandle>,
		&TaskManager,
		FrontierBackend<Block>,
		BlockNumber,
	) -> Result<sc_consensus::DefaultImportQueue<Block>, sc_service::Error>,
{
	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial::<RuntimeApi, BIQ>(
		&parachain_config,
		first_evm_block,
		build_import_queue,
	)?;
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
		hwbench.clone(),
	)
	.await
	.map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

	let validator = parachain_config.role.is_authority();
	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();
	let net_config = sc_network::config::FullNetworkConfiguration::new(&parachain_config.network);

	let (network, system_rpc_tx, tx_handler_controller, start_network, sync_service) =
		build_network(BuildNetworkParams {
			parachain_config: &parachain_config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			para_id,
			spawn_handle: task_manager.spawn_handle(),
			relay_chain_interface: relay_chain_interface.clone(),
			import_queue: params.import_queue,
			sybil_resistance_level: CollatorSybilResistance::Resistant, // because of Aura
		})
		.await?;

	let rpc_client = client.clone();
	let pool = transaction_pool.clone();

	let overrides = rpc::evm::overrides_handle(client.clone());
	let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
		task_manager.spawn_handle(),
		overrides.clone(),
		eth_config.eth_log_block_cache,
		eth_config.eth_statuses_cache,
		prometheus_registry.clone(),
	));

	let rpc_builder = {
		let network = network.clone();
		let frontier_backend = frontier_backend.clone();
		let fee_history_cache = fee_history_cache.clone();
		let filter_pool = filter_pool.clone();
		let overrides = overrides.clone();
		let sync_service = sync_service.clone();
		move |deny, subscription_task_executor| {
			rpc_ext_builder(
				rpc_client.clone(),
				pool.clone(),
				deny,
				subscription_task_executor,
				network.clone(),
				sync_service.clone(),
				frontier_backend.clone(),
				filter_pool.clone(),
				fee_history_cache.clone(),
				overrides.clone(),
				block_data_cache.clone(),
			)
		}
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_builder: Box::new(rpc_builder),
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.keystore(),
		backend: backend.clone(),
		network: network.clone(),
		system_rpc_tx,
		tx_handler_controller,
		sync_service: sync_service.clone(),
		telemetry: telemetry.as_mut(),
	})?;

	spawn_frontier_tasks::<RuntimeApi>(
		&task_manager,
		client.clone(),
		backend.clone(),
		frontier_backend.clone(),
		filter_pool.clone(),
		overrides,
		fee_history_cache.clone(),
		eth_config.fee_history_limit,
		sync_service.clone(),
		Arc::new(Default::default()),
	);

	if let Some(hwbench) = hwbench {
		sc_sysinfo::print_hwbench(&hwbench);
		// Here you can check whether the hardware meets your chains' requirements.
		// Putting a link in there and swapping out the requirements for your own are
		// probably a good idea. The requirements for a para-chain are dictated by its
		// relay-chain.
		match SUBSTRATE_REFERENCE_HARDWARE.check_hardware(&hwbench) {
			Err(err) if validator => {
				log::warn!(
				"⚠️  The hardware does not meet the minimal requirements {} for role 'Authority'.",
				err
			);
			}
			_ => {}
		}

		if let Some(ref mut telemetry) = telemetry {
			let telemetry_handle = telemetry.handle();
			task_manager.spawn_handle().spawn(
				"telemetry_hwbench",
				None,
				sc_sysinfo::initialize_hwbench_telemetry(telemetry_handle, hwbench),
			);
		}
	}

	let announce_block = {
		let sync_service = sync_service.clone();
		Arc::new(move |hash, data| sync_service.announce_block(hash, data))
	};
	let relay_chain_slot_duration = Duration::from_secs(6);
	let overseer_handle = relay_chain_interface
		.overseer_handle()
		.map_err(|e| sc_service::Error::Application(Box::new(e)))?;

	// Follows Polkadot SDK
	start_relay_chain_tasks(StartRelayChainTasksParams {
		client: client.clone(),
		announce_block: announce_block.clone(),
		para_id,
		relay_chain_interface: relay_chain_interface.clone(),
		task_manager: &mut task_manager,
		da_recovery_profile: if validator {
			DARecoveryProfile::Collator
		} else {
			DARecoveryProfile::FullNode
		},
		import_queue: import_queue_service,
		relay_chain_slot_duration,
		recovery_handle: Box::new(overseer_handle.clone()),
		sync_service: sync_service.clone(),
	})?;

	// Follows Polkadot SDK
	if validator {
		start_consensus::<RuntimeApi>(
			client.clone(),
			block_import,
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface.clone(),
			transaction_pool,
			sync_service.clone(),
			params.keystore_container.keystore(),
			relay_chain_slot_duration,
			para_id,
			collator_key.expect("Command line arguments do not allow this. qed"),
			overseer_handle,
			announce_block,
		)?;
	}
	start_network.start_network();

	Ok((task_manager, client))
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::extra_unused_type_parameters)]
fn spawn_frontier_tasks<RuntimeApi>(
	task_manager: &TaskManager,
	client: Arc<FullClient<RuntimeApi>>,
	backend: Arc<FullBackend>,
	frontier_backend: FrontierBackend<Block>,
	filter_pool: FilterPool,
	overrides: Arc<OverrideHandle<Block>>,
	fee_history_cache: FeeHistoryCache,
	fee_history_cache_limit: FeeHistoryCacheLimit,
	sync: Arc<SyncingService<Block>>,
	pubsub_notification_sinks: Arc<
		fc_mapping_sync::EthereumBlockNotificationSinks<
			fc_mapping_sync::EthereumBlockNotification<Block>,
		>,
	>,
) where
	RuntimeApi:
		ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection,
{
	match frontier_backend {
		FrontierBackend::KeyValue(fb) => {
			task_manager.spawn_essential_handle().spawn(
				"frontier-mapping-sync-worker",
				Some("frontier"),
				MappingSyncWorker::new(
					client.import_notification_stream(),
					Duration::new(6, 0),
					client.clone(),
					backend,
					overrides.clone(),
					Arc::new(fb),
					3,
					0,
					SyncStrategy::Parachain,
					sync,
					pubsub_notification_sinks,
				)
				.for_each(|()| future::ready(())),
			);
		}
		#[cfg(feature = "sql")]
		fc_db::Backend::Sql(fb) => {
			task_manager.spawn_essential_handle().spawn_blocking(
				"frontier-mapping-sync-worker",
				Some("frontier"),
				fc_mapping_sync::sql::SyncWorker::run(
					client.clone(),
					backend.clone(),
					Arc::new(fb),
					client.import_notification_stream(),
					fc_mapping_sync::sql::SyncWorkerConfig {
						read_notification_timeout: Duration::from_secs(10),
						check_indexed_blocks_interval: Duration::from_secs(60),
					},
					fc_mapping_sync::SyncStrategy::Parachain,
					sync.clone(),
					pubsub_notification_sinks.clone(),
				),
			);
		}
	}

	// Spawn Frontier EthFilterApi maintenance task.
	// Each filter is allowed to stay in the pool for 100 blocks.
	const FILTER_RETAIN_THRESHOLD: u64 = 100;
	task_manager.spawn_essential_handle().spawn(
		"frontier-filter-pool",
		Some("frontier"),
		EthTask::filter_pool_task(client.clone(), filter_pool, FILTER_RETAIN_THRESHOLD),
	);

	// Spawn Frontier FeeHistory cache maintenance task.
	task_manager.spawn_essential_handle().spawn(
		"frontier-fee-history",
		Some("frontier"),
		EthTask::fee_history_task(
			client,
			overrides,
			fee_history_cache,
			fee_history_cache_limit,
		),
	);
}
