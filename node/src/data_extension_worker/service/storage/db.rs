use std::{marker::PhantomData, path::PathBuf};

use crate::data_extension_worker::{
	service::Storage,
	types::{BaseError, Document as DocumentT},
};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
	#[error("Document create error: {0}")]
	DocumentCreateError(BaseError),
}

pub struct DBStorage<Document> {
	_marker: PhantomData<Document>,
}

impl<Document> DBStorage<Document>
where
	Document: for<'d> DocumentT<'d>,
{
	pub fn new(_storage_path: PathBuf) -> Self {
		Self {
			_marker: Default::default(),
		}
	}
}

impl<Document> Storage<Document> for DBStorage<Document>
where
	Document: for<'d> DocumentT<'d>,
{
	fn create_document(&self, _document: Document) -> Result<Document, BaseError> {
		todo!()
	}

	fn get_document_latest(
		&self,
		_document_id: <Document as DocumentT<'_>>::Id,
	) -> Result<Document, BaseError> {
		todo!()
	}

	fn get_document_version(
		&self,
		_document_id: <Document as DocumentT<'_>>::Id,
		_version: <Document as DocumentT<'_>>::Version,
	) -> Result<Document, BaseError> {
		todo!()
	}
}
