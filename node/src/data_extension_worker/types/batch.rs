use serde::{Deserialize, Serialize};

pub trait Batch<'b>: Clone + Send + Sync + Serialize + Deserialize<'b> + 'static {}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct DataExtensionWorkerBatch {}

impl<'b> Batch<'b> for DataExtensionWorkerBatch {}
