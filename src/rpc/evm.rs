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

// Frontier
use fc_db::Backend as FrontierBackend;
pub use fc_rpc::{
	EthBlockDataCacheTask, OverrideHandle, RuntimeApiStorageOverride, SchemaV1Override,
	SchemaV2Override, SchemaV3Override, StorageOverride,
};
pub use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use fp_rpc::{ConvertTransaction, ConvertTransactionRuntimeApi, EthereumRuntimeRPCApi};
use fp_storage::EthereumStorageSchema;
use jsonrpsee::RpcModule;
// Substrate
use sc_client_api::{
	backend::{AuxStore, Backend, StateBackend, StorageProvider},
	client::BlockchainEvents,
};
use sc_network::NetworkService;
use sc_rpc::SubscriptionTaskExecutor;
use sc_transaction_pool::{ChainApi, Pool};
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};

/// Extra dependencies for Ethereum compatibility.
pub struct Deps<C, P, A: ChainApi, CT, B: BlockT> {
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
	/// Frontier Backend.
	pub frontier_backend: Arc<FrontierBackend<B>>,
	/// Ethereum data access overrides.
	pub overrides: Arc<OverrideHandle<B>>,
	/// Cache for Ethereum block data.
	pub block_data_cache: Arc<EthBlockDataCacheTask<B>>,
	/// EthFilterApi pool.
	pub filter_pool: FilterPool,
	/// Maximum number of logs in a query.
	pub max_past_logs: u32,
	/// Fee history cache.
	pub fee_history_cache: FeeHistoryCache,
	/// Maximum fee history cache size.
	pub fee_history_cache_limit: FeeHistoryCacheLimit,
	/// Maximum allowed gas limit will be ` block.gas_limit *
	/// execute_gas_limit_multiplier` when using eth_call/eth_estimateGas.
	pub execute_gas_limit_multiplier: u64,
}

impl<C, P, A: ChainApi, CT: Clone, B: BlockT> Clone for Deps<C, P, A, CT, B> {
	fn clone(&self) -> Self {
		Self {
			client: self.client.clone(),
			pool: self.pool.clone(),
			graph: self.graph.clone(),
			converter: self.converter.clone(),
			is_authority: self.is_authority,
			enable_dev_signer: self.enable_dev_signer,
			network: self.network.clone(),
			frontier_backend: self.frontier_backend.clone(),
			overrides: self.overrides.clone(),
			block_data_cache: self.block_data_cache.clone(),
			filter_pool: self.filter_pool.clone(),
			max_past_logs: self.max_past_logs,
			fee_history_cache: self.fee_history_cache.clone(),
			fee_history_cache_limit: self.fee_history_cache_limit,
			execute_gas_limit_multiplier: self.execute_gas_limit_multiplier,
		}
	}
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
		Box::new(SchemaV1Override::new(client.clone()))
			as Box<dyn StorageOverride<_> + Send + Sync>,
	);
	overrides_map.insert(
		EthereumStorageSchema::V2,
		Box::new(SchemaV2Override::new(client.clone()))
			as Box<dyn StorageOverride<_> + Send + Sync>,
	);
	overrides_map.insert(
		EthereumStorageSchema::V3,
		Box::new(SchemaV3Override::new(client.clone()))
			as Box<dyn StorageOverride<_> + Send + Sync>,
	);

	Arc::new(OverrideHandle {
		schemas: overrides_map,
		fallback: Box::new(RuntimeApiStorageOverride::new(client)),
	})
}

/// Instantiate Ethereum-compatible RPC extensions.
pub fn create<C, BE, P, A, CT, B>(
	mut io: RpcModule<()>,
	deps: Deps<C, P, A, CT, B>,
	subscription_task_executor: SubscriptionTaskExecutor,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
	B: BlockT<Hash = H256>,
	C: ProvideRuntimeApi<B>,
	C::Api: BlockBuilderApi<B> + EthereumRuntimeRPCApi<B> + ConvertTransactionRuntimeApi<B>,
	C: BlockchainEvents<B> + 'static,
	C: HeaderBackend<B> + HeaderMetadata<B, Error = BlockChainError> + StorageProvider<B, BE>,
	BE: Backend<B> + 'static,
	BE::State: StateBackend<BlakeTwo256>,
	P: TransactionPool<Block = B> + 'static,
	A: ChainApi<Block = B> + 'static,
	CT: ConvertTransaction<<B as BlockT>::Extrinsic> + Send + Sync + 'static,
{
	use fc_rpc::{
		Eth, EthApiServer, EthDevSigner, EthFilter, EthFilterApiServer, EthPubSub,
		EthPubSubApiServer, EthSigner, Net, NetApiServer, Web3, Web3ApiServer,
	};

	let Deps {
		client,
		pool,
		graph,
		converter,
		is_authority,
		enable_dev_signer,
		network,
		frontier_backend,
		overrides,
		block_data_cache,
		filter_pool,
		max_past_logs,
		fee_history_cache,
		fee_history_cache_limit,
		execute_gas_limit_multiplier,
	} = deps;

	let mut signers = Vec::new();
	if enable_dev_signer {
		signers.push(Box::new(EthDevSigner::new()) as Box<dyn EthSigner>);
	}

	io.merge(
		Eth::new(
			client.clone(),
			pool.clone(),
			graph,
			converter,
			network.clone(),
			vec![],
			overrides.clone(),
			frontier_backend.clone(),
			is_authority,
			block_data_cache.clone(),
			fee_history_cache,
			fee_history_cache_limit,
			execute_gas_limit_multiplier,
		)
		.into_rpc(),
	)?;

	io.merge(
		EthFilter::new(
			client.clone(),
			frontier_backend,
			filter_pool,
			500_usize, // max stored filters
			max_past_logs,
			block_data_cache,
		)
		.into_rpc(),
	)?;

	io.merge(
		EthPubSub::new(
			pool,
			client.clone(),
			network.clone(),
			subscription_task_executor,
			overrides,
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
