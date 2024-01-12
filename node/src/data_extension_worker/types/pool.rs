use serde::{Deserialize, Serialize};

pub trait PoolInfo<'p>: Clone + Send + Sync + Serialize + Deserialize<'p> + 'static {}

#[derive(Clone, Serialize, Deserialize, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct CentrifugePoolInfo {}

impl<'p> PoolInfo<'p> for CentrifugePoolInfo {}
