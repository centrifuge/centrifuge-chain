use std::marker::PhantomData;

use crate::data_extension_worker::{document::Document as DocumentT, BaseError};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
	#[error("Document create error: {0}")]
	DocumentCreateError(BaseError),
}

pub trait Storage<Document>: Send + Sync + 'static
where
	Document: for<'d> DocumentT<'d>,
{
	fn create_document(&self, document: Document) -> Result<Document, StorageError>;

	fn get_document(
		&self,
		document_id: <Document as DocumentT<'_>>::Id,
	) -> Result<Document, StorageError>;
}

pub struct LocalStorage<Document> {
	_marker: PhantomData<Document>,
}

impl<Document> LocalStorage<Document>
where
	Document: for<'d> DocumentT<'d>,
{
	pub fn new(_storage_path: String) -> Self {
		Self {
			_marker: Default::default(),
		}
	}
}

impl<Document> Storage<Document> for LocalStorage<Document>
where
	Document: for<'d> DocumentT<'d>,
{
	fn create_document(&self, _document: Document) -> Result<Document, StorageError> {
		todo!()
	}

	fn get_document(
		&self,
		_document_id: <Document as DocumentT<'_>>::Id,
	) -> Result<Document, StorageError> {
		todo!()
	}
}
