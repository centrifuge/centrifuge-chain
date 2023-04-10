use cfg_mocks::pallet_mock_accrual;
use cfg_traits::accrual::{Adjustment, DebtAccrual};
use frame_support::{assert_err, assert_ok};
use sp_arithmetic::fixed_point::FixedU64;
use sp_runtime::DispatchError;

impl pallet_mock_accrual::Config for Runtime {
	type AccRate = FixedU64;
	type Cache = ();
	type MaxRateCount = ConstU32<0>;
	type Moment = u64;
	type OuterRate = u8;
}

cfg_mocks::make_runtime_for_mock!(Runtime, Mock, pallet_mock_accrual, new_test_ext);

const ERROR: DispatchError = DispatchError::Other("Error");
const OUTER_1: u8 = 1;
const OUTER_2: u8 = 2;
const WRONG_OUTER: u8 = 0;
const LAST: u64 = 1000;

fn config_mocks() {
	Mock::mock_accrual(|outer| match outer {
		OUTER_1 => Ok(FixedU64::from_float(0.3)),
		OUTER_2 => Ok(FixedU64::from_float(0.6)),
		_ => Err(ERROR),
	});
	Mock::mock_accrual_at(|outer, moment| {
		assert!(moment < LAST);
		match outer {
			OUTER_1 => Ok(FixedU64::from_float(0.1)),
			OUTER_2 => Ok(FixedU64::from_float(0.2)),
			_ => Err(ERROR),
		}
	});
	Mock::mock_last_updated(|| LAST);
}

#[test]
fn wrong_outer() {
	const WHEN: u64 = 10000;

	new_test_ext().execute_with(|| {
		config_mocks();

		assert_err!(Mock::current_debt(WRONG_OUTER, 1), ERROR);
		assert_err!(Mock::calculate_debt(WRONG_OUTER, 1, WHEN), ERROR);
		assert_err!(
			Mock::adjust_normalized_debt(WRONG_OUTER, 1, Adjustment::Increase(42)),
			ERROR
		);
		assert_err!(Mock::renormalize_debt(WRONG_OUTER, OUTER_2, 1), ERROR);
		assert_err!(Mock::renormalize_debt(OUTER_1, WRONG_OUTER, 1), ERROR);
	});
}

#[test]
fn calculate_debt() {
	const NORM_DEBT: u64 = 100;

	new_test_ext().execute_with(|| {
		config_mocks();

		assert_ok!(
			Mock::calculate_debt(OUTER_1, NORM_DEBT, LAST),
			(NORM_DEBT as f32 * 0.3) as u64
		);

		assert_ok!(
			Mock::calculate_debt(OUTER_1, NORM_DEBT, LAST + 100),
			(NORM_DEBT as f32 * 0.3) as u64
		);

		assert_ok!(
			Mock::calculate_debt(OUTER_1, NORM_DEBT, LAST - 100),
			(NORM_DEBT as f32 * 0.1) as u64
		);
	});
}
