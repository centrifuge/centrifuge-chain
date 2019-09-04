use std::sync::Arc;

use sr_primitives::traits;
use jsonrpc_core::{Result, Error, ErrorCode};
use jsonrpc_derive::rpc;

/// Accounts RPC methods.
#[rpc]
pub trait AnchorApi {
    /// Returns the next valid index (aka nonce) for given account.
    #[rpc(name = "anchor_getAnchorById")]
    fn nonce(&self) -> Result<u64>;
}

pub struct Anchors<C> {
    client: Arc<C>,
}

impl<C> Anchors<C> {

    pub fn new(client: Arc<C>) -> Self {
        Anchors {
            client,
        }
    }
}

impl<C> AnchorApi for Anchors<C>
    where
        C: traits::ProvideRuntimeApi,
        C: Send + Sync + 'static,
{
    fn nonce(&self) -> Result<u64> {
        Ok(12)
    }
}
