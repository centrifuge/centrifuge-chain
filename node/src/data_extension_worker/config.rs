use std::{path::PathBuf, time::Duration};

use clap::Parser;
use sc_network::{
	request_responses::{IncomingRequest, ProtocolConfig},
	ProtocolName,
};

/// The DataExtensionWorker configuration used when running a node.
#[derive(Debug, Parser)]
pub struct DataExtensionWorkerConfiguration {
	/// Path used for RocksDB.
	#[clap(value_parser, default_value = "/tmp/centrifuge/data-extension-worker")]
	pub data_extension_worker_db_path: Option<PathBuf>,
}

mod p2p {
	use super::*;

	pub(crate) const PROTOCOL_NAME: &str = "/centrifuge/data_extension_worker/1";
	pub(crate) const FALLBACK_PROTOCOL_NAMES: [&str; 1] = ["/data_extension_worker/1"];
	pub(crate) const MAX_REQUEST_SIZE: u64 = 5 * 1024 * 1024; // 5 MiB
	pub(crate) const MAX_RESPONSE_SIZE: u64 = 5 * 1024 * 1024; // 5 MiB
	pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

	pub fn get_data_extension_worker_request_response_config(
		request_channel: async_channel::Sender<IncomingRequest>,
	) -> ProtocolConfig {
		ProtocolConfig {
			name: ProtocolName::Static(PROTOCOL_NAME),
			fallback_names: FALLBACK_PROTOCOL_NAMES.iter().map(|&s| s.into()).collect(),
			max_request_size: MAX_REQUEST_SIZE,
			max_response_size: MAX_RESPONSE_SIZE,
			request_timeout: REQUEST_TIMEOUT,
			inbound_queue: Some(request_channel),
		}
	}
}

pub use p2p::*;
