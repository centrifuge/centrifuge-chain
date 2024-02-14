use std::{
	future::Future,
	marker::PhantomData,
	pin::Pin,
	task::{Context, Poll},
};

use async_channel::TryRecvError;
use cumulus_primitives_core::BlockT;
use sc_network::config::ExHashT;

use crate::{
	config::DataExtensionWorkerConfiguration,
	service::{DBDocumentStorage, DocumentStorage, P2PService},
	types::{
		BaseError, Batch as BatchT, DataExtensionWorkerMessage, DataExtensionWorkerMessageReceiver,
		Document as DocumentT, PoolInfo as PoolInfoT,
	},
};

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
	#[error("Services build error: {0}")]
	ServicesBuildError(BaseError),

	#[error("Service start error: {0}")]
	ServicesStartError(BaseError),

	#[error("Response sending error")]
	ResponseSendingError,

	#[error("Document creation error")]
	DocumentCreationError,

	#[error("Document notification error")]
	DocumentNotificationError,

	#[error("Document retrieval error")]
	DocumentRetrievalError,
}

pub struct DataExtensionWorker<Document, Batch, PoolInfo, B, H>
where
	Document: DocumentT,
	Batch: BatchT,
	PoolInfo: PoolInfoT,
	B: BlockT + 'static,
	H: ExHashT,
{
	message_receiver: DataExtensionWorkerMessageReceiver<Document>,
	document_storage: Box<dyn DocumentStorage<Document>>,
	p2p_service: P2PService<B, H, Document>,
	_marker: PhantomData<(Document, Batch, PoolInfo, B, H)>,
}

impl<Document, Batch, PoolInfo, B, H> DataExtensionWorker<Document, Batch, PoolInfo, B, H>
where
	Document: DocumentT,
	Batch: BatchT,
	PoolInfo: PoolInfoT,
	B: BlockT + 'static,
	H: ExHashT,
{
	pub fn new(
		config: DataExtensionWorkerConfiguration,
		message_receiver: DataExtensionWorkerMessageReceiver<Document>,
		p2p_service: P2PService<B, H, Document>,
	) -> Result<Self, WorkerError> {
		let storage = Box::new(DBDocumentStorage::<Document>::new(
			config
				.data_extension_worker_db_path
				.clone()
				.expect("RocksDB path should have default"),
		));

		Ok(Self {
			message_receiver,
			document_storage: storage,
			p2p_service,
			_marker: Default::default(),
		})
	}

	fn handle_msg(&self, msg: DataExtensionWorkerMessage<Document>) -> Result<(), BaseError> {
		log::info!(
			target: "data-extension-worker",
			"Processing Data Extension Worker message",
		);

		match msg {
			DataExtensionWorkerMessage::CreateDocument {
				document,
				res_channel,
			} => {
				match self.document_storage.store_document(document.clone()) {
					Err(e) => {
						res_channel
							.send(Err(WorkerError::DocumentCreationError.into()))
							.map_err(|_| WorkerError::ResponseSendingError)?;

						return Err(e);
					}
					Ok(_) => {}
				};

				res_channel
					.send(Ok(()))
					.map_err(|_| WorkerError::ResponseSendingError)?;

				Ok(())
			}
			DataExtensionWorkerMessage::GetDocumentLatest { id, res_channel } => {
				let document = match self.document_storage.get_document_latest(id) {
					Err(e) => {
						res_channel
							.send(Err(WorkerError::DocumentRetrievalError.into()))
							.map_err(|_| WorkerError::ResponseSendingError)?;

						return Err(e);
					}
					Ok(document) => document,
				};

				res_channel
					.send(Ok(document))
					.map_err(|_| WorkerError::ResponseSendingError)?;

				Ok(())
			}
			DataExtensionWorkerMessage::GetDocumentVersion {
				id,
				version,
				res_channel,
			} => {
				let document = match self.document_storage.get_document_version(id, version) {
					Err(e) => {
						res_channel
							.send(Err(WorkerError::DocumentRetrievalError.into()))
							.map_err(|_| WorkerError::ResponseSendingError)?;

						return Err(e);
					}
					Ok(document) => document,
				};

				res_channel
					.send(Ok(document))
					.map_err(|_| WorkerError::ResponseSendingError)?;

				Ok(())
			}
		}
	}
}

impl<Document, Batch, PoolInfo, B, H> Future
	for DataExtensionWorker<Document, Batch, PoolInfo, B, H>
where
	Document: DocumentT,
	Batch: BatchT,
	PoolInfo: PoolInfoT,
	B: BlockT + 'static,
	H: ExHashT,
{
	type Output = ();

	fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
		match self.message_receiver.try_recv() {
			Ok(msg) => match self.handle_msg(msg) {
				Err(e) => {
					log::error!(
						target: "data-extension-worker",
						"Message handling error - {}",
						e,
					);
				}
				Ok(_) => {}
			},
			Err(e) => match e {
				TryRecvError::Empty => {}
				TryRecvError::Closed => return Poll::Ready(()),
			},
		}

		Poll::Pending
	}
}
