use std::{
	future::Future,
	marker::PhantomData,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use cumulus_primitives_core::BlockT;
use sc_network::{config::ExHashT, NetworkService};
use tokio::task::JoinHandle;

use crate::data_extension_worker::{
	config::DataExtensionWorkerConfiguration,
	service::build_default_services,
	types::{BaseError, Batch as BatchT, Document as DocumentT, PoolInfo as PoolInfoT},
};
#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
	#[error("Services build error: {0}")]
	ServicesBuildError(BaseError),

	#[error("Service start error: {0}")]
	ServicesStartError(BaseError),
}

pub struct DataExtensionWorker<Document, Batch, PoolInfo, B, H> {
	handles: Vec<JoinHandle<()>>,
	_marker: PhantomData<(Document, Batch, PoolInfo, B, H)>,
}

impl<Document, Batch, PoolInfo, B, H> DataExtensionWorker<Document, Batch, PoolInfo, B, H>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	PoolInfo: for<'p> PoolInfoT<'p>,
	B: BlockT + 'static,
	H: ExHashT,
{
	pub fn new(
		config: DataExtensionWorkerConfiguration,
		network_service: Arc<NetworkService<B, H>>,
	) -> Result<Self, WorkerError> {
		let mut services =
			build_default_services::<Document, Batch, PoolInfo, B, H>(config, network_service)
				.map_err(WorkerError::ServicesBuildError)?;

		let mut handles = Vec::new();

		for service in services.iter_mut() {
			let fut = service
				.get_runner()
				.map_err(WorkerError::ServicesStartError)?;

			handles.push(tokio::spawn(fut));
		}

		Ok(Self {
			handles,
			_marker: Default::default(),
		})
	}
}

impl<Document, Batch, PoolInfo, B, H> Future
	for DataExtensionWorker<Document, Batch, PoolInfo, B, H>
where
	Document: for<'d> DocumentT<'d>,
	Batch: for<'b> BatchT<'b>,
	PoolInfo: for<'p> PoolInfoT<'p>,
	B: BlockT + 'static,
	H: ExHashT,
{
	type Output = ();

	fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
		for handle in &self.handles {
			if handle.is_finished() {
				return Poll::Ready(());
			}
		}

		Poll::Pending
	}
}
