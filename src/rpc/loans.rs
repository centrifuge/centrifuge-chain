use std::sync::Arc;

use codec::Codec;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use runtime_common::apis::LoansApi as LoansRuntimeApi;
use sp_api::{ApiRef, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

use crate::rpc::{invalid_params_error, runtime_error};

#[rpc(client, server)]
pub trait LoansApi<PoolId, LoanId, Loan, BlockHash> {
	#[method(name = "loans_portfolio")]
	fn portfolio(&self, pool_id: PoolId, at: Option<BlockHash>) -> RpcResult<Vec<(LoanId, Loan)>>;

	#[method(name = "loans_portfolioLoan")]
	fn portfolio_loan(
		&self,
		pool_id: PoolId,
		loan_id: LoanId,
		at: Option<BlockHash>,
	) -> RpcResult<Loan>;
}

pub struct Loans<C, Block> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<Block>,
}

impl<C, Block> Loans<C, Block> {
	pub fn new(client: Arc<C>) -> Self {
		Self {
			client,
			_marker: Default::default(),
		}
	}
}

impl<C, Block> Loans<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
{
	pub fn api(&self, at: Option<Block::Hash>) -> (ApiRef<C::Api>, BlockId<Block>) {
		let api = self.client.runtime_api();
		let at = if let Some(hash) = at {
			BlockId::hash(hash)
		} else {
			BlockId::hash(self.client.info().best_hash)
		};

		(api, at)
	}
}

impl<C, Block, PoolId, LoanId, Loan> LoansApiServer<PoolId, LoanId, Loan, Block::Hash>
	for Loans<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: LoansRuntimeApi<Block, PoolId, LoanId, Loan>,
	PoolId: Codec,
	LoanId: Codec,
	Loan: Codec,
{
	fn portfolio(
		&self,
		pool_id: PoolId,
		at: Option<Block::Hash>,
	) -> RpcResult<Vec<(LoanId, Loan)>> {
		let (api, at) = self.api(at);

		api.portfolio(&at, pool_id)
			.map_err(|e| runtime_error("Unable to query portfolio", e))
	}

	fn portfolio_loan(
		&self,
		pool_id: PoolId,
		loan_id: LoanId,
		at: Option<Block::Hash>,
	) -> RpcResult<Loan> {
		let (api, at) = self.api(at);

		api.portfolio_loan(&at, pool_id, loan_id)
			.map_err(|e| runtime_error("Unable to query a loan from the portfolio", e))?
			.ok_or_else(|| invalid_params_error("Loan not found"))
	}
}
