use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use runtime_common::loans::LoansApi as LoansRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait LoansApi<PoolId, LoanId, Balance> {
	/// Returns an anchor given an anchor id from the runtime storage
	#[rpc(name = "loans_nav")]
	fn nav(&self, id: PoolId) -> Result<Balance>;

	#[rpc(name = "loans_maxBorrowAmount")]
	fn max_borrow_amount(&self, id: PoolId, loan_id: LoanId) -> Result<Balance>;
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

impl<C, Block, PoolId, LoanId, Balance> LoansApi<PoolId, LoanId, Balance> for Loans<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: LoansRuntimeApi<Block, PoolId, LoanId, Balance>,
	Balance: Codec + MaybeDisplay + Copy,
	LoanId: Codec + Copy,
	PoolId: Codec + Copy,
{
	fn nav(&self, pool_id: PoolId) -> Result<Balance> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let nav = api
			.nav(&at, pool_id)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query NAV of pool.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool not found.".into(),
				data: Some(format!("PoolId: {:?}", pool_id).into()),
			})?;

		Ok(nav)
	}

	fn max_borrow_amount(&self, pool_id: PoolId, loan_id: LoanId) -> Result<Balance> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let max_borrow_amount = api
			.nav(&at, pool_id)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query MaxBorrowAmount for loan.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: ErrorCode::InvalidParams,
				message: "Pool or loan not found.".into(),
				data: Some(format!("PoolId: {:?}, LoanId: {:?}", pool_id, loan_id).into()),
			})?;

		Ok(max_borrow_amount)
	}
}
