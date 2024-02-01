use serde::{Deserialize, Serialize};

use crate::data_extension_worker::types::{BaseError, Document as DocumentT};

/// Message type for Data Extension Worker communication on the P2P layer.
#[derive(Deserialize, Serialize)]
pub enum DataExtensionWorkerP2PMessage {
	Ping,
	Pong,
}
