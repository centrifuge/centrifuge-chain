use std::path::PathBuf;

use clap::Parser;

/// The DataExtensionWorker configuration used when running a node.
#[derive(Debug, Parser)]
pub struct DataExtensionWorkerConfiguration {
	/// Path used for RocksDB.
	#[clap(value_parser, default_value = "/tmp/centrifuge/data-extension-worker")]
	pub data_extension_worker_db_path: Option<PathBuf>,
}
