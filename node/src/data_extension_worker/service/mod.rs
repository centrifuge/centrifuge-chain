use std::{future::Future, pin::Pin, sync::Arc};

use sc_network::{config::ExHashT, NetworkService};
use sp_runtime::traits::Block as BlockT;

use crate::data_extension_worker::{
	config::DataExtensionWorkerConfiguration,
	document::{Batch as BatchT, Document as DocumentT},
	service::{
		p2p::P2PService,
		rpc::{api::build_rpc_api, RPCService},
		storage::LocalStorage,
	},
	BaseError,
};

pub(crate) mod p2p;
pub(crate) mod rpc;
pub(crate) mod storage;

pub trait Service: Send + Sync + 'static {
	fn get_runner(&self) -> Result<Pin<Box<dyn Future<Output = ()> + Send>>, BaseError>;
}

pub fn build_default_services<Document, Batch, B, H>(
	config: DataExtensionWorkerConfiguration,
	network_service: Arc<NetworkService<B, H>>,
) -> Result<Vec<Arc<dyn Service>>, BaseError>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	B: BlockT + 'static,
	H: ExHashT,
{
	let storage = Arc::new(LocalStorage::<Document>::new(String::new()));

	let p2p_service = Arc::new(P2PService::<B, H>::new(network_service));

	let rpc_api = build_rpc_api::<_, Batch, _, _>(storage, p2p_service.clone())?;

	let rpc_service = Arc::new(RPCService::new(config, rpc_api));

	Ok(vec![p2p_service, rpc_service])
}
