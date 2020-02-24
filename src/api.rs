use jsonrpc_core::Result as RpcResult;
use jsonrpc_derive::rpc;
use node_primitives::{BlockNumber, Hash};
use node_runtime::{anchor::AnchorData, AnchorApi};
use sc_client::{CallExecutor, Client};
use sc_client_api::backend;
use sp_core::Blake2Hasher;
use sp_runtime::{generic::BlockId, traits::Block as BlockT, traits::ProvideRuntimeApi};
use std::sync::Arc;

/// Anchor RPC methods.
#[rpc]
pub trait AnchorRpcApi {
    /// Returns an anchor given an anchor id from the runtime storage
    #[rpc(name = "anchor_getAnchorById")]
    fn get_anchor_by_id(&self, id: Hash) -> RpcResult<AnchorData<Hash, BlockNumber>>;
}

/// Anchors api with support for querying anchor child storage
pub struct Anchors<B, E, Block: BlockT, RA> {
    client: Arc<Client<B, E, Block, RA>>,
}

impl<B, E, Block: BlockT, RA> Anchors<B, E, Block, RA> {
    pub fn new(client: Arc<Client<B, E, Block, RA>>) -> Self {
        Anchors { client }
    }
}

impl<B, E, Block, RA> AnchorRpcApi for Anchors<B, E, Block, RA>
where
    Block: BlockT<Hash = Hash> + 'static,
    B: backend::Backend<Block, Blake2Hasher> + Send + Sync + 'static,
    E: CallExecutor<Block, Blake2Hasher> + Send + Sync + 'static + Clone,
    RA: Send + Sync + 'static,
    Client<B, E, Block, RA>: ProvideRuntimeApi,
    <Client<B, E, Block, RA> as ProvideRuntimeApi>::Api: AnchorApi<Block>,
{
    fn get_anchor_by_id(&self, id: Hash) -> RpcResult<AnchorData<Hash, BlockNumber>> {
        let api = self.client.runtime_api();
        let best = self.client.usage_info().chain.best_hash;
        let at = BlockId::hash(best);
        api.get_anchor_by_id(&at, id)
            .ok()
            .unwrap()
            .ok_or(jsonrpc_core::Error {
                code: jsonrpc_core::ErrorCode::InternalError,
                message: "Unable to find anchor".into(),
                data: Some(format!("{:?}", id).into()),
            })
    }
}
