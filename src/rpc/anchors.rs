use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use node_primitives::BlockNumber;
use pallet_anchors::AnchorData;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

use crate::rpc::invalid_params_error;

pub use runtime_common::AnchorApi as AnchorRuntimeApi;

#[rpc(client, server)]
pub trait AnchorApi<IdHash, BlockHash> {
	/// Returns an anchor given an anchor id from the runtime storage
	#[method(name = "anchor_getAnchorById")]
	fn get_anchor_by_id(
		&self,
		id: IdHash,
		at: Option<BlockId<BlockHash>>,
	) -> RpcResult<AnchorData<IdHash, BlockNumber>>;
}

/// A struct that implements the [`AnchorApi`].
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

impl<C, Block> AnchorApiServer for Anchors<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: AnchorRuntimeApi<Block, runtime_common::Hash, Block::Hash>,
{
	fn get_anchor_by_id(
		&self,
		id: runtime_common::Hash,
		at: Option<BlockId<Block::Hash>>,
	) -> RpcResult<AnchorData<runtime_common::Hash, BlockNumber>> {
		let api = self.client.runtime_api();
		let at = if let Some(block) = at {
			block
		} else {
			let best = self.client.info().best_hash;
			BlockId::hash(best);
		};

		api.get_anchor_by_id(&at, id)
			.ok()
			.unwrap()
			.ok_or(invalid_params_error("Unable to find anchor"))
	}
}
