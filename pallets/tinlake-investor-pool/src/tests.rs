use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use primitives_tokens::CurrencyId;
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

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches: tranches.into(),
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 40,
			available_reserve: Zero::zero(),
			total_reserve: 39,
		};

		let full_solution = epoch_targets
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		let current_tranche_values = [80, 20, 5, 5];

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(
				pool,
				&epoch_targets,
				&current_tranche_values,
				&full_solution
			),
			Error::<Test>::InsufficientCurrency
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

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches: tranches.into(),
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 5,
			available_reserve: Zero::zero(),
			total_reserve: 40,
		};

		let full_solution = epoch_targets
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		let current_tranche_values = [80, 20, 5, 5];

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(
				pool,
				&epoch_targets,
				&current_tranche_values,
				&full_solution
			),
			Error::<Test>::InsufficientReserve
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

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches: tranches.into(),
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
		};

		let full_solution = epoch_targets
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		let current_tranche_values = [80, 20, 5, 5];

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(
				pool,
				&epoch_targets,
				&current_tranche_values,
				&full_solution
			),
			Error::<Test>::SubordinationRatioViolated
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

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches: tranches.into(),
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
		};

		let full_solution = epoch_targets
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		let current_tranche_values = [80, 20, 5, 5];

		assert_ok!(TinlakeInvestorPool::is_epoch_valid(
			pool,
			&epoch_targets,
			&current_tranche_values,
			&full_solution
		));
	});
}
