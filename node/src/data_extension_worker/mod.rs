use std::{
	fmt::Debug,
	future::Future,
	marker::PhantomData,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use sc_network::{config::ExHashT, NetworkService};
use sc_service::TaskManager;
use sp_runtime::traits::Block as BlockT;
use tokio::task::JoinHandle;

use crate::data_extension_worker::{
	config::DataExtensionWorkerConfiguration, document::Document as DocumentT,
	service::build_default_services,
};

pub(crate) mod config;
pub(crate) mod document;
pub(crate) mod service;

pub(crate) type BaseError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
	#[error("Services build error: {0}")]
	ServicesBuildError(BaseError),

	#[error("Service start error: {0}")]
	ServicesStartError(BaseError),
}

pub struct DataExtensionWorker<Document, B, H> {
	handles: Vec<JoinHandle<()>>,
	_marker: PhantomData<(Document, B, H)>,
}

impl<Document, B, H> DataExtensionWorker<Document, B, H>
where
	Document: for<'d> DocumentT<'d>,
	B: BlockT + 'static,
	H: ExHashT,
{
	pub fn new(
		config: DataExtensionWorkerConfiguration,
		network_service: Arc<NetworkService<B, H>>,
	) -> Result<Self, WorkerError> {
		let mut services = build_default_services::<Document, B, H>(config, network_service)
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

impl<Document, B, H> Future for DataExtensionWorker<Document, B, H>
where
	Document: for<'d> DocumentT<'d>,
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
