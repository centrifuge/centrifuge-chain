use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use primitives_tokens::CurrencyId;
use sp_runtime::traits::{One, Zero};
use sp_runtime::Perquintill;

#[test]
fn core_constraints_currency_available_cant_cover_redemptions() {
	new_test_ext().execute_with(|| {
		let tranches: Vec<_> = std::iter::repeat(Tranche {
			epoch_redeem: 10,
			..Default::default()
		})
		.take(4)
		.collect();

		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 5, 5]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				supply: tranche.epoch_supply,
				redeem: tranche.epoch_redeem,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 40,
			available_reserve: Zero::zero(),
			total_reserve: 39,
		};

		let epoch = EpochExecutionInfo {
			nav: Zero::zero(),
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(pool, &epoch, &full_solution),
			Error::<Test>::InsufficientCurrency
		);
	});
}

#[test]
fn pool_constraints_pool_reserve_above_max_reserve() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			epoch_supply: 10,
			epoch_redeem: 10,
			..Default::default()
		};
		let tranche_b = Tranche {
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
			..Default::default()
		};
		let tranche_c = Tranche {
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
			..Default::default()
		};
		let tranche_d = Tranche {
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
			..Default::default()
		};
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];
		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 5, 5]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				supply: tranche.epoch_supply,
				redeem: tranche.epoch_redeem,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 5,
			available_reserve: Zero::zero(),
			total_reserve: 40,
		};

		let epoch = EpochExecutionInfo {
			nav: Zero::zero(),
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(pool, &epoch, &full_solution),
			Error::<Test>::InsufficientReserve
		);
	});
}

#[test]
fn pool_constraints_tranche_violates_sub_ratio() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.4), // Violates constraint here
			epoch_supply: 100,
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranche_b = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: 20,
			..Default::default()
		};
		let tranche_c = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranche_d = Tranche {
			min_subordination_ratio: Perquintill::zero(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];

		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 5, 5]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				supply: tranche.epoch_supply,
				redeem: tranche.epoch_redeem,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
		};

		let epoch = EpochExecutionInfo {
			nav: Zero::zero(),
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(pool, &epoch, &full_solution),
			Error::<Test>::SubordinationRatioViolated
		);
	});
}

#[test]
fn pool_constraints_pass() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.2),
			epoch_supply: 100,
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranche_b = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: 30,
			..Default::default()
		};
		let tranche_c = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranche_d = Tranche {
			min_subordination_ratio: Perquintill::zero(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];

		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 5, 5]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				supply: tranche.epoch_supply,
				redeem: tranche.epoch_redeem,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
		};

		let epoch = EpochExecutionInfo {
			nav: Zero::zero(),
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		assert_ok!(TinlakeInvestorPool::is_epoch_valid(
			pool,
			&epoch,
			&full_solution
		));
	});
}
