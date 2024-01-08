use std::{marker::PhantomData, sync::Arc};

use cfg_primitives::PoolId;
use jsonrpsee::{
	core::{RpcResult, __reexports::serde_json},
	proc_macros::rpc,
	types::{error::CallError, ErrorObject},
	RpcModule,
};

use crate::data_extension_worker::{
	document::{Batch as BatchT, Document as DocumentT},
	service::{p2p::DocumentNotifier, rpc::RPCError, storage::Storage as StorageT},
	BaseError,
};

/// The RPC API exposed by the DataExtensionWorker.
#[rpc(client, server)]
pub trait DataExtensionWorkerApi<Document, Batch>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
{
	/// Creates a document.
	#[method(name = "dataExtensionWorker_createDocument")]
	fn create_document(&self, document: Document) -> RpcResult<Document>;

	/// Retrieves a document.
	#[method(name = "dataExtensionWorker_getDocument")]
	fn get_document(&self, document_id: <Document as DocumentT<'_>>::Id) -> RpcResult<Document>;

	/// Processes a batch of items.
	#[method(name = "dataExtensionWorker_processBatch")]
	fn process_batch(&self, batch: Batch) -> RpcResult<()>;

	/// Updates the information that is stored for a pool.
	#[method(name = "dataExtensionWorker_updatePoolInfo")]
	fn update_pool_info(&self, pool_id: <Document as DocumentT<'_>>::PoolId) -> RpcResult<()>;
}

pub struct Api<Document, Batch, Notifier, Storage> {
	storage: Arc<Storage>,
	document_notifier: Arc<Notifier>,
	_marker: PhantomData<(Document, Batch)>,
}

impl<Document, Batch, Notifier, Storage> Api<Document, Batch, Notifier, Storage>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	Notifier: DocumentNotifier<Document>,
	Storage: StorageT<Document>,
{
	pub fn new(storage: Arc<Storage>, document_notifier: Arc<Notifier>) -> Self {
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

impl<Document, Batch, Notifier, Storage> DataExtensionWorkerApiServer<Document, Batch>
	for Api<Document, Batch, Notifier, Storage>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	Notifier: DocumentNotifier<Document>,
	Storage: StorageT<Document>,
{
	fn create_document(&self, document: Document) -> RpcResult<Document> {
		self.storage.create_document(document).map_err(|e| {
			CallError::Custom(ErrorObject::owned(
				DOCUMENT_CREATION_ERROR,
				format!("Document creation error: {}", e),
				Some(format!("{:?}", e)),
			))
			.into()
		})
	}

	fn get_document(&self, document_id: <Document as DocumentT<'_>>::Id) -> RpcResult<Document> {
		self.storage.get_document(document_id).map_err(|e| {
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

	fn update_pool_info(&self, _pool_id: <Document as DocumentT<'_>>::PoolId) -> RpcResult<()> {
		todo!()
	}
}

pub fn build_rpc_api<Document, Batch, Storage, Notifier>(
	storage: Arc<Storage>,
	document_notifier: Arc<Notifier>,
) -> Result<RpcModule<()>, BaseError>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	Storage: StorageT<Document>,
	Notifier: DocumentNotifier<Document>,
{
	let mut rpc_api = RpcModule::new(());

	let data_extension_worker_api =
		Api::<_, Batch, _, _>::new(storage, document_notifier).into_rpc();

	rpc_api.merge(data_extension_worker_api)?;

	let mut available_methods = rpc_api.method_names().collect::<Vec<_>>();
	available_methods.sort();

	rpc_api
		.register_method("rpc_methods", move |_, _| {
			Ok(serde_json::json!({
				"methods": available_methods,
			}))
		})
		.map_err(|e| RPCError::RPCStartError(e.into()))?;

	Ok(rpc_api)
}
