use serde::{Deserialize, Serialize};

/// Message type for Data Extension Worker communication on the P2P layer.
#[derive(Deserialize, Serialize)]
pub enum DataExtensionWorkerP2PMessage {
	Ping,
	Pong,
}
