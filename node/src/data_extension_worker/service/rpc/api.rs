use std::{marker::PhantomData, sync::Arc};

use cfg_primitives::PoolId;
use jsonrpsee::{
	core::{RpcResult, __reexports::serde_json},
	proc_macros::rpc,
	types::{error::CallError, ErrorObject},
	RpcModule,
};
use serde::{Deserialize, Serialize};

use crate::data_extension_worker::{
	service::{
		p2p::DocumentNotifier as DocumentNotifierT, rpc::RpcServiceError,
		storage::DocumentStorage as DocumentStorageT,
	},
	types::{BaseError, Batch as BatchT, Document as DocumentT, PoolInfo as PoolInfoT},
};

/// The RPC API exposed by the DataExtensionWorker.
#[rpc(client, server)]
pub trait DataExtensionWorkerApi<Document, Batch, PoolInfo>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	PoolInfo: for<'p> PoolInfoT<'p>,
{
	/// Creates a document.
	#[method(name = "dataExtensionWorker_createDocument")]
	fn create_document(&self, document: Document) -> RpcResult<Document>;

	/// Retrieves the latest version of a document.
	#[method(name = "dataExtensionWorker_getDocumentLatest")]
	fn get_document_latest(
		&self,
		document_id: <Document as DocumentT<'_>>::Id,
	) -> RpcResult<Document>;

	/// Retrieves a specific version of a document.
	#[method(name = "dataExtensionWorker_getDocumentVersion")]
	fn get_document_version(
		&self,
		document_id: <Document as DocumentT<'_>>::Id,
		version: <Document as DocumentT<'_>>::Version,
	) -> RpcResult<Document>;

	/// Processes a batch of items.
	#[method(name = "dataExtensionWorker_processBatch")]
	fn process_batch(&self, batch: Batch) -> RpcResult<()>;

	/// Updates the information that is stored for a pool.
	#[method(name = "dataExtensionWorker_updatePoolInfo")]
	fn update_pool_info(&self, pool_id: PoolId) -> RpcResult<PoolInfo>;
}

pub struct Api<Document, Batch, PoolInfo, DocumentStorage, DocumentNotifier> {
	storage: Arc<DocumentStorage>,
	document_notifier: Arc<DocumentNotifier>,
	_marker: PhantomData<(Document, Batch, PoolInfo)>,
}

impl<Document, Batch, PoolInfo, DocumentStorage, DocumentNotifier>
	Api<Document, Batch, PoolInfo, DocumentStorage, DocumentNotifier>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	PoolInfo: for<'p> PoolInfoT<'p>,
	DocumentStorage: DocumentStorageT<Document>,
	DocumentNotifier: DocumentNotifierT<Document>,
{
	pub fn new(storage: Arc<DocumentStorage>, document_notifier: Arc<DocumentNotifier>) -> Self {
		Self {
			storage,
			document_notifier,
			_marker: Default::default(),
		}
	}
}

const BASE_ERROR: i32 = 100;

const DOCUMENT_CREATION_ERROR: i32 = BASE_ERROR + 1;
const DOCUMENT_RETRIEVAL_ERROR: i32 = BASE_ERROR + 2;
const DOCUMENT_NOTIFICATION_ERROR: i32 = BASE_ERROR + 3;

impl<Document, Batch, PoolInfo, DocumentStorage, DocumentNotifier>
	DataExtensionWorkerApiServer<Document, Batch, PoolInfo>
	for Api<Document, Batch, PoolInfo, DocumentStorage, DocumentNotifier>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	PoolInfo: for<'p> PoolInfoT<'p>,
	DocumentStorage: DocumentStorageT<Document>,
	DocumentNotifier: DocumentNotifierT<Document>,
{
	fn create_document(&self, document: Document) -> RpcResult<Document> {
		self.storage.store_document(document.clone()).map_err(|e| {
			CallError::Custom(ErrorObject::owned(
				DOCUMENT_CREATION_ERROR,
				format!("Document creation error: {}", e),
				Some(format!("{:?}", e)),
			))
		})?;

		self.document_notifier
			.send_new_document_notification(document.clone())
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					DOCUMENT_NOTIFICATION_ERROR,
					format!("Document notification error: {}", e),
					Some(format!("{:?}", e)),
				))
			})?;

		Ok(document)
	}

	fn get_document_latest(
		&self,
		document_id: <Document as DocumentT<'_>>::Id,
	) -> RpcResult<Document> {
		self.storage.get_document_latest(document_id).map_err(|e| {
			CallError::Custom(ErrorObject::owned(
				DOCUMENT_RETRIEVAL_ERROR,
				format!("Document retrieval error: {}", e),
				Some(format!("{:?}", e)),
			))
			.into()
		})
	}

	fn get_document_version(
		&self,
		document_id: <Document as DocumentT<'_>>::Id,
		version: <Document as DocumentT<'_>>::Version,
	) -> RpcResult<Document> {
		self.storage
			.get_document_version(document_id, version)
			.map_err(|e| {
				CallError::Custom(ErrorObject::owned(
					DOCUMENT_RETRIEVAL_ERROR,
					format!("Document retrieval error: {}", e),
					Some(format!("{:?}", e)),
				))
				.into()
			})
	}

	fn process_batch(&self, _batch: Batch) -> RpcResult<()> {
		todo!()
	}

	fn update_pool_info(&self, _pool_id: PoolId) -> RpcResult<PoolInfo> {
		todo!()
	}
}

pub fn build_rpc_api<Document, Batch, PoolInfo, DocumentStorage, DocumentNotifier>(
	storage: Arc<DocumentStorage>,
	document_notifier: Arc<DocumentNotifier>,
) -> Result<RpcModule<()>, BaseError>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	PoolInfo: for<'p> PoolInfoT<'p>,
	DocumentStorage: DocumentStorageT<Document>,
	DocumentNotifier: DocumentNotifierT<Document>,
{
	let mut rpc_api = RpcModule::new(());

	let data_extension_worker_api =
		Api::<_, Batch, PoolInfo, _, _>::new(storage, document_notifier).into_rpc();

	rpc_api.merge(data_extension_worker_api)?;

	let mut available_methods = rpc_api.method_names().collect::<Vec<_>>();
	available_methods.sort();

	rpc_api
		.register_method("rpc_methods", move |_, _| {
			Ok(serde_json::json!({
				"methods": available_methods,
			}))
		})
		.map_err(|e| RpcServiceError::RpcStartError(e.into()))?;

	Ok(rpc_api)
}
