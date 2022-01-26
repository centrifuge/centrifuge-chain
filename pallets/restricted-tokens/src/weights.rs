use frame_support::weights::Weight;

pub trait WeightInfo {
	fn transfer_native() -> Weight;
	fn transfer_other() -> Weight;
	fn transfer_keep_alive_native() -> Weight;
	fn transfer_keep_alive_other() -> Weight;
	fn transfer_all_native() -> Weight;
	fn transfer_all_other() -> Weight;
	fn force_transfer_native() -> Weight;
	fn force_transfer_other() -> Weight;
	fn set_balance_native() -> Weight;
	fn set_balance_other() -> Weight;
}

impl WeightInfo for () {
	fn transfer_native() -> Weight {
		0
	}

	fn transfer_other() -> Weight {
		0
	}

	fn transfer_keep_alive_native() -> Weight {
		0
	}

	fn transfer_keep_alive_other() -> Weight {
		0
	}

	fn transfer_all_native() -> Weight {
		0
	}

	fn transfer_all_other() -> Weight {
		0
	}

	fn force_transfer_native() -> Weight {
		0
	}

	fn force_transfer_other() -> Weight {
		0
	}

	fn set_balance_native() -> Weight {
		0
	}

	fn set_balance_other() -> Weight {
		0
	}
}
