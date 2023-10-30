use std::sync::Arc;

use cfg_primitives::BlockNumber;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use pallet_anchors::AnchorData;
pub use runtime_common::apis::AnchorApi as AnchorRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{traits::Block as BlockT};

use crate::rpc::invalid_params_error;

#[rpc(client, server)]
pub trait AnchorApi<IdHash, BlockHash> {
	/// Returns an anchor given an anchor id from the runtime storage
	#[method(name = "anchor_getAnchorById")]
	fn get_anchor_by_id(
		&self,
		id: IdHash,
		at: Option<BlockHash>,
	) -> RpcResult<AnchorData<IdHash, BlockNumber>>;
}

/// A struct that implements the `AnchorApi`.
pub struct Anchors<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> Anchors<C, P> {
	/// Create new `Anchor` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Self {
			client,
			_marker: Default::default(),
		}
	}
}

impl<C, Block> AnchorApiServer<cfg_primitives::Hash, Block::Hash> for Anchors<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: AnchorRuntimeApi<Block, cfg_primitives::Hash, BlockNumber>,
{
	fn get_anchor_by_id(
		&self,
		id: cfg_primitives::Hash,
		at: Option<Block::Hash>,
	) -> RpcResult<AnchorData<cfg_primitives::Hash, BlockNumber>> {
		let api = self.client.runtime_api();
		let hash = match at {
			Some(hash) => hash,
			None => self.client.info().best_hash,
		};

		api.get_anchor_by_id(hash, id)
			.ok()
			.unwrap()
			.ok_or_else(|| invalid_params_error("Unable to find anchor"))
	}
}
