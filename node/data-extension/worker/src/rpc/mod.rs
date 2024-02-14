use std::marker::PhantomData;

use cfg_primitives::PoolId;
use futures_channel::oneshot;
use jsonrpsee::{
	core::RpcResult,
	proc_macros::rpc,
	types::{error::CallError, ErrorObject},
};

use crate::types::{
	Batch as BatchT, DataExtensionWorkerMessage, DataExtensionWorkerMessageSender,
	Document as DocumentT, PoolInfo as PoolInfoT,
};

/// The RPC API exposed by the DataExtensionWorker.
#[rpc(client, server)]
pub trait DataExtensionWorkerApi<Document, Batch, PoolInfo>
where
	Document: DocumentT,
	Batch: BatchT,
	PoolInfo: PoolInfoT,
{
	/// Creates a document.
	#[method(name = "dataExtensionWorker_createDocument")]
	fn create_document(&self, document: Document) -> RpcResult<()>;

	/// Retrieves the latest version of a document.
	#[method(name = "dataExtensionWorker_getDocumentLatest")]
	fn get_document_latest(&self, document_id: Document::Id) -> RpcResult<Document>;

	/// Retrieves a specific version of a document.
	#[method(name = "dataExtensionWorker_getDocumentVersion")]
	fn get_document_version(
		&self,
		document_id: Document::Id,
		version: Document::Version,
	) -> RpcResult<Document>;

	/// Processes a batch of items.
	#[method(name = "dataExtensionWorker_processBatch")]
	fn process_batch(&self, batch: Batch) -> RpcResult<()>;

	/// Updates the information that is stored for a pool.
	#[method(name = "dataExtensionWorker_updatePoolInfo")]
	fn update_pool_info(&self, pool_id: PoolId) -> RpcResult<PoolInfo>;
}

pub struct Api<Document, Batch, PoolInfo>
where
	Document: DocumentT,
{
	message_sender: DataExtensionWorkerMessageSender<Document>,
	_marker: PhantomData<(Document, Batch, PoolInfo)>,
}

impl<'d, Document, Batch, PoolInfo> Api<Document, Batch, PoolInfo>
where
	Document: DocumentT,
	Batch: BatchT,
	PoolInfo: PoolInfoT,
{
	pub fn new(message_sender: DataExtensionWorkerMessageSender<Document>) -> Self {
		Self {
			message_sender,
			_marker: Default::default(),
		}
	}
}

const BASE_ERROR: i32 = 100;

const WORKER_MESSAGE_SEND_ERROR: i32 = BASE_ERROR + 1;
const FUTURES_EXECUTOR_ERROR: i32 = BASE_ERROR + 2;
const DOCUMENT_CREATION_ERROR: i32 = BASE_ERROR + 3;
const DOCUMENT_RETRIEVAL_ERROR: i32 = BASE_ERROR + 4;

impl<Document, Batch, PoolInfo> DataExtensionWorkerApiServer<Document, Batch, PoolInfo>
	for Api<Document, Batch, PoolInfo>
where
	Document: DocumentT,
	Batch: BatchT,
	PoolInfo: PoolInfoT,
{
	fn create_document(&self, document: Document) -> RpcResult<()> {
		let (tx, rx) = oneshot::channel();

		self.message_sender
			.try_send(DataExtensionWorkerMessage::CreateDocument {
				document,
				res_channel: tx,
			})
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					WORKER_MESSAGE_SEND_ERROR,
					format!("Worker message send error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?;

		futures::executor::block_on(rx)
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					FUTURES_EXECUTOR_ERROR,
					format!("Futures executor error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					DOCUMENT_CREATION_ERROR,
					format!("Document creation error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?;

		Ok(())
	}

	fn get_document_latest(&self, document_id: Document::Id) -> RpcResult<Document> {
		let (tx, rx) = oneshot::channel();

		self.message_sender
			.try_send(DataExtensionWorkerMessage::GetDocumentLatest {
				id: document_id,
				res_channel: tx,
			})
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					WORKER_MESSAGE_SEND_ERROR,
					format!("Worker message send error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?;

		let document = futures::executor::block_on(rx)
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					FUTURES_EXECUTOR_ERROR,
					format!("Futures executor error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					DOCUMENT_RETRIEVAL_ERROR,
					format!("Document retrieval error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?;

		Ok(document)
	}

	fn get_document_version(
		&self,
		document_id: Document::Id,
		version: Document::Version,
	) -> RpcResult<Document> {
		let (tx, rx) = oneshot::channel();

		self.message_sender
			.try_send(DataExtensionWorkerMessage::GetDocumentVersion {
				id: document_id,
				version,
				res_channel: tx,
			})
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					WORKER_MESSAGE_SEND_ERROR,
					format!("Worker message send error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?;

		let document = futures::executor::block_on(rx)
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					FUTURES_EXECUTOR_ERROR,
					format!("Futures executor error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					DOCUMENT_RETRIEVAL_ERROR,
					format!("Document retrieval error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?;

		Ok(document)
	}

	fn process_batch(&self, _batch: Batch) -> RpcResult<()> {
		todo!()
	}

	fn update_pool_info(&self, _pool_id: PoolId) -> RpcResult<PoolInfo> {
		todo!()
	}
}
