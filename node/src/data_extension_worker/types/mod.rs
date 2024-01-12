mod batch;
mod document;
mod pool;

pub use batch::*;
pub use document::*;
pub use pool::*;

pub(crate) type BaseError = Box<dyn std::error::Error + Send + Sync + 'static>;
