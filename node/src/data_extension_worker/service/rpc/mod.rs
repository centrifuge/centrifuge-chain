use std::{
	future::Future, marker::PhantomData, net::SocketAddr, pin::Pin, str::FromStr, sync::Arc,
};

use jsonrpsee::{
	core::__reexports::serde_json,
	server::{RandomIntegerIdProvider, ServerBuilder},
	RpcModule,
};
use sc_service::SpawnTaskHandle;

use crate::data_extension_worker::{
	config::DataExtensionWorkerConfiguration,
	document::{DataExtensionWorkerBatch, Document as DocumentT},
	service::{
		p2p::DocumentNotifier,
		rpc::api::{Api, DataExtensionWorkerApiServer},
		storage::Storage as StorageT,
		Service,
	},
	BaseError,
};

pub(crate) mod api;

const MAX_REQUEST_BODY_SIZE: u32 = 10 * 1024 * 1024;
const MAX_RESPONSE_BODY_SIZE: u32 = 10 * 1024 * 1024;
const MAX_CONNECTIONS: u32 = 10;
const MAX_SUBSCRIPTIONS_PER_CONNECTIONS: u32 = 100;

#[derive(Debug, thiserror::Error)]
pub enum RPCError {
	#[error("RPC API not built")]
	RPCAPINotBuilt,

	#[error("RPC Start error: {0}")]
	RPCStartError(BaseError),
}

pub struct RPCService {
	config: DataExtensionWorkerConfiguration,
	rpc_api: RpcModule<()>,
}

impl RPCService {
	pub fn new(config: DataExtensionWorkerConfiguration, rpc_api: RpcModule<()>) -> Self {
		Self { config, rpc_api }
	}
}

impl Service for RPCService {
	fn get_runner(&self) -> Result<Pin<Box<dyn Future<Output = ()> + Send>>, BaseError> {
		let builder = ServerBuilder::new()
			// .set_host_filtering()
			// .set_middleware()
			// .custom_tokio_runtime()
			.max_request_body_size(MAX_REQUEST_BODY_SIZE)
			.max_response_body_size(MAX_RESPONSE_BODY_SIZE)
			.max_connections(MAX_CONNECTIONS)
			.max_subscriptions_per_connection(MAX_SUBSCRIPTIONS_PER_CONNECTIONS)
			.ping_interval(std::time::Duration::from_secs(30))
			.set_id_provider(RandomIntegerIdProvider);

		let socket_addr_str = format!(
			"{}:{}",
			self.config
				.data_extension_worker_rpc_addr
				.expect("data extension worker RPC address should have a default"),
			self.config.data_extension_worker_rpc_port
		);

		log::info!(
			"Running Data Extension Worker JSON-RPC server: addr={}",
			socket_addr_str.as_str()
		);

		let rpc_addr = SocketAddr::from_str(socket_addr_str.as_str())
			.map_err(|e| RPCError::RPCStartError(e.into()))?;

		let server = futures::executor::block_on(builder.build(rpc_addr))
			.map_err(|e| RPCError::RPCStartError(e.into()))?;

		let handle = server
			.start(self.rpc_api.clone())
			.map_err(|e| RPCError::RPCStartError(e.into()))?;

		Ok(Box::pin(async move {
			loop {
				if handle.is_stopped() {
					log::info!("Stopping Data Extension Worker JSON-RPC server");

					return;
				}
			}
		}))
	}
}
