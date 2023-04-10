use cfg_mocks::pallet_mock_accrual;
use cfg_traits::accrual::{AccrualRate, Adjustment, DebtAccrual};
use frame_support::{assert_err, assert_ok};
use sp_arithmetic::fixed_point::FixedU64;
use sp_runtime::DispatchError;

impl pallet_mock_accrual::Config for Runtime {
	type Cache = MockAccrual;
	type InnerRate = FixedU64;
	type Moment = u64;
	type OuterRate = u8;
}

cfg_mocks::make_runtime_for_mock!(Runtime, MockAccrual, pallet_mock_accrual, new_test_ext);

const ERROR: DispatchError = DispatchError::Other("Error");
const OUTER_1: u8 = 1;
const OUTER_2: u8 = 2;
const WRONG_OUTER: u8 = 0;
const LAST: u64 = 1000;

fn config_mocks() {
	MockAccrual::mock_accrual_rate(|outer| match outer {
		OUTER_1 => Ok(AccrualRate {
			inner: FixedU64::from_float(0.5),
			acc: FixedU64::from_float(0.6),
		}),
		OUTER_2 => Ok(AccrualRate {
			inner: FixedU64::from_float(0.3),
			acc: FixedU64::from_float(0.2),
		}),
		_ => Err(ERROR),
	});
	MockAccrual::mock_last_updated(|| LAST);
}

#[test]
fn wrong_outer() {
	const NORM_DEBT: u64 = 100;
	const WHEN: u64 = 10000;

	new_test_ext().execute_with(|| {
		config_mocks();

		assert_err!(MockAccrual::current_debt(WRONG_OUTER, NORM_DEBT), ERROR);
		assert_err!(
			MockAccrual::calculate_debt(WRONG_OUTER, NORM_DEBT, WHEN),
			ERROR
		);
		assert_err!(
			MockAccrual::adjust_debt(WRONG_OUTER, NORM_DEBT, Adjustment::Increase(42u32)),
			ERROR
		);
		assert_err!(
			MockAccrual::normalize_debt(WRONG_OUTER, OUTER_2, NORM_DEBT),
			ERROR
		);
		assert_err!(
			MockAccrual::normalize_debt(OUTER_1, WRONG_OUTER, NORM_DEBT),
			ERROR
		);
	});
}

#[test]
fn calculate_debt() {
	const NORM_DEBT: u64 = 100;

	new_test_ext().execute_with(|| {
		config_mocks();

		assert_ok!(
			MockAccrual::calculate_debt(OUTER_1, NORM_DEBT, LAST),
			(NORM_DEBT as f32 * 0.5) as u64
		);

		assert_ok!(
			MockAccrual::calculate_debt(OUTER_1, NORM_DEBT, LAST + 100),
			(NORM_DEBT as f32 * 0.5) as u64
		);

		assert_ok!(
			MockAccrual::calculate_debt(OUTER_1, NORM_DEBT, LAST - 100),
			(NORM_DEBT as f32 * 0.5) as u64
		);
	});
}
