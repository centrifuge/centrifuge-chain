use crate::data_extension_worker::types::{BaseError, Document as DocumentT};

mod service;

pub use service::*;

pub trait DocumentNotifier<Document>: Send + Sync + 'static
where
	Document: for<'d> DocumentT<'d>,
{
	fn send_document_notification(&self, document: Document) -> Result<(), BaseError>;
}
