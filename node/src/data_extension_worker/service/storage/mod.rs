use crate::data_extension_worker::types::{BaseError, Document as DocumentT};

mod db;

pub use db::*;

pub trait DocumentStorage<Document>: Send + Sync + 'static
where
	Document: for<'d> DocumentT<'d>,
{
	fn create_document(&self, document: Document) -> Result<Document, BaseError>;

	fn get_document_latest(
		&self,
		document_id: <Document as DocumentT<'_>>::Id,
	) -> Result<Document, BaseError>;

	fn get_document_version(
		&self,
		document_id: <Document as DocumentT<'_>>::Id,
		version: <Document as DocumentT<'_>>::Version,
	) -> Result<Document, BaseError>;
}
