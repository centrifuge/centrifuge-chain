use std::{future, future::Future, pin::Pin, sync::Arc};

use futures::StreamExt;
use sc_network::{config::ExHashT, Event, NetworkEventStream, NetworkService};
use sp_runtime::traits::Block as BlockT;

use crate::data_extension_worker::{document::Document as DocumentT, service::Service, BaseError};

#[derive(Debug, thiserror::Error)]
pub enum P2PError {
	#[error("Document notification error: {0}")]
	DocumentNotificationError(BaseError),
}

pub trait DocumentNotifier<Document>: Send + Sync + 'static
where
	Document: for<'d> DocumentT<'d>,
{
	fn send_document_notification(&self, document: Document) -> Result<(), P2PError>;
}

pub struct P2PService<B: BlockT + 'static, H: ExHashT> {
	network_service: Arc<NetworkService<B, H>>,
}

impl<B: BlockT + 'static, H: ExHashT> P2PService<B, H> {
	pub fn new(network_service: Arc<NetworkService<B, H>>) -> Self {
		Self { network_service }
	}
}

const DATA_EXTENSION_WORKER_EVENT_STREAM_NAME: &'static str = "data-extension-worker";

impl<B: BlockT + 'static, H: ExHashT> Service for P2PService<B, H> {
	fn get_runner(&self) -> Result<Pin<Box<dyn Future<Output = ()> + Send>>, BaseError> {
		let network_service = self.network_service.clone();

		let event_stream = network_service.event_stream(DATA_EXTENSION_WORKER_EVENT_STREAM_NAME);

		log::info!("Running Data Extension Worker P2P service");

		Ok(Box::pin(event_stream.for_each(|event| {
			match event {
				Event::NotificationsReceived { remote, messages } => {
					let (_r, _m) = (remote, messages);
				}
				_ => {}
			}

			future::ready(())
		})))
	}
}

impl<Document, B, H> DocumentNotifier<Document> for P2PService<B, H>
where
	Document: for<'d> DocumentT<'d>,
	B: BlockT + 'static,
	H: ExHashT,
{
	fn send_document_notification(&self, _document: Document) -> Result<(), P2PError> {
		todo!()
	}
}
