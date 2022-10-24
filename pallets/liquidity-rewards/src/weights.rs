use frame_support::weights::Weight;

pub trait WeightInfo {
	fn stake() -> Weight;
	fn unstake() -> Weight;
	fn claim_reward() -> Weight;
	fn set_distributed_reward() -> Weight;
	fn set_epoch_duration() -> Weight;
	fn set_group_weight() -> Weight;
	fn set_currency_group() -> Weight;
	fn distribute(groups: u32) -> Weight;
	fn attach_currency() -> Weight;
	fn insert_group_weight() -> Weight;
}

impl WeightInfo for () {
	fn stake() -> Weight {
		0
	}

	fn unstake() -> Weight {
		0
	}

	fn claim_reward() -> Weight {
		0
	}

	fn set_distributed_reward() -> Weight {
		0
	}

	fn set_epoch_duration() -> Weight {
		0
	}

	fn set_group_weight() -> Weight {
		0
	}

	fn set_currency_group() -> Weight {
		0
	}

	fn distribute(groups: u32) -> Weight {
		0
	}

	fn attach_currency() -> Weight {
		0
	}

	fn insert_group_weight() -> Weight {
		0
	}
}
