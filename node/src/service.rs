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

use std::{sync::Arc, time::Duration};

use cfg_primitives::{AccountId, AuraId, Balance, Block, BlockNumber, Hash, Nonce};
use cumulus_client_cli::CollatorOptions;
use cumulus_client_collator::service::CollatorService;
use cumulus_client_consensus_common::ParachainBlockImport as TParachainBlockImport;
use cumulus_client_consensus_proposer::Proposer;
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_interface::{OverseerHandle, RelayChainInterface};
use fc_db::Backend as FrontierBackend;
use fc_rpc::pending::{AuraConsensusDataProvider, ConsensusDataProvider};
use polkadot_primitives::CollatorPair;
use sc_executor::WasmExecutor;
use sc_network_sync::SyncingService;
use sc_service::{Configuration, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::TelemetryHandle;
use sp_api::ConstructRuntimeApi;
use sp_core::U256;
use sp_keystore::KeystorePtr;
use substrate_prometheus_endpoint::Registry;

use crate::rpc::{self};

pub(crate) mod evm;
use evm::EthConfiguration;

#[cfg(feature = "runtime-benchmarks")]
type HostFunctions = (
	sp_io::SubstrateHostFunctions,
	frame_benchmarking::benchmarking::HostFunctions,
);

#[cfg(not(feature = "runtime-benchmarks"))]
type HostFunctions = sp_io::SubstrateHostFunctions;

type FullClient<RuntimeApi> = TFullClient<Block, RuntimeApi, WasmExecutor<HostFunctions>>;

type FullBackend = TFullBackend<Block>;

type ParachainBlockImport<RuntimeApi> =
	TParachainBlockImport<Block, Arc<FullClient<RuntimeApi>>, FullBackend>;

pub trait RuntimeApiCollection:
	sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
	+ sp_api::ApiExt<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
	+ sp_api::Metadata<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
	+ fp_rpc::ConvertTransactionRuntimeApi<Block>
	+ fp_rpc::EthereumRuntimeRPCApi<Block>
	+ sp_consensus_aura::AuraApi<Block, AuraId>
	+ runtime_common::apis::AnchorApi<Block, Hash, BlockNumber>
	+ cumulus_primitives_core::CollectCollationInfo<Block>
{
}

impl<Api> RuntimeApiCollection for Api where
	Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::ApiExt<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ fp_rpc::ConvertTransactionRuntimeApi<Block>
		+ fp_rpc::EthereumRuntimeRPCApi<Block>
		+ sp_consensus_aura::AuraApi<Block, AuraId>
		+ runtime_common::apis::AnchorApi<Block, Hash, BlockNumber>
		+ cumulus_primitives_core::CollectCollationInfo<Block>
{
}

/// Start a generic parachain node.
pub async fn start_node<RuntimeApi>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	eth_config: EthConfiguration,
	collator_options: CollatorOptions,
	id: ParaId,
	hwbench: Option<sc_sysinfo::HwBench>,
	first_evm_block: BlockNumber,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi>>)>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection,
{
	let is_authority = parachain_config.role.is_authority();

	evm::start_node_impl::<RuntimeApi, _, _>(
		parachain_config,
		polkadot_config,
		eth_config,
		collator_options,
		id,
		hwbench,
		first_evm_block,
		// follows Moonbeam's create_full
		move |client,
		      pool,
		      deny_unsafe,
		      subscription_task_executor,
		      network,
		      sync_service,
		      frontier_backend,
		      filter_pool,
		      fee_history_cache,
		      overrides,
		      block_data_cache| {

            let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
            let target_gas_price = eth_config.target_gas_price;
            let pending_create_inherent_data_providers = move |_, ()| async move {
                let current = sp_timestamp::InherentDataProvider::from_system_time();
                let next_slot = current.timestamp().as_millis() + slot_duration.as_millis();
                let timestamp = sp_timestamp::InherentDataProvider::new(next_slot.into());
                let slot =
                    sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                        *timestamp,
                        slot_duration,
                    );
                let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
                Ok((slot, timestamp, dynamic_fee))
            };
			let pending_consensus_data_provider = Some(Box::new(AuraConsensusDataProvider::new(client.clone())) as Box<dyn ConsensusDataProvider<_>>);

			let module = rpc::create_full(client.clone(), pool.clone(), deny_unsafe)?;
			let eth_deps = rpc::evm::EvmDeps {
				client,
				pool: pool.clone(),
				graph: pool.pool().clone(),
				converter: Some(development_runtime::TransactionConverter),
				is_authority,
				enable_dev_signer: eth_config.enable_dev_signer,
				network,
				sync: sync_service.clone(),
				frontier_backend: match frontier_backend.clone() {
					fc_db::Backend::KeyValue(b) => Arc::new(b),
					#[cfg(feature = "sql")]
					fc_db::Backend::Sql(b) => Arc::new(b),
				},
				overrides,
				block_data_cache,
				filter_pool: Some(filter_pool),
				max_past_logs: eth_config.max_past_logs,
				fee_history_cache,
				fee_history_cache_limit: eth_config.fee_history_limit,
				execute_gas_limit_multiplier: eth_config.execute_gas_limit_multiplier,
				forced_parent_hashes: None,
				pending_create_inherent_data_providers,
				pending_consensus_data_provider
			};
			let module = rpc::evm::create(
				module,
				eth_deps,
				subscription_task_executor,
				Arc::new(Default::default()),
			)?;
			Ok(module)
		},
		build_import_queue::<RuntimeApi>,
	)
	.await
}

/// Builds a generic import queue. The runtime is specified via the generics.
///
/// NOTE: Almost entirely taken from Polkadot SDK.
#[allow(clippy::type_complexity)]
pub fn build_import_queue<RuntimeApi>(
	client: Arc<FullClient<RuntimeApi>>,
	block_import: ParachainBlockImport<RuntimeApi>,
	config: &Configuration,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	frontier_backend: FrontierBackend<Block>,
	first_evm_block: BlockNumber,
) -> Result<sc_consensus::DefaultImportQueue<Block>, sc_service::Error>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection,
{
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;
	let block_import = evm::BlockImport::new(
		block_import,
		first_evm_block,
		client.clone(),
		Arc::new(frontier_backend),
	);

	Ok(
		cumulus_client_consensus_aura::equivocation_import_queue::fully_verifying_import_queue::<
			sp_consensus_aura::sr25519::AuthorityPair,
			_,
			_,
			_,
			_,
		>(
			client,
			block_import,
			move |_, _| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
				Ok(timestamp)
			},
			slot_duration,
			&task_manager.spawn_essential_handle(),
			config.prometheus_registry(),
			telemetry,
		),
	)
}

/// Starts the aura consensus.
///
/// NOTE: Taken from Polkadot SDK because Moonbeam uses their custom Nimbus
/// consensus
#[allow(clippy::too_many_arguments)]
fn start_consensus<RuntimeApi>(
	client: Arc<FullClient<RuntimeApi>>,
	block_import: ParachainBlockImport<RuntimeApi>,
	prometheus_registry: Option<&Registry>,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	relay_chain_interface: Arc<dyn RelayChainInterface>,
	transaction_pool: Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>>,
	sync_oracle: Arc<SyncingService<Block>>,
	keystore: KeystorePtr,
	relay_chain_slot_duration: Duration,
	para_id: ParaId,
	collator_key: CollatorPair,
	overseer_handle: OverseerHandle,
	announce_block: Arc<dyn Fn(Hash, Option<Vec<u8>>) + Send + Sync>,
) -> Result<(), sc_service::Error>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection,
{
	use cumulus_client_consensus_aura::collators::basic::{
		self as basic_aura, Params as BasicAuraParams,
	};

	// NOTE: because we use Aura here explicitly, we can use
	// `CollatorSybilResistance::Resistant` when starting the network.

	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

	let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
		task_manager.spawn_handle(),
		client.clone(),
		transaction_pool,
		prometheus_registry,
		telemetry.clone(),
	);

	let proposer = Proposer::new(proposer_factory);

	let collator_service = CollatorService::new(
		client.clone(),
		Arc::new(task_manager.spawn_handle()),
		announce_block,
		client.clone(),
	);

	let params = BasicAuraParams {
		create_inherent_data_providers: move |_, ()| async move { Ok(()) },
		block_import,
		para_client: client,
		relay_client: relay_chain_interface,
		sync_oracle,
		keystore,
		collator_key,
		para_id,
		overseer_handle,
		slot_duration,
		relay_chain_slot_duration,
		proposer,
		collator_service,
		// Very limited proposal time.
		authoring_duration: Duration::from_millis(500),
		collation_request_receiver: None,
	};

	let fut =
		basic_aura::run::<Block, sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _, _>(
			params,
		);
	task_manager
		.spawn_essential_handle()
		.spawn("aura", None, fut);

	Ok(())
}
