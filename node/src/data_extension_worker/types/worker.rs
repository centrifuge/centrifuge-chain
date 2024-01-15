use futures_channel::oneshot;

use crate::data_extension_worker::types::{BaseError, Document as DocumentT};

pub type DataExtensionWorkerMessageSender<Document> =
	async_channel::Sender<DataExtensionWorkerMessage<Document>>;

pub type DataExtensionWorkerMessageReceiver<Document> =
	async_channel::Receiver<DataExtensionWorkerMessage<Document>>;

pub enum DataExtensionWorkerMessage<Document: DocumentT> {
	CreateDocument {
		document: Document,
		res_channel: oneshot::Sender<Result<(), BaseError>>,
	},
	GetDocumentLatest {
		id: Document::Id,
		res_channel: oneshot::Sender<Result<Document, BaseError>>,
	},
	GetDocumentVersion {
		id: Document::Id,
		version: Document::Version,
		res_channel: oneshot::Sender<Result<Document, BaseError>>,
	},
}
