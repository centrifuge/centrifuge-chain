use cfg_types::pools::FeeBucket;
use frame_support::{assert_noop, assert_ok};
use rand::Rng;
use sp_arithmetic::FixedPointNumber;
use sp_runtime::SaturatedConversion;

use super::*;
use crate::{
	mock::{
		add_fees, config_change_mocks, config_mocks, fees, new_fee, new_test_ext, ten_percent_rate,
		AccountId, ChangeId, CurrencyId, MockChangeGuard, MockPermissions, MockPools, OrmlTokens,
		PoolFees, Runtime, RuntimeOrigin, ADMIN, ANY, CHANGE_ID, DESTINATION, EDITOR,
		ERR_CHANGE_GUARD_RELEASE, NOT_ADMIN, NOT_DESTINATION, NOT_EDITOR, POOL,
	},
	types::Change,
};

mod extrinsics {
	use super::*;

	mod should_work {
		use cfg_primitives::Balance;

		use super::*;

		#[test]
		fn propose_new_fee_works() {
			new_test_ext().execute_with(|| {
				let fees = fees();
				config_mocks();

				for (i, fee) in fees.into_iter().enumerate() {
					assert!(
						PoolFees::propose_new_fee(
							RuntimeOrigin::signed(ADMIN),
							POOL,
							FeeBucket::Top,
							fee.clone()
						)
						.is_ok(),
						"Failed to propose fee {:?} at position {:?}",
						fee,
						i
					);
				}
			})
		}

		#[test]
		fn apply_new_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();
				config_change_mocks(fees().first().unwrap());

				assert_ok!(PoolFees::apply_new_fee(
					RuntimeOrigin::signed(ANY),
					POOL,
					CHANGE_ID
				));
			})
		}

		#[test]
		fn remove_only_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![fees().first().unwrap().clone()]);

				assert_ok!(PoolFees::remove_fee(RuntimeOrigin::signed(EDITOR), 1));
			})
		}

		#[test]
		fn remove_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();

				let pool_fees = fees();
				assert!(pool_fees.len() > 1);
				add_fees(pool_fees);

				let last_fee_id = LastFeeId::<Runtime>::get();
				assert_ok!(PoolFees::remove_fee(
					RuntimeOrigin::signed(EDITOR),
					last_fee_id
				));
			})
		}

		#[test]
		fn charge_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();
				let pool_fees = fees();
				add_fees(pool_fees.clone());

				for i in 1..=pool_fees.len() {
					assert_ok!(PoolFees::charge_fee(
						RuntimeOrigin::signed(DESTINATION),
						i.saturated_into(),
						1000
					));
				}
			})
		}

		#[test]
		fn uncharge_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();
				let pool_fees = fees();
				add_fees(pool_fees.clone());
				let mut rng = rand::thread_rng();

				for i in 1..=pool_fees.len() {
					let charge_amount: Balance = rng.gen_range(1..u128::MAX);
					let uncharge_amount: Balance = rng.gen_range(1..=charge_amount);

					assert_ok!(PoolFees::charge_fee(
						RuntimeOrigin::signed(DESTINATION),
						i.saturated_into(),
						charge_amount
					));

					assert_ok!(PoolFees::uncharge_fee(
						RuntimeOrigin::signed(DESTINATION),
						i.saturated_into(),
						uncharge_amount
					));
				}
			})
		}
	}

	mod should_fail {
		use super::*;

		#[test]
		fn propose_new_wrong_origin() {
			new_test_ext().execute_with(|| {
				config_mocks();
				let fees = fees();

				for account in NOT_ADMIN {
					assert_noop!(
						PoolFees::propose_new_fee(
							RuntimeOrigin::signed(account),
							POOL,
							FeeBucket::Top,
							fees.get(0).unwrap().clone()
						),
						Error::<Runtime>::NotPoolAdmin
					);
				}
			})
		}

		#[test]
		fn apply_new_fee_changeguard_unreleased() {
			new_test_ext().execute_with(|| {
				config_mocks();

				// Requires mocking ChangeGuard::release
				assert_noop!(
					PoolFees::apply_new_fee(RuntimeOrigin::signed(ANY), POOL, CHANGE_ID),
					ERR_CHANGE_GUARD_RELEASE
				);
			})
		}

		#[test]
		fn remove_fee_wrong_origin() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![fees().first().unwrap().clone()]);

				for account in NOT_EDITOR {
					assert_noop!(
						PoolFees::remove_fee(RuntimeOrigin::signed(account), 1),
						Error::<Runtime>::UnauthorizedEdit
					);
				}
			})
		}

		#[test]
		fn charge_fee_wrong_origin() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![fees().first().unwrap().clone()]);

				for account in NOT_DESTINATION {
					assert_noop!(
						PoolFees::charge_fee(RuntimeOrigin::signed(account), 1, 1000),
						Error::<Runtime>::UnauthorizedCharge
					);
				}
			})
		}
	}
}
