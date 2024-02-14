use serde::{Deserialize, Serialize};

pub trait Batch: Clone + Send + Sync + Serialize + for<'b> Deserialize<'b> + 'static {}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct DataExtensionWorkerBatch {}

impl Batch for DataExtensionWorkerBatch {}
