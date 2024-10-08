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

use std::{collections::BTreeMap, sync::Arc};

use fc_rpc::pending;
pub use fc_rpc::{
	EthBlockDataCacheTask, OverrideHandle, RuntimeApiStorageOverride, SchemaV1Override,
	SchemaV2Override, SchemaV3Override, StorageOverride,
};
pub use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use fp_rpc::{ConvertTransaction, ConvertTransactionRuntimeApi, EthereumRuntimeRPCApi};
use fp_storage::EthereumStorageSchema;
use jsonrpsee::RpcModule;
use sc_client_api::{
	backend::{AuxStore, Backend, StateBackend, StorageProvider},
	client::BlockchainEvents,
	UsageProvider,
};
use sc_network::NetworkService;
use sc_network_sync::SyncingService;
use sc_rpc::SubscriptionTaskExecutor;
use sc_transaction_pool::{ChainApi, Pool};
use sc_transaction_pool_api::TransactionPool;
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_consensus_aura::{sr25519::AuthorityId as AuraId, AuraApi};
use sp_core::H256;
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};

pub struct CentrifugeEthConfig<B, C, BE>(std::marker::PhantomData<(B, C, BE)>);
impl<B, C, BE> fc_rpc::EthConfig<B, C> for CentrifugeEthConfig<B, C, BE>
where
	B: BlockT,
	C: sc_client_api::StorageProvider<B, BE> + Sync + Send + 'static,
	BE: Backend<B> + 'static,
{
	// This type is intended to override (i.e. adapt) evm calls to precompiles for
	// proper gas estimation.
	//
	// NOTE: Not used by our precompiles right now. Therefore, no need to provide
	// impl.
	type EstimateGasAdapter = ();
	// Assumes the use of HashedMapping<BlakeTwo256> for address mapping
	type RuntimeStorageOverride =
		fc_rpc::frontier_backend_client::SystemAccountId32StorageOverride<B, C, BE>;
}

/// Extra dependencies for Ethereum compatibility.
pub struct EvmDeps<C, P, A: ChainApi, CT, B: BlockT, CIDP> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Graph pool instance.
	pub graph: Arc<Pool<A>>,
	/// Ethereum transaction converter.
	pub converter: Option<CT>,
	/// The Node authority flag
	pub is_authority: bool,
	/// Whether to enable dev signer
	pub enable_dev_signer: bool,
	/// Network service
	pub network: Arc<NetworkService<B, B::Hash>>,
	/// Chain syncing service
	pub sync: Arc<SyncingService<B>>,
	/// Frontier Backend.
	pub frontier_backend: Arc<dyn fc_api::Backend<B>>,
	/// Ethereum data access overrides.
	pub overrides: Arc<OverrideHandle<B>>,
	/// Cache for Ethereum block data.
	pub block_data_cache: Arc<EthBlockDataCacheTask<B>>,
	/// EthFilterApi pool.
	pub filter_pool: Option<FilterPool>,
	/// Maximum number of logs in a query.
	pub max_past_logs: u32,
	/// Fee history cache.
	pub fee_history_cache: FeeHistoryCache,
	/// Maximum fee history cache size.
	pub fee_history_cache_limit: FeeHistoryCacheLimit,
	/// Maximum allowed gas limit will be ` block.gas_limit *
	/// execute_gas_limit_multiplier` when using eth_call/eth_estimateGas.
	pub execute_gas_limit_multiplier: u64,
	/// Mandated parent hashes for a given block hash.
	pub forced_parent_hashes: Option<BTreeMap<H256, H256>>,
	/// Something that can create the inherent data providers for pending state
	pub pending_create_inherent_data_providers: CIDP,
	/// Something that can create the consensus data providers for pending state
	pub pending_consensus_data_provider: Option<Box<dyn pending::ConsensusDataProvider<B>>>,
}

pub fn overrides_handle<B: BlockT<Hash = H256>, C, BE>(client: Arc<C>) -> Arc<OverrideHandle<B>>
where
	C: ProvideRuntimeApi<B> + StorageProvider<B, BE> + AuxStore,
	C: HeaderBackend<B> + HeaderMetadata<B, Error = BlockChainError>,
	C: Send + Sync + 'static,
	C::Api: sp_api::ApiExt<B>
		+ fp_rpc::EthereumRuntimeRPCApi<B>
		+ fp_rpc::ConvertTransactionRuntimeApi<B>,
	BE: Backend<B> + 'static,
	BE::State: StateBackend<BlakeTwo256>,
{
	let mut overrides_map = BTreeMap::new();
	overrides_map.insert(
		EthereumStorageSchema::V1,
		Box::new(SchemaV1Override::new(client.clone())) as Box<dyn StorageOverride<_>>,
	);
	overrides_map.insert(
		EthereumStorageSchema::V2,
		Box::new(SchemaV2Override::new(client.clone())) as Box<dyn StorageOverride<_>>,
	);
	overrides_map.insert(
		EthereumStorageSchema::V3,
		Box::new(SchemaV3Override::new(client.clone())) as Box<dyn StorageOverride<_>>,
	);

	Arc::new(OverrideHandle {
		schemas: overrides_map,
		fallback: Box::new(RuntimeApiStorageOverride::new(client)),
	})
}

pub fn create<C, BE, P, A, CT, B, CIDP>(
	mut io: RpcModule<()>,
	deps: EvmDeps<C, P, A, CT, B, CIDP>,
	subscription_task_executor: SubscriptionTaskExecutor,
	pubsub_notification_sinks: Arc<
		fc_mapping_sync::EthereumBlockNotificationSinks<
			fc_mapping_sync::EthereumBlockNotification<B>,
		>,
	>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
	B: BlockT<Hash = H256>,
	C: ProvideRuntimeApi<B>
		+ BlockchainEvents<B>
		+ 'static
		+ HeaderBackend<B>
		+ HeaderMetadata<B, Error = BlockChainError>
		+ StorageProvider<B, BE>
		+ CallApiAt<B>
		+ AuxStore
		+ UsageProvider<B>
		+ StorageProvider<B, BE>,
	C::Api: BlockBuilderApi<B>
		+ EthereumRuntimeRPCApi<B>
		+ ConvertTransactionRuntimeApi<B>
		+ AuraApi<B, AuraId>,
	BE: Backend<B> + 'static,
	BE::State: StateBackend<BlakeTwo256>,
	P: TransactionPool<Block = B> + 'static,
	A: ChainApi<Block = B> + 'static,
	CT: ConvertTransaction<<B as BlockT>::Extrinsic> + Send + Sync + 'static,
	CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
{
	use fc_rpc::{
		Eth, EthApiServer, EthDevSigner, EthFilter, EthFilterApiServer, EthPubSub,
		EthPubSubApiServer, EthSigner, Net, NetApiServer, Web3, Web3ApiServer,
	};

	let EvmDeps {
		client,
		pool,
		graph,
		converter: _converter,
		sync,
		is_authority,
		enable_dev_signer,
		network,
		overrides,
		frontier_backend,
		block_data_cache,
		fee_history_cache,
		fee_history_cache_limit,
		execute_gas_limit_multiplier,
		filter_pool,
		max_past_logs,
		forced_parent_hashes,
		pending_create_inherent_data_providers,
		pending_consensus_data_provider,
	} = deps;

	let mut signers = Vec::new();
	if enable_dev_signer {
		signers.push(Box::new(EthDevSigner::new()) as Box<dyn EthSigner>);
	}

	enum Never {}
	impl<T> fp_rpc::ConvertTransaction<T> for Never {
		fn convert_transaction(&self, _transaction: pallet_ethereum::Transaction) -> T {
			// The Never type is not instantiable, but this method requires the type to be
			// instantiated to be called (`&self` parameter), so if the code compiles we
			// have the guarantee that this function will never be called.
			unreachable!()
		}
	}
	let convert_transaction: Option<Never> = None;

	io.merge(
		Eth::<_, _, _, _, _, _, _, ()>::new(
			Arc::clone(&client),
			Arc::clone(&pool),
			graph.clone(),
			convert_transaction,
			Arc::clone(&sync),
			signers,
			Arc::clone(&overrides),
			Arc::clone(&frontier_backend),
			is_authority,
			Arc::clone(&block_data_cache),
			fee_history_cache,
			fee_history_cache_limit,
			execute_gas_limit_multiplier,
			forced_parent_hashes,
			pending_create_inherent_data_providers,
			pending_consensus_data_provider,
		)
		.replace_config::<CentrifugeEthConfig<B, C, BE>>()
		.into_rpc(),
	)?;

	if let Some(filter_pool) = filter_pool {
		io.merge(
			EthFilter::new(
				client.clone(),
				frontier_backend.clone(),
				graph.clone(),
				filter_pool,
				500_usize,
				max_past_logs,
				block_data_cache,
			)
			.into_rpc(),
		)?;
	}

	io.merge(
		EthPubSub::new(
			pool,
			client.clone(),
			sync,
			subscription_task_executor,
			overrides,
			pubsub_notification_sinks,
		)
		.into_rpc(),
	)?;

	io.merge(
		Net::new(
			client.clone(),
			network,
			// Whether to format the `peer_count` response as Hex (default) or not.
			true,
		)
		.into_rpc(),
	)?;

	io.merge(Web3::new(client).into_rpc())?;

	Ok(io)
}
