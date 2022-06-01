use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use pallet_pools::{EpochSolution, TrancheIndex, TrancheLoc, TrancheSolution};
use runtime_common::apis::PoolsApi as PoolsRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::fmt::Debug;
use std::sync::Arc;

#[rpc]
pub trait PoolsApi<PoolId, TrancheId, Balance, Currency, BalanceRatio> {
	#[rpc(name = "pools_currency")]
	fn currency(&self, poold_id: PoolId) -> Result<Currency>;

	#[rpc(name = "pools_inspectEpochSolution")]
	fn inspect_epoch_solution(
		&self,
		pool_id: PoolId,
		solution: Vec<TrancheSolution>,
	) -> Result<EpochSolution<Balance>>;

	#[rpc(name = "pools_trancheTokenPrice")]
	fn tranche_token_price(&self, pool_id: PoolId, tranche: TrancheId) -> Result<BalanceRatio>;

	#[rpc(name = "pools_trancheTokenPrices")]
	fn tranche_token_prices(&self, pool_id: PoolId) -> Result<Vec<BalanceRatio>>;

	#[rpc(name = "pools_trancheIds")]
	fn tranche_ids(&self, pool_id: PoolId) -> Result<Vec<TrancheId>>;

	#[rpc(name = "pools_trancheId")]
	fn tranche_id(&self, pool_id: PoolId, tranche_index: TrancheIndex) -> Result<TrancheId>;

	#[rpc(name = "pools_trancheCurrency")]
	fn tranche_currency(&self, pool_id: PoolId, tranche_id: TrancheId) -> Result<Currency>;
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
	PoolsApi<PoolId, TrancheId, Balance, Currency, BalanceRatio> for Pools<C, Block>
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
	fn currency(&self, pool_id: PoolId) -> Result<Currency> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.currency(&at, pool_id)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query currency of pool.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool not found.".into(),
				data: Some(format!("PoolId: {:?}", pool_id).into()),
			})
	}

	fn inspect_epoch_solution(
		&self,
		pool_id: PoolId,
		solution: Vec<TrancheSolution>,
	) -> Result<EpochSolution<Balance>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.inspect_epoch_solution(&at, pool_id, solution.clone())
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query inspection for epoch solution.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool not found or invalid solution.".into(),
				data: Some(format!("PoolId: {:?}, Solution: {:?}", pool_id, solution).into()),
			})
	}

	fn tranche_token_price(&self, pool_id: PoolId, tranche_id: TrancheId) -> Result<BalanceRatio> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.tranche_token_price(&at, pool_id, TrancheLoc::Id(tranche_id.clone()))
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query tranche token price.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool or tranche not found.".into(),
				data: Some(format!("PoolId: {:?}, TrancheId: {:?}", pool_id, tranche_id).into()),
			})
	}

	fn tranche_token_prices(&self, pool_id: PoolId) -> Result<Vec<BalanceRatio>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.tranche_token_prices(&at, pool_id)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query tranche token prices.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool not found.".into(),
				data: Some(format!("PoolId: {:?}", pool_id).into()),
			})
	}

	fn tranche_ids(&self, pool_id: PoolId) -> Result<Vec<TrancheId>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.tranche_ids(&at, pool_id)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query tranche ids.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool not found.".into(),
				data: Some(format!("PoolId: {:?}", pool_id).into()),
			})
	}

	fn tranche_id(&self, pool_id: PoolId, tranche_index: TrancheIndex) -> Result<TrancheId> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.tranche_id(&at, pool_id, tranche_index)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query tranche ids.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool or tranche not found.".into(),
				data: Some(
					format!("PoolId: {:?}, TrancheIndex: {:?}", pool_id, tranche_index).into(),
				),
			})
	}

	fn tranche_currency(&self, pool_id: PoolId, tranche_id: TrancheId) -> Result<Currency> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.tranche_currency(&at, pool_id, TrancheLoc::Id(tranche_id.clone()))
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query tranche currency.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool or tranche not found.".into(),
				data: Some(format!("PoolId: {:?}, TrancheId: {:?}", pool_id, tranche_id).into()),
			})
	}
}
