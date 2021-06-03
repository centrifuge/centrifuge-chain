use super::{WeightInfo, Weight};

impl WeightInfo for () {
    fn set_fee() -> Weight {
        195_000_000
    }
}
