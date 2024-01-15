use crate::data_extension_worker::types::{BaseError, Document as DocumentT};

mod service;

pub use service::*;

pub trait DocumentNotifier<Document>: Send + Sync + 'static
where
	Document: DocumentT,
{
	/// Send a notification to all the users of a document to inform them of the
	/// document creation.
	fn send_new_document_notification(&self, document: Document) -> Result<(), BaseError>;
}
