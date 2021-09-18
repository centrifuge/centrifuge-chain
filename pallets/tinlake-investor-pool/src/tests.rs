use super::*;
use crate::mock::*;
use sp_runtime::traits::{One, Zero};
use sp_runtime::Perquintill;

#[test]
fn core_constraints_currency_available_cant_cover_redemptions() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
		};
		let tranche_b = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
		};
		let tranche_c = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
		};
		let tranche_d = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
		};
		let tranches = [tranche_a, tranche_b, tranche_c, tranche_d];
		let tranche_prices = [One::one(), One::one(), One::one(), One::one()];

		let epoch_targets =
			TinlakeInvestorPool::calculate_epoch_transfers(&tranche_prices, &tranches).unwrap();

		let tranche_ratios = tranches
			.iter()
			.map(|tranche| tranche.min_subordination_ratio)
			.collect::<Vec<_>>();

		let current_tranche_values = [80, 20, 5, 5];
		let nav = Zero::zero();

		assert_eq!(
			TinlakeInvestorPool::is_epoch_valid(
				39,
				40,
				&epoch_targets,
				&tranche_ratios,
				&current_tranche_values,
				nav
			),
			false
		);
	});
}

#[test]
fn pool_constraints_pool_reserve_above_max_reserve() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: 10,
			epoch_redeem: 10,
		};
		let tranche_b = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
		};
		let tranche_c = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
		};
		let tranche_d = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Default::default(),
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
		};
		let tranches = [tranche_a, tranche_b, tranche_c, tranche_d];
		let tranche_prices = [One::one(), One::one(), One::one(), One::one()];

		let epoch_targets =
			TinlakeInvestorPool::calculate_epoch_transfers(&tranche_prices, &tranches).unwrap();

		let tranche_ratios = tranches
			.iter()
			.map(|tranche| tranche.min_subordination_ratio)
			.collect::<Vec<_>>();

		let current_tranche_values = [80, 20, 5, 5];
		let nav = Zero::zero();

		assert_eq!(
			TinlakeInvestorPool::is_epoch_valid(
				40,
				5,
				&epoch_targets,
				&tranche_ratios,
				&current_tranche_values,
				nav
			),
			false
		);
	});
}

#[test]
fn pool_constraints_tranche_violates_sub_ratio() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Perquintill::from_float(0.4), // Violates constraint here
			epoch_supply: 100,
			epoch_redeem: Zero::zero(),
		};
		let tranche_b = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: 30,
		};
		let tranche_c = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
		};
		let tranche_d = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Perquintill::zero(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
		};
		let tranches = [tranche_a, tranche_b, tranche_c, tranche_d];
		let tranche_prices = [One::one(), One::one(), One::one(), One::one()];

		let epoch_targets =
			TinlakeInvestorPool::calculate_epoch_transfers(&tranche_prices, &tranches).unwrap();

		let tranche_ratios = tranches
			.iter()
			.map(|tranche| tranche.min_subordination_ratio)
			.collect::<Vec<_>>();

		let current_tranche_values = [80, 20, 5, 5];
		let max_reserve = 150;
		let current_reserve = 50;
		let nav = Zero::zero();

		assert_eq!(
			TinlakeInvestorPool::is_epoch_valid(
				current_reserve,
				max_reserve,
				&epoch_targets,
				&tranche_ratios,
				&current_tranche_values,
				nav
			),
			false
		);
	});
}

#[test]
fn pool_constraints_pass() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Perquintill::from_float(0.2),
			epoch_supply: 100,
			epoch_redeem: Zero::zero(),
		};
		let tranche_b = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: 30,
		};
		let tranche_c = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
		};
		let tranche_d = Tranche {
			interest_per_sec: Default::default(),
			min_subordination_ratio: Perquintill::zero(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
		};
		let tranches = [tranche_a, tranche_b, tranche_c, tranche_d];
		let tranche_prices = [One::one(), One::one(), One::one(), One::one()];

		let epoch_targets =
			TinlakeInvestorPool::calculate_epoch_transfers(&tranche_prices, &tranches).unwrap();

		let tranche_ratios = tranches
			.iter()
			.map(|tranche| tranche.min_subordination_ratio)
			.collect::<Vec<_>>();

		let current_tranche_values = [80, 20, 5, 5];
		let max_reserve = 150;
		let current_reserve = 50;
		let nav = Zero::zero();

		assert_eq!(
			TinlakeInvestorPool::is_epoch_valid(
				current_reserve,
				max_reserve,
				&epoch_targets,
				&tranche_ratios,
				&current_tranche_values,
				nav
			),
			true
		);
	});
}
