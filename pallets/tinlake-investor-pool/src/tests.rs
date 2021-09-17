use crate::mock::*;
use crate::{self as tinlake_investor_pool};
use sp_runtime::traits::Zero;

#[test]
fn core_constraints_currency_available_cant_cover_redemptions() {
	new_test_ext().execute_with(|| {
		let tranche_a = tinlake_investor_pool::Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: 10000000,
		};
		let tranche_b = tinlake_investor_pool::Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
		};
		let tranche_c = tinlake_investor_pool::Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
		};
		let tranche_d = tinlake_investor_pool::Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
		};
		let tranches = [tranche_a, tranche_b, tranche_c, tranche_d];

		assert_eq!(
			TinlakeInvestorPool::is_epoch_valid_x(1000000, &tranches),
			false
		);
	});
}
