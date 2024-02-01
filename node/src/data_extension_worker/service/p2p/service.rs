use std::{future, future::Future, pin::Pin, str::FromStr, sync::Arc, thread, time::Duration};

use cumulus_primitives_core::BlockT;
use futures::StreamExt;
use sc_network::{
	config::{ExHashT, IncomingRequest, OutgoingResponse},
	IfDisconnected, NetworkRequest, NetworkService, NetworkStateInfo, PeerId,
};
use sc_telemetry::serde_json;

use crate::data_extension_worker::{
	config::PROTOCOL_NAME,
	service::Service,
	types::{
		BaseError, DataExtensionWorkerMessageSender, DataExtensionWorkerP2PMessage,
		Document as DocumentT,
	},
};

#[derive(Debug, thiserror::Error)]
pub enum P2PError {
	#[error("Document notification error: {0}")]
	DocumentNotificationError(BaseError),

	#[error("Notification send error: {0}")]
	NotificationSendError(BaseError),
}

pub struct P2PService<B: BlockT + 'static, H: ExHashT, Document: DocumentT> {
	network_service: Arc<NetworkService<B, H>>,
	p2p_request_receiver: async_channel::Receiver<IncomingRequest>,
	message_sender: DataExtensionWorkerMessageSender<Document>,
}

impl<B: BlockT + 'static, H: ExHashT, Document: DocumentT> P2PService<B, H, Document> {
	pub fn new(
		network_service: Arc<NetworkService<B, H>>,
		p2p_request_receiver: async_channel::Receiver<IncomingRequest>,
		message_sender: DataExtensionWorkerMessageSender<Document>,
	) -> Self {
		Self {
			network_service,
			p2p_request_receiver,
			message_sender,
		}
	}
}

impl<B: BlockT + 'static, H: ExHashT, Document: DocumentT> Service for P2PService<B, H, Document> {
	fn get_runner(&self) -> Result<Pin<Box<dyn Future<Output = ()> + Send>>, BaseError> {
		let network_service = self.network_service.clone();

		log::info!(target: "data-extension-worker-p2p", "Running Data Extension Worker P2P service");

		let mut handles = Vec::new();

		let peers: Vec<PeerId> = vec![
			PeerId::from_str("12D3KooWDNuHdrRQjd6P2azZxNXqm7YC13UndSz5xvZxBoVNREK1")
				.expect("can parse peer ID"),
			PeerId::from_str("12D3KooWHxgLKo1DeNbTpWDZ3qHEQ1KD36na2NqhYBdQYr9bAsCi")
				.expect("can parse peer ID"),
		];

		for peer_id in peers {
			if network_service.local_peer_id() == peer_id {
				continue;
			}

			log::info!(target: "data-extension-worker-p2p", "Sending initial PING to - {}", peer_id);

			let ping_ns = network_service.clone();

			let handle = tokio::spawn(async move {
				let req = serde_json::to_string(&DataExtensionWorkerP2PMessage::Ping)
					.expect("can serialize ping");

				loop {
					match ping_ns
						.request(
							peer_id,
							PROTOCOL_NAME.into(),
							req.clone().into(),
							IfDisconnected::TryConnect,
						)
						.await
					{
						Ok(r) => {
							log::info!(target: "data-extension-worker-p2p", "Sent PING  to - {}", peer_id);

							let p2p_msg = serde_json::from_str(
								String::from_utf8(r)
									.expect("can decode msg payload")
									.as_str(),
							)
							.expect("Can deserialize p2p message");

							match p2p_msg {
								DataExtensionWorkerP2PMessage::Pong => {
									log::info!(target: "data-extension-worker-p2p", "Received PONG from - {}", peer_id);
								}
								_ => {
									log::error!(target: "data-extension-worker-p2p", "Expected PONG from - {}", peer_id);

									return;
								}
							}
						}
						Err(e) => {
							log::info!(target: "data-extension-worker-p2p", "Error while sending PING to - {};\nError: {}", peer_id, e);
						}
					}

					thread::sleep(Duration::from_secs(1));
				}
			});

			handles.push(handle);
		}

		let pong_fn = self.p2p_request_receiver.clone().for_each(|req| {
			let p2p_msg = serde_json::from_str(
				String::from_utf8(req.payload)
					.expect("can decode msg payload")
					.as_str(),
			)
			.expect("Can deserialize p2p message");

			let mut res_msg: Option<Vec<u8>> = None;

			match p2p_msg {
				DataExtensionWorkerP2PMessage::Ping => {
					log::info!(target: "data-extension-worker-p2p", "Got PING from - {}, sending PONG.", req.peer);

					res_msg = Some(
						serde_json::to_string(&DataExtensionWorkerP2PMessage::Pong)
							.expect("can serialize pong")
							.into(),
					);
				}
				_ => {
					log::error!(target: "data-extension-worker-p2p", "Expected PING from - {}", req.peer);
				}
			}

			if let Some(m) = res_msg {
				let res = OutgoingResponse {
					result: Ok(m),
					reputation_changes: vec![],
					//TODO(cdamian) use this?
					sent_feedback: None,
				};

				match req.pending_response.send(res) {
					Ok(_) => {
						log::info!(target: "data-extension-worker-p2p", "Response sent to {}.", req.peer);
					}
					Err(_) => {
						log::info!(target: "data-extension-worker-p2p", "Error sending response sent to {}.", req.peer);
					}
				}
			}

			future::ready(())
		});

		handles.push(tokio::spawn(pong_fn));

		Ok(Box::pin(async move {
			for handle in handles {
				let _ = handle.await;
			}
		}))
	}
}
