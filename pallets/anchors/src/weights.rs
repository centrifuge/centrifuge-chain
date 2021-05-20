use frame_support::{weights::Weight};

/// Weight functions needed for Fees.
pub trait WeightInfo {
    fn pre_commit() -> Weight;
    fn commit() -> Weight;
    fn evict_pre_commits() -> Weight;
    fn evict_anchors() -> Weight;
}

impl WeightInfo for () {
    fn pre_commit() -> Weight {
        193_000_000
    }

    fn commit() -> Weight {
        190_000_000
    }

    fn evict_pre_commits() -> Weight {
        192_000_000
    }

    fn evict_anchors() -> Weight {
        195_000_000
    }
}
