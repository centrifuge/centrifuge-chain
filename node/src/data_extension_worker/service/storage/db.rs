use std::{marker::PhantomData, path::PathBuf};

use crate::data_extension_worker::{
	service::DocumentStorage,
	types::{BaseError, Document as DocumentT},
};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
	#[error("Document create error: {0}")]
	DocumentCreateError(BaseError),
}

pub struct DBDocumentStorage<Document> {
	_marker: PhantomData<Document>,
}

impl<Document> DBDocumentStorage<Document>
where
	Document: DocumentT,
{
	pub fn new(_storage_path: PathBuf) -> Self {
		Self {
			_marker: Default::default(),
		}
	}
}

impl<Document> DocumentStorage<Document> for DBDocumentStorage<Document>
where
	Document: DocumentT,
{
	fn store_document(&self, _document: Document) -> Result<(), BaseError> {
		todo!()
	}

	fn get_document_latest(&self, _document_id: Document::Id) -> Result<Document, BaseError> {
		todo!()
	}

	fn get_document_version(
		&self,
		_document_id: Document::Id,
		_version: Document::Version,
	) -> Result<Document, BaseError> {
		todo!()
	}
}