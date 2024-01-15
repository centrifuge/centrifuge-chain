use serde::{Deserialize, Serialize};

pub trait PoolInfo: Clone + Send + Sync + Serialize + for<'p> Deserialize<'p> + 'static {}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct CentrifugePoolInfo {}

impl PoolInfo for CentrifugePoolInfo {}
