pub(crate) mod config;
pub(crate) mod document;
pub(crate) mod service;
pub(crate) mod worker;

pub(crate) type BaseError = Box<dyn std::error::Error + Send + Sync + 'static>;
