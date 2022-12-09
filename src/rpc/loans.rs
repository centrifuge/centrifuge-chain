use std::{fmt::Debug, sync::Arc};

use codec::Codec;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use runtime_common::apis::LoansApi as LoansRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

use crate::rpc::{invalid_params_error, runtime_error};

#[rpc(client, server)]
pub trait LoansApi<PoolId, Balance, BlockHash> {
	#[method(name = "loans_pool_valuation")]
	fn pool_valuation(&self, pool_id: PoolId, at: Option<BlockHash>) -> RpcResult<Balance>;
}

pub struct Loans<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> Loans<C, P> {
	pub fn new(client: Arc<C>) -> Self {
		Loans {
			client,
			_marker: Default::default(),
		}
	}
}

impl<C, Block, PoolId, Balance> LoansApiServer<PoolId, Balance, Block::Hash> for Loans<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: LoansRuntimeApi<Block, PoolId, Balance>,
	Balance: Codec + Copy,
	PoolId: Codec + Copy + Debug,
{
	fn pool_valuation(&self, pool_id: PoolId, at: Option<Block::Hash>) -> RpcResult<Balance> {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		api.pool_valuation(&at, pool_id)
			.map_err(|e| runtime_error("Unable to query pool valuation.", e))?
			.ok_or_else(|| invalid_params_error("Pool not found."))
	}
}
