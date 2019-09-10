use std::sync::Arc;

use jsonrpc_core::{Result, Error, ErrorCode};
use jsonrpc_derive::rpc;
use substrate_client::{Client, CallExecutor, backend};
use sr_primitives::{traits::Block as BlockT, traits::ProvideRuntimeApi, generic::BlockId};
use primitives::Blake2Hasher;
use centrifuge_chain_runtime::{Hash, AnchorApi, anchor::AnchorData, BlockNumber};

const RUNTIME_ERROR: i64 = 1;

/// Anchor RPC methods.
#[rpc]
pub trait AnchorRpcApi {
    /// Returns an anchor given an anchor id from the runtime storage
    #[rpc(name = "anchor_getAnchorById")]
    fn get_anchor_by_id(&self, id: Hash) -> Result<AnchorData<Hash, BlockNumber>>;
}

/// Anchors api with support for querying anchor child storage
pub struct Anchors<B, E, Block: BlockT, RA> {
    client: Arc<Client<B, E, Block, RA>>,
}

impl<B, E, Block:BlockT, RA> Anchors<B, E, Block, RA> {

    pub fn new(client: Arc<Client<B, E, Block, RA>>) -> Self {
        Anchors {
            client,
        }
    }
}

impl<B, E, Block, RA> AnchorRpcApi for Anchors<B, E, Block, RA>
    where
        Block: BlockT<Hash=Hash> + 'static,
        B: backend::Backend<Block, Blake2Hasher> + Send + Sync + 'static,
        E: CallExecutor<Block, Blake2Hasher> + Send + Sync + 'static + Clone,
        RA: Send + Sync + 'static,
        Client<B, E, Block, RA>: ProvideRuntimeApi,
        <Client<B, E, Block, RA> as ProvideRuntimeApi>::Api: AnchorApi<Block>
{
    fn get_anchor_by_id(&self, id: Hash) -> Result<AnchorData<Hash, BlockNumber>> {
        let api = self.client.runtime_api();
        let best = self.client.info().chain.best_hash;
        let at = BlockId::hash(best);
        api.get_anchor_by_id(&at, id).ok().unwrap().ok_or(Error {
            code: ErrorCode::ServerError(RUNTIME_ERROR),
            message: "Unable to find anchor".into(),
            data: Some(format!("{:?}", id).into()),
        })
    }
}
