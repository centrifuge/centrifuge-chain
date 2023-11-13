use std::{fmt::Debug, sync::Arc};

use codec::Codec;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use runtime_common::apis::{RewardDomain, RewardsApi as RewardsRuntimeApi};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;

use crate::rpc::{invalid_params_error, runtime_error};

#[rpc(client, server)]
pub trait RewardsApi<AccountId, Balance, CurrencyId, BlockHash> {
	#[method(name = "rewards_listCurrencies")]
	fn list_currencies(
		&self,
		domain: RewardDomain,
		account_id: AccountId,
		at: Option<BlockHash>,
	) -> RpcResult<Vec<CurrencyId>>;

	#[method(name = "rewards_computeReward")]
	fn compute_reward(
		&self,
		domain: RewardDomain,
		currency_id: CurrencyId,
		account_id: AccountId,
		at: Option<BlockHash>,
	) -> RpcResult<Balance>;
}

pub struct Rewards<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> Rewards<C, P> {
	pub fn new(client: Arc<C>) -> Self {
		Rewards {
			client,
			_marker: Default::default(),
		}
	}
}

impl<C, Block, AccountId, Balance, CurrencyId>
	RewardsApiServer<AccountId, Balance, CurrencyId, Block::Hash> for Rewards<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: RewardsRuntimeApi<Block, AccountId, Balance, CurrencyId>,
	AccountId: Codec,
	Balance: Codec + Copy,
	CurrencyId: Codec + Copy + Debug,
{
	fn list_currencies(
		&self,
		domain: RewardDomain,
		account_id: AccountId,
		at: Option<Block::Hash>,
	) -> RpcResult<Vec<CurrencyId>> {
		let api = self.client.runtime_api();

		let hash = match at {
			Some(hash) => hash,
			None => self.client.info().best_hash,
		};

		api.list_currencies(hash, domain, account_id)
			.map_err(|e| runtime_error("Unable to list currencies", e))
	}

	fn compute_reward(
		&self,
		domain: RewardDomain,
		currency_id: CurrencyId,
		account_id: AccountId,
		at: Option<Block::Hash>,
	) -> RpcResult<Balance> {
		let api = self.client.runtime_api();

		let at = match at {
			Some(hash) => hash,
			None => self.client.info().best_hash,
		};

		api.compute_reward(at, domain, currency_id, account_id)
			.map_err(|e| runtime_error("Unable to compute reward", e))?
			.ok_or_else(|| invalid_params_error("Reward not found"))
	}
}
