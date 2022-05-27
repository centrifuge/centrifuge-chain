use jsonrpc_core::Result;
// use jsonrpc_derive::rpc;
use jsonrpsee::{
	core::{async_trait, Error as JsonRpseeError, RpcResult},
	proc_macros::rpc,
	types::error::{CallError, ErrorCode, ErrorObject},
};
use node_primitives::{BlockNumber, Hash};
use pallet_anchors::AnchorData;
pub use runtime_common::AnchorApi as AnchorRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc(client, server)]
pub trait AnchorApi {
	/// Returns an anchor given an anchor id from the runtime storage
	#[method(name = "payment_queryInfo")]
	fn get_anchor_by_id(&self, id: Hash) -> Result<AnchorData<Hash, BlockNumber>>;
}

/// A struct that implements the [`AnchorApi`].
pub struct Anchor<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> Anchor<C, P> {
	/// Create new `Anchor` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Anchor {
			client,
			_marker: Default::default(),
		}
	}
}

impl<C, Block> AnchorApi for Anchor<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: AnchorRuntimeApi<Block>,
{
	fn get_anchor_by_id(&self, id: Hash) -> RpcResult<AnchorData<Hash, BlockNumber>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);
		api.get_anchor_by_id(&at, id)
			.ok()
			.unwrap()
			.ok_or(jsonrpsee::Call(CallError::Custom(ErrorObject::owned(
				ErrorCode::InternalError.into(),
				"Unable to find anchor".into(),
				Some(format!("{:?}", id).into()),
			))))
	}
}