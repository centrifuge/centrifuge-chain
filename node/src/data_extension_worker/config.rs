/// The DataExtensionWorker configuration used when running a node.
#[derive(Clone, Copy, Debug, clap::Parser)]
pub struct DataExtensionWorkerConfiguration {
	/// Flag for enabling the Data Extension Worker.
	#[clap(long)]
	pub enable_data_extension_worker: bool,

	/// RPC address for the Data Extension Worker.
	#[clap(long, default_value = "127.0.0.1")]
	pub data_extension_worker_rpc_addr: Option<std::net::IpAddr>,

	/// RPC port for the Data Extension Worker.
	#[clap(long, default_value = "33999")]
	pub data_extension_worker_rpc_port: u32,
}
