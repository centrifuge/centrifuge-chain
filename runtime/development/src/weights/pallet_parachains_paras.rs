use core::marker::PhantomData;

use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> polkadot_runtime_parachains::paras::WeightInfo for WeightInfo<T> {
	fn force_set_current_code(c: u32) -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn force_set_current_head(s: u32) -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn force_set_most_recent_context() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn force_schedule_code_upgrade(c: u32) -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn force_note_new_head(s: u32) -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn force_queue_action() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn add_trusted_validation_code(c: u32) -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn poke_unused_validation_code() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn include_pvf_check_statement_finalize_upgrade_accept() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn include_pvf_check_statement_finalize_upgrade_reject() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn include_pvf_check_statement_finalize_onboarding_accept() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn include_pvf_check_statement_finalize_onboarding_reject() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}

	fn include_pvf_check_statement() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
}
