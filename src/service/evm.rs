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
use cumulus_client_consensus_common::{ParachainBlockImportMarker, ParachainConsensus};
use cumulus_client_network::BlockAnnounceValidator;
use cumulus_client_service::{
	build_relay_chain_interface, prepare_node_config, start_collator, start_full_node,
	StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_interface::RelayChainInterface;
use fc_consensus::Error;
use fc_db::Backend as FrontierBackend;
use fc_mapping_sync::{kv::MappingSyncWorker, SyncStrategy};
use fc_rpc::{EthBlockDataCacheTask, EthTask, OverrideHandle};
use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use fp_consensus::ensure_log;
use fp_rpc::{ConvertTransactionRuntimeApi, EthereumRuntimeRPCApi};
use futures::{future, StreamExt};
use polkadot_cli::Cli;
use sc_cli::SubstrateCli;
use sc_client_api::{backend::AuxStore, BlockOf, BlockchainEvents};
use sc_consensus::{
	BlockCheckParams, BlockImport as BlockImportT, BlockImportParams, ImportQueue, ImportResult,
};
use sc_network::{NetworkBlock, NetworkService};
use sc_network_sync::SyncingService;
use sc_rpc::SubscriptionTaskExecutor;
use sc_rpc_api::DenyUnsafe;
use sc_service::{BasePath, Configuration, PartialComponents, TFullBackend, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sp_api::{ConstructRuntimeApi, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{HeaderBackend};
use sp_consensus::Error as ConsensusError;
use sp_keystore::KeystorePtr;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Header as HeaderT};
use substrate_prometheus_endpoint::Registry;

use super::{rpc, FullBackend, FullClient, HostFunctions, ParachainBlockImport};

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
	I: BlockImportT<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
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
	I: BlockImportT<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + BlockOf,
	C::Api: EthereumRuntimeRPCApi<B>,
	C::Api: BlockBuilderApi<B>,
{
	type Error = ConsensusError;
	type Transaction = sp_api::TransactionFor<C, B>;

	async fn check_block(
		&mut self,
		block: BlockCheckParams<B>,
	) -> Result<ImportResult, Self::Error> {
		self.inner.check_block(block).await.map_err(Into::into)
	}

	async fn import_block(
		&mut self,
		block: BlockImportParams<B, Self::Transaction>,
	) -> Result<ImportResult, Self::Error> {
		// Validate that there is one and exactly one frontier log,
		// but only on blocks created after frontier was enabled.
		if *block.header.number() >= self.first_evm_block {
			ensure_log(block.header.digest()).map_err(Error::from)?;
		}
		self.inner
			.import_block(block)
			.await
			.map_err(Into::into)
	}
}

impl<B: BlockT, I, C> ParachainBlockImportMarker for BlockImport<B, I, C> {}

fn db_config_dir(config: &Configuration) -> PathBuf {
	config.base_path.config_dir(config.chain_spec.id())

	// todo!("nuno: not sure when this unwrap_or_else was ever called if `config.base_path` always returns?
	// .unwrap_or_else(|| {
	// 	BasePath::from_project("", "", &Cli::executable_name())
	// 		.config_dir(config.chain_spec.id())
	// })
}

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the
/// builder in order to be able to perform chain operations.
#[allow(clippy::type_complexity)]
pub fn new_partial<RuntimeApi, BIQ>(
	config: &Configuration,
	first_evm_block: BlockNumber,
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
			FrontierBackend<Block>,
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
		FrontierBackend<Block>,
		BlockNumber,
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
#[sc_tracing::logging::prefix_logs_with("Parachain")]
pub(crate) async fn start_node_impl<RuntimeApi, Executor, RB, BIQ, BIC>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	eth_config: EthConfiguration,
	collator_options: CollatorOptions,
	id: ParaId,
	first_evm_block: BlockNumber,
	rpc_ext_builder: RB,
	build_import_queue: BIQ,
	build_consensus: BIC,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi>>)>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::Metadata<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ cumulus_primitives_core::CollectCollationInfo<Block>
		+ EthereumRuntimeRPCApi<Block>
		+ ConvertTransactionRuntimeApi<Block>,
	sc_client_api::StateBackendFor<FullBackend, Block>: sp_api::StateBackend<BlakeTwo256>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
	RB: Fn(
			Arc<FullClient<RuntimeApi>>,
			Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>>,
			DenyUnsafe,
			SubscriptionTaskExecutor,
			Arc<NetworkService<Block, Hash>>,
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
		Arc<SyncingService<Block>>,
		KeystorePtr,
		bool,
	) -> Result<Box<dyn ParachainConsensus<Block>>, sc_service::Error>,
{
	let parachain_config = prepare_node_config(parachain_config);

	let params =
		new_partial::<RuntimeApi, BIQ>(&parachain_config, first_evm_block, build_import_queue)?;
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
	.map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

	let block_announce_validator = BlockAnnounceValidator::new(relay_chain_interface.clone(), id);

	let force_authoring = parachain_config.force_authoring;
	let validator = parachain_config.role.is_authority();
	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();
	let net_config = sc_network::config::FullNetworkConfiguration::new(&parachain_config.network);

	let (network, system_rpc_tx, tx_handler_controller, start_network, sync_service) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &parachain_config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue: params.import_queue,
			block_announce_validator_builder: Some(Box::new(|_| {
				Box::new(block_announce_validator)
			})),
			warp_sync_params: None
		})?;

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
				overrides.clone(),
				block_data_cache.clone(),
			)
		}
	};

	let pubsub_notification_sinks: fc_mapping_sync::EthereumBlockNotificationSinks<
		fc_mapping_sync::EthereumBlockNotification<Block>,
	> = Default::default();

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

	// do we need this at all?
	spawn_frontier_tasks::<RuntimeApi, Executor>(
		&task_manager,
		client.clone(),
		backend.clone(),
		frontier_backend.clone(),
		filter_pool.clone(),
		overrides,
		fee_history_cache.clone(),
		eth_config.fee_history_limit,
		sync_service.clone(),
		Arc::new(pubsub_notification_sinks)
	);

	let announce_block = {
		let sync_service = sync_service.clone();
		Arc::new(move |hash, data| sync_service.announce_block(hash, data))
	};
	let relay_chain_slot_duration = Duration::from_secs(6);
	let _overseer_handle = relay_chain_interface
		.overseer_handle()
		.map_err(|e| sc_service::Error::Application(Box::new(e)))?;

	let recovery_handle = Box::new(_overseer_handle);

	if validator {
		let parachain_consensus = build_consensus(
			client.clone(),
			block_import,
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface.clone(),
			transaction_pool,
			sync_service.clone(),
			params.keystore_container.keystore(),
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
			recovery_handle,
			sync_service,
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
			recovery_handle,
			sync_service,
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}

#[allow(clippy::too_many_arguments)]
fn spawn_frontier_tasks<RuntimeApi, Executor>(
	task_manager: &TaskManager,
	client: Arc<FullClient<RuntimeApi>>,
	backend: Arc<TFullBackend<Block>>,
	frontier_backend: FrontierBackend<Block>,
	filter_pool: FilterPool,
	overrides: Arc<OverrideHandle<Block>>,
	fee_history_cache: FeeHistoryCache,
	fee_history_cache_limit: FeeHistoryCacheLimit,
	sync: Arc<SyncingService<Block>>,
	pubsub_notification_sinks: Arc<
		fc_mapping_sync::EthereumBlockNotificationSinks<
			fc_mapping_sync::EthereumBlockNotification<Block>,
		>>,
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

	match frontier_backend {
		FrontierBackend::KeyValue(fb) => {
			task_manager.spawn_essential_handle().spawn(
				"frontier-mapping-sync-worker",
				None,
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
		},
		// nuno: do we want to handle this?
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
			fee_history_cache_limit,
		),
	);
}
