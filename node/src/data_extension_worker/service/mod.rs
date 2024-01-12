use std::{future::Future, pin::Pin, sync::Arc};

use sc_network::{config::ExHashT, NetworkService};
use sp_runtime::traits::Block as BlockT;

use crate::data_extension_worker::{
	config::DataExtensionWorkerConfiguration,
	service::{
		p2p::P2PService,
		rpc::{build_rpc_api, RPCService},
		storage::DBDocumentStorage,
	},
	types::{BaseError, Batch as BatchT, Document as DocumentT, PoolInfo as PoolInfoT},
};

mod p2p;
mod rpc;
mod storage;

pub use p2p::*;
pub use rpc::*;
pub use storage::*;

pub trait Service: Send + Sync + 'static {
	fn get_runner(&self) -> Result<Pin<Box<dyn Future<Output = ()> + Send>>, BaseError>;
}

pub fn build_default_services<Document, Batch, PoolInfo, B, H>(
	config: DataExtensionWorkerConfiguration,
	network_service: Arc<NetworkService<B, H>>,
) -> Result<Vec<Arc<dyn Service>>, BaseError>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	PoolInfo: for<'p> PoolInfoT<'p>,
	B: BlockT + 'static,
	H: ExHashT,
{
	let storage = Arc::new(DBDocumentStorage::<Document>::new(
		config
			.rocks_db_path
			.clone()
			.expect("RocksDB path should have default"),
	));

	let p2p_service = Arc::new(P2PService::<B, H>::new(network_service));

	let rpc_api = build_rpc_api::<_, Batch, PoolInfo, _, _>(storage, p2p_service.clone())?;

	let rpc_service = Arc::new(RPCService::new(config, rpc_api));

	Ok(vec![p2p_service, rpc_service])
}
