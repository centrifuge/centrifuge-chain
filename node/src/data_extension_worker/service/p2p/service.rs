use std::{future, future::Future, pin::Pin, str::FromStr, sync::Arc, task::Poll};

use cfg_primitives::Bytes;
use cumulus_primitives_core::BlockT;
use futures::{FutureExt, StreamExt};
use sc_network::{
	config::ExHashT, Event, NetworkEventStream, NetworkNotification, NetworkService, PeerId,
	ProtocolName,
};

use crate::data_extension_worker::{
	service::{DocumentNotifier, Service},
	types::{BaseError, Document as DocumentT},
};

#[derive(Debug, thiserror::Error)]
pub enum P2PError {
	#[error("Document notification error: {0}")]
	DocumentNotificationError(BaseError),

	#[error("Notification send error: {0}")]
	NotificationSendError(BaseError),
}

pub struct P2PService<B: BlockT + 'static, H: ExHashT> {
	network_service: Arc<NetworkService<B, H>>,
}

impl<B: BlockT + 'static, H: ExHashT> P2PService<B, H> {
	pub fn new(network_service: Arc<NetworkService<B, H>>) -> Self {
		Self { network_service }
	}

	async fn send_notification(
		network_service: Arc<NetworkService<B, H>>,
		peer: PeerId,
		message: Vec<u8>,
	) -> Result<(), P2PError> {
		let notification_sender = network_service
			.notification_sender(
				peer,
				ProtocolName::Static(DATA_EXTENSION_WORKER_EVENT_STREAM_NAME),
			)
			.map_err(|e| P2PError::NotificationSendError(BaseError::from(e)))?;

		let mut ready = notification_sender
			.ready()
			.await
			.map_err(|e| P2PError::NotificationSendError(BaseError::from(e)))?;

		ready
			.send(message)
			.map_err(|e| P2PError::NotificationSendError(BaseError::from(e)))
	}
}

const DATA_EXTENSION_WORKER_EVENT_STREAM_NAME: &'static str = "data-extension-worker";

impl<B: BlockT + 'static, H: ExHashT> Service for P2PService<B, H> {
	fn get_runner(&self) -> Result<Pin<Box<dyn Future<Output = ()> + Send>>, BaseError> {
		let network_service = self.network_service.clone();

		let event_stream = network_service.event_stream(DATA_EXTENSION_WORKER_EVENT_STREAM_NAME);

		log::info!(target: "data-extension-worker-p2p", "Running Data Extension Worker P2P service");

		let mut handles = Vec::new();

		let ping_ns = self.network_service.clone();

		let ping_fn = async move {
			loop {
				log::info!(target: "data-extension-worker-p2p", "Getting network state...");

				let network_state = ping_ns
					.network_state()
					.await
					.expect("can retrieve network state");

				log::info!(target: "data-extension-worker-p2p", "Found {} connected peers", network_state.connected_peers.len());

				let peers = network_state
					.connected_peers
					.into_iter()
					.map(|(peer_id, _)| {
						PeerId::from_str(peer_id.as_str()).expect("can parse peer id")
					})
					.collect::<Vec<_>>();

				log::info!(target: "data-extension-worker-p2p", "Parsed {} peers", peers.len());

				for peer in peers {
					log::info!(target: "data-extension-worker-p2p", "Sending ping to - {}", peer);

					match Self::send_notification(ping_ns.clone(), peer, "ping".into()).await {
						Err(e) => {
							log::error!(target: "data-extension-worker-p2p", "Notification sender error - {}", e)
						}
						Ok(_) => {}
					}
				}
			}
		};

		handles.push(tokio::spawn(ping_fn));

		let stream_ns = self.network_service.clone();

		let stream_fn = event_stream.for_each(move |event| {
			match event {
				Event::NotificationsReceived { remote, messages } => {
					let peers = messages
						.iter()
						.filter_map(|res| {
							let ping = Bytes::from("ping");
							let pong = Bytes::from("pong");

							match res {
								(
									ProtocolName::Static(DATA_EXTENSION_WORKER_EVENT_STREAM_NAME),
									ping,
								) => {
									log::info!(target: "data-extension-worker-p2p", "Got ping from - {}", remote);

									Some(remote)
								}
								(
									ProtocolName::Static(DATA_EXTENSION_WORKER_EVENT_STREAM_NAME),
									pong,
								) => {
									log::info!(target: "data-extension-worker-p2p", "Got pong from - {}", remote);

									None
								}
								_ => None,
							}
						})
						.collect::<Vec<_>>();

					for peer in peers {
						match futures::executor::block_on(Self::send_notification(
							stream_ns.clone(),
							peer,
							"pong".into(),
						)) {
							Err(e) => {
								log::error!(target: "data-extension-worker-p2p", "Notification sender error - {}", e)
							}
							Ok(_) => {}
						}
					}
				}
				_ => {}
			}

			future::ready(())
		});

		handles.push(tokio::spawn(stream_fn));

		Ok(Box::pin(async move {
			for handle in handles {
				let _ = handle.await;

				log::error!(target: "data-extension-worker-p2p", "P2PService handle finished");
			}
		}))
	}
}

impl<Document, B, H> DocumentNotifier<Document> for P2PService<B, H>
where
	Document: DocumentT,
	B: BlockT + 'static,
	H: ExHashT,
{
	fn send_new_document_notification(&self, _document: Document) -> Result<(), BaseError> {
		todo!()
	}
}
