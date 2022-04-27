use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use pallet_anchors::AnchorData;
use runtime_common::AnchorApi as AnchorRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait AnchorApi<Hash, BlockNumber> {
	/// Returns an anchor given an anchor id from the runtime storage
	#[rpc(name = "anchor_getAnchorById")]
	fn get_anchor_by_id(&self, id: Hash) -> Result<AnchorData<Hash, BlockNumber>>;
}

pub struct Anchor<C, P> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<P>,
}

impl<C, P> Anchor<C, P> {
	pub fn new(client: Arc<C>) -> Self {
		Anchor {
			client,
			_marker: Default::default(),
		}
	}
}

impl<C, Block, Hash> AnchorApi<Hash, sp_runtime::traits::NumberFor<Block>> for Anchor<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: AnchorRuntimeApi<Block, Hash, sp_runtime::traits::NumberFor<Block>>,
	Hash: Codec + Clone + sp_std::fmt::Debug,
{
	fn get_anchor_by_id(
		&self,
		id: Hash,
	) -> Result<AnchorData<Hash, sp_runtime::traits::NumberFor<Block>>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);
		api.get_anchor_by_id(&at, id.clone())
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(crate::rpc::Error::RuntimeError.into()),
				message: "Unable to query anchor by id.".into(),
				data: Some(format!("{:?}", e).into()),
			})?
			.ok_or(RpcError {
				code: jsonrpc_core::ErrorCode::InternalError,
				message: "Unable to find anchor".into(),
				data: Some(format!("{:?}", id).into()),
			})
	}
}
