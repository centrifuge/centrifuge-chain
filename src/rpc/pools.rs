use std::{fmt::Debug, sync::Arc};

use codec::Codec;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use pallet_pool_system::{
	tranches::{TrancheIndex, TrancheLoc, TrancheSolution},
	EpochSolution,
};
use runtime_common::apis::PoolsApi as PoolsRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

use crate::rpc::{invalid_params_error, runtime_error};

#[rpc(client, server)]
pub trait PoolsApi<PoolId, TrancheId, Balance, Currency, BalanceRatio, BlockHash> {
	#[method(name = "pools_currency")]
	fn currency(&self, poold_id: PoolId, at: Option<BlockHash>) -> RpcResult<Currency>;

	#[method(name = "pools_inspectEpochSolution")]
	fn inspect_epoch_solution(
		&self,
		pool_id: PoolId,
		solution: Vec<TrancheSolution>,
		at: Option<BlockHash>,
	) -> RpcResult<EpochSolution<Balance>>;

	#[method(name = "pools_trancheTokenPrice")]
	fn tranche_token_price(
		&self,
		pool_id: PoolId,
		tranche: TrancheId,
		at: Option<BlockHash>,
	) -> RpcResult<BalanceRatio>;

	#[method(name = "pools_trancheTokenPrices")]
	fn tranche_token_prices(
		&self,
		pool_id: PoolId,
		at: Option<BlockHash>,
	) -> RpcResult<Vec<BalanceRatio>>;

	#[method(name = "pools_trancheIds")]
	fn tranche_ids(&self, pool_id: PoolId, at: Option<BlockHash>) -> RpcResult<Vec<TrancheId>>;

	#[method(name = "pools_trancheId")]
	fn tranche_id(
		&self,
		pool_id: PoolId,
		tranche_index: TrancheIndex,
		at: Option<BlockHash>,
	) -> RpcResult<TrancheId>;

	#[method(name = "pools_trancheCurrency")]
	fn tranche_currency(
		&self,
		pool_id: PoolId,
		tranche_id: TrancheId,
		at: Option<BlockHash>,
	) -> RpcResult<Currency>;
}

pub struct Pools<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> Pools<C, P> {
	pub fn new(client: Arc<C>) -> Self {
		Pools {
			client,
			_marker: Default::default(),
		}
	}
}

impl<C, Block, PoolId, TrancheId, Balance, Currency, BalanceRatio>
	PoolsApiServer<PoolId, TrancheId, Balance, Currency, BalanceRatio, Block::Hash> for Pools<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: PoolsRuntimeApi<Block, PoolId, TrancheId, Balance, Currency, BalanceRatio>,
	Balance: Codec + Copy,
	PoolId: Codec + Copy + Debug,
	TrancheId: Codec + Clone + Debug,
	Currency: Codec,
	BalanceRatio: Codec,
{
	fn currency(&self, pool_id: PoolId, at: Option<Block::Hash>) -> RpcResult<Currency> {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		api.currency(&at, pool_id)
			.map_err(|e| runtime_error("Unable to query pool currency", e))?
			.ok_or_else(|| invalid_params_error("Pool not found"))
	}

	fn inspect_epoch_solution(
		&self,
		pool_id: PoolId,
		solution: Vec<TrancheSolution>,
		at: Option<Block::Hash>,
	) -> RpcResult<EpochSolution<Balance>> {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		api.inspect_epoch_solution(&at, pool_id, solution)
			.map_err(|e| runtime_error("Unable to query inspection for epoch solution", e))?
			.ok_or_else(|| invalid_params_error("Pool not found or invalid solution"))
	}

	fn tranche_token_price(
		&self,
		pool_id: PoolId,
		tranche_id: TrancheId,
		at: Option<Block::Hash>,
	) -> RpcResult<BalanceRatio> {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		api.tranche_token_price(&at, pool_id, TrancheLoc::Id(tranche_id))
			.map_err(|e| runtime_error("Unable to query tranche token price", e))?
			.ok_or_else(|| invalid_params_error("Pool or tranche not found"))
	}

	fn tranche_token_prices(
		&self,
		pool_id: PoolId,
		at: Option<Block::Hash>,
	) -> RpcResult<Vec<BalanceRatio>> {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		api.tranche_token_prices(&at, pool_id)
			.map_err(|e| runtime_error("Unable to query tranche token prices.", e))?
			.ok_or_else(|| invalid_params_error("Pool not found."))
	}

	fn tranche_ids(&self, pool_id: PoolId, at: Option<Block::Hash>) -> RpcResult<Vec<TrancheId>> {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		api.tranche_ids(&at, pool_id)
			.map_err(|e| runtime_error("Unable to query tranche ids.", e))?
			.ok_or_else(|| invalid_params_error("Pool not found"))
	}

	fn tranche_id(
		&self,
		pool_id: PoolId,
		tranche_index: TrancheIndex,
		at: Option<Block::Hash>,
	) -> RpcResult<TrancheId> {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		api.tranche_id(&at, pool_id, tranche_index)
			.map_err(|e| runtime_error("Unable to query tranche ids.", e))?
			.ok_or_else(|| invalid_params_error("Pool or tranche not found."))
	}

	fn tranche_currency(
		&self,
		pool_id: PoolId,
		tranche_id: TrancheId,
		at: Option<Block::Hash>,
	) -> RpcResult<Currency> {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		api.tranche_currency(&at, pool_id, TrancheLoc::Id(tranche_id))
			.map_err(|e| runtime_error("Unable to query tranche currency.", e))?
			.ok_or_else(|| invalid_params_error("Pool or tranche not found."))
	}
}
