use crate::data_extension_worker::types::{BaseError, Document as DocumentT};

mod db;

pub use db::*;

pub trait DocumentStorage<Document>: Send + Sync + 'static
where
	Document: DocumentT,
{
	/// Stores the document.
	fn store_document(&self, document: Document) -> Result<(), BaseError>;

	/// Retrieves the latest version of the document.
	fn get_document_latest(&self, document_id: Document::Id) -> Result<Document, BaseError>;

	/// Retrieves a specific version of the document.
	fn get_document_version(
		&self,
		document_id: Document::Id,
		version: Document::Version,
	) -> Result<Document, BaseError>;
}
