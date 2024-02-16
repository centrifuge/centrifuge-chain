use std::{future::Future, pin::Pin};

use crate::types::BaseError;

mod p2p;
mod storage;

pub use p2p::*;
pub use storage::*;

pub trait Service: Send + Sync + 'static {
	fn get_runner(&self) -> Result<Pin<Box<dyn Future<Output = ()> + Send>>, BaseError>;
}
