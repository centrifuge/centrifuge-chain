use cfg_primitives::Balance;
use frame_support::{assert_noop, assert_ok};
use rand::Rng;
use sp_arithmetic::FixedPointNumber;

use super::*;
use crate::mock::{
	add_fees, assert_pending_fee, config_change_mocks, config_mocks, default_chargeable_fees,
	default_fees, default_fixed_fee, new_fee, new_test_ext, OrmlTokens, PoolFees, Runtime,
	RuntimeOrigin, System, ADMIN, ANY, BUCKET, CHANGE_ID, DESTINATION, EDITOR,
	ERR_CHANGE_GUARD_RELEASE, NOT_ADMIN, NOT_DESTINATION, NOT_EDITOR, POOL,
};

// TODO: CannotCharge
// TODO: Pending for fixed

mod extrinsics {
	use super::*;

	mod should_work {
		use super::*;

		#[test]
		fn propose_new_fee_works() {
			new_test_ext().execute_with(|| {
				let fees = default_fees();
				config_mocks();

				for (i, fee) in fees.into_iter().enumerate() {
					assert!(
						PoolFees::propose_new_fee(
							RuntimeOrigin::signed(ADMIN),
							POOL,
							BUCKET,
							fee.clone()
						)
						.is_ok(),
						"Failed to propose fee {:?} at position {:?}",
						fee,
						i
					);

					System::assert_last_event(
						Event::<Runtime>::Proposed {
							pool_id: POOL,
							bucket: BUCKET,
							fee,
						}
						.into(),
					);
				}
			})
		}

		#[test]
		fn apply_new_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();
				config_change_mocks(&default_fixed_fee());

				assert_ok!(PoolFees::apply_new_fee(
					RuntimeOrigin::signed(ANY),
					POOL,
					CHANGE_ID
				));

				System::assert_last_event(
					Event::<Runtime>::Added {
						pool_id: POOL,
						bucket: BUCKET,
						fee: default_fixed_fee(),
						fee_id: 1,
					}
					.into(),
				);
			})
		}

		#[test]
		fn remove_only_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![default_fixed_fee()]);

				assert_ok!(PoolFees::remove_fee(RuntimeOrigin::signed(EDITOR), 1));

				System::assert_last_event(
					Event::<Runtime>::Removed {
						pool_id: POOL,
						bucket: BUCKET,
						fee_id: 1,
					}
					.into(),
				);
			})
		}

		#[test]
		fn remove_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();

				let pool_fees = default_fees();
				assert!(pool_fees.len() > 1);
				add_fees(pool_fees);

				let last_fee_id = LastFeeId::<Runtime>::get();
				assert_ok!(PoolFees::remove_fee(
					RuntimeOrigin::signed(EDITOR),
					last_fee_id
				));
				System::assert_last_event(
					Event::<Runtime>::Removed {
						pool_id: POOL,
						bucket: BUCKET,
						fee_id: last_fee_id,
					}
					.into(),
				);
			})
		}

		#[test]
		fn charge_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();
				let pool_fees = default_chargeable_fees();
				add_fees(pool_fees.clone());

				for (i, fee) in pool_fees.into_iter().enumerate() {
					let fee_id = (i + 1) as u64;
					assert_ok!(PoolFees::charge_fee(
						RuntimeOrigin::signed(DESTINATION),
						fee_id,
						1000
					));
					assert_pending_fee(fee_id, fee.clone(), 1000, 0, 0);
					System::assert_last_event(
						Event::<Runtime>::Charged {
							fee_id,
							amount: 1000,
							pending: 1000,
						}
						.into(),
					);

					assert_ok!(PoolFees::charge_fee(
						RuntimeOrigin::signed(DESTINATION),
						fee_id,
						337
					));
					assert_pending_fee(fee_id, fee.clone(), 1337, 0, 0);
					System::assert_last_event(
						Event::<Runtime>::Charged {
							fee_id,
							amount: 337,
							pending: 1337,
						}
						.into(),
					);
				}
			})
		}

		#[test]
		fn uncharge_fee_works() {
			new_test_ext().execute_with(|| {
				config_mocks();

				let pool_fees = default_chargeable_fees();
				add_fees(pool_fees.clone());
				let mut rng = rand::thread_rng();

				for (i, fee) in pool_fees.into_iter().enumerate() {
					let fee_id = (i + 1) as u64;
					let charge_amount: Balance = rng.gen_range(1..u128::MAX);
					let uncharge_amount: Balance = rng.gen_range(1..=charge_amount);

					assert_ok!(PoolFees::charge_fee(
						RuntimeOrigin::signed(DESTINATION),
						fee_id,
						charge_amount
					));

					assert_ok!(PoolFees::uncharge_fee(
						RuntimeOrigin::signed(DESTINATION),
						fee_id,
						uncharge_amount
					));
					assert_pending_fee(fee_id, fee.clone(), charge_amount - uncharge_amount, 0, 0);

					System::assert_last_event(
						Event::<Runtime>::Uncharged {
							fee_id,
							amount: uncharge_amount,
							pending: charge_amount - uncharge_amount,
						}
						.into(),
					);
				}
			})
		}
	}

	mod should_fail {
		use sp_arithmetic::ArithmeticError;
		use sp_runtime::DispatchError;

		use super::*;
		use crate::mock::default_chargeable_fee;

		#[test]
		fn propose_new_fee_wrong_origin() {
			new_test_ext().execute_with(|| {
				config_mocks();
				let fees = default_fees();

				for account in NOT_ADMIN {
					assert_noop!(
						PoolFees::propose_new_fee(
							RuntimeOrigin::signed(account),
							POOL,
							BUCKET,
							fees.get(0).unwrap().clone()
						),
						Error::<Runtime>::NotPoolAdmin
					);
				}
			})
		}

		#[test]
		fn propose_new_fee_missing_pool() {
			new_test_ext().execute_with(|| {
				config_mocks();
				assert_noop!(
					PoolFees::propose_new_fee(
						RuntimeOrigin::signed(ADMIN),
						POOL + 1,
						BUCKET,
						default_fixed_fee()
					),
					Error::<Runtime>::PoolNotFound
				);
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
		fn apply_new_fee_missing_pool() {
			new_test_ext().execute_with(|| {
				config_mocks();

				// Requires mocking ChangeGuard::release
				assert_noop!(
					PoolFees::apply_new_fee(RuntimeOrigin::signed(ANY), POOL + 1, CHANGE_ID),
					Error::<Runtime>::PoolNotFound
				);
			})
		}

		#[test]
		fn remove_fee_wrong_origin() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![default_fixed_fee()]);

				for account in NOT_EDITOR {
					assert_noop!(
						PoolFees::remove_fee(RuntimeOrigin::signed(account), 1),
						Error::<Runtime>::UnauthorizedEdit
					);
				}
			})
		}

		#[test]
		fn remove_fee_missing_fee() {
			new_test_ext().execute_with(|| {
				config_mocks();
				assert_noop!(
					PoolFees::remove_fee(RuntimeOrigin::signed(EDITOR), 1),
					Error::<Runtime>::FeeNotFound
				);
			})
		}

		#[test]
		fn charge_fee_wrong_origin() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![default_fixed_fee()]);

				for account in NOT_DESTINATION {
					assert_noop!(
						PoolFees::charge_fee(RuntimeOrigin::signed(account), 1, 1000),
						Error::<Runtime>::UnauthorizedCharge
					);
				}
			})
		}

		#[test]
		fn charge_fee_missing_fee() {
			new_test_ext().execute_with(|| {
				config_mocks();
				assert_noop!(
					PoolFees::charge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1000),
					Error::<Runtime>::FeeNotFound
				);
			})
		}

		#[test]
		fn charge_fee_overflow() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![default_chargeable_fee()]);

				assert_ok!(PoolFees::charge_fee(
					RuntimeOrigin::signed(DESTINATION),
					1,
					u128::MAX
				));
				assert_noop!(
					PoolFees::charge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1),
					DispatchError::Arithmetic(ArithmeticError::Overflow)
				);
			})
		}

		#[test]
		fn uncharge_fee_wrong_origin() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![default_chargeable_fee()]);

				for account in NOT_DESTINATION {
					assert_noop!(
						PoolFees::uncharge_fee(RuntimeOrigin::signed(account), 1, 1000),
						Error::<Runtime>::UnauthorizedCharge
					);
				}
			})
		}

		#[test]
		fn uncharge_fee_missing_fee() {
			new_test_ext().execute_with(|| {
				config_mocks();
				assert_noop!(
					PoolFees::uncharge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1000),
					Error::<Runtime>::FeeNotFound
				);
			})
		}

		#[test]
		fn uncharge_fee_overflow() {
			new_test_ext().execute_with(|| {
				config_mocks();
				add_fees(vec![default_chargeable_fee()]);

				assert_noop!(
					PoolFees::uncharge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1),
					DispatchError::Arithmetic(ArithmeticError::Underflow)
				);
			})
		}
	}
}

mod disbursements {
	use cfg_primitives::SECONDS_PER_YEAR;
	use cfg_types::{
		fixed_point::Rate,
		pools::{PoolFeeAmount, PoolFeeType},
	};
	use frame_support::traits::fungibles::Inspect;

	use super::*;
	use crate::mock::{get_disbursements, pay_single_fee_and_assert, NAV, POOL_CURRENCY};

	mod single_fee {
		use super::*;

		mod fixed {
			use super::*;

			mod share_of_portfolio_valuation {
				use super::*;
				#[test]
				fn sufficient_reserve_sfs() {
					new_test_ext().execute_with(|| {
						config_mocks();
						let fee_id = 1;
						let res_pre_fees = NAV;
						let annual_rate = Rate::saturating_from_rational(1, 10);
						let fee_amount = res_pre_fees / 10;

						let fee = new_fee(PoolFeeType::Fixed {
							limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
						});
						add_fees(vec![fee.clone()]);

						// Fees (10% of NAV) consume 10% of reserve
						let res_post_fees = PoolFees::update_active_fees(
							POOL,
							BUCKET,
							NAV,
							res_pre_fees,
							SECONDS_PER_YEAR,
						);

						assert_eq!(res_post_fees, res_pre_fees - fee_amount);
						assert_eq!(get_disbursements(), vec![fee_amount]);

						pay_single_fee_and_assert(fee_id, fee_amount);
					});
				}

				#[test]
				fn insufficient_reserve_sfs() {
					new_test_ext().execute_with(|| {
						config_mocks();
						let fee_id = 1;
						let res_pre_fees = NAV / 100;
						let annual_rate = Rate::saturating_from_rational(1, 10);

						let fee = new_fee(PoolFeeType::Fixed {
							limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
						});
						add_fees(vec![fee.clone()]);

						// Fees (10% of NAV) consume entire reserve
						let res_post_fees = PoolFees::update_active_fees(
							POOL,
							BUCKET,
							NAV,
							res_pre_fees,
							SECONDS_PER_YEAR,
						);

						assert_eq!(res_post_fees, 0);
						assert_eq!(get_disbursements(), vec![res_pre_fees]);

						pay_single_fee_and_assert(fee_id, res_pre_fees);
					});
				}
			}

			mod amount_per_second {
				use super::*;
				#[test]
				fn sufficient_reserve_sfa() {
					new_test_ext().execute_with(|| {
						config_mocks();
						let fee_id = 1;
						let res_pre_fees: Balance = (2 * SECONDS_PER_YEAR).into();
						let amount_per_second = 1;
						let fee_amount = SECONDS_PER_YEAR.into();

						let fee = new_fee(PoolFeeType::Fixed {
							limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
						});
						add_fees(vec![fee.clone()]);

						// Fees (10% of NAV) consume 10% of reserve
						let res_post_fees = PoolFees::update_active_fees(
							POOL,
							BUCKET,
							NAV,
							res_pre_fees,
							SECONDS_PER_YEAR,
						);

						assert_eq!(res_post_fees, res_pre_fees - fee_amount);
						assert_eq!(get_disbursements(), vec![fee_amount]);

						pay_single_fee_and_assert(fee_id, fee_amount);
					});
				}

				#[test]
				fn insufficient_reserve_sfa() {
					new_test_ext().execute_with(|| {
						config_mocks();
						let fee_id = 1;
						let res_pre_fees: Balance = (SECONDS_PER_YEAR / 2).into();
						let amount_per_second = 1;

						let fee = new_fee(PoolFeeType::Fixed {
							limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
						});
						add_fees(vec![fee.clone()]);

						// Fees (10% of NAV) consume entire reserve
						let res_post_fees = PoolFees::update_active_fees(
							POOL,
							BUCKET,
							NAV,
							res_pre_fees,
							SECONDS_PER_YEAR,
						);

						assert_eq!(res_post_fees, 0);
						assert_eq!(get_disbursements(), vec![res_pre_fees]);

						pay_single_fee_and_assert(fee_id, res_pre_fees);
					});
				}
			}
		}

		mod charged_up_to {
			use super::*;

			mod fixed {

				use super::*;

				mod share_of_portfolio {
					use super::*;
					use crate::mock::assert_pending_fee;
					#[test]
					fn empty_charge_scfs() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV;
							let annual_rate = Rate::saturating_from_rational(1, 10);

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
							});
							add_fees(vec![fee.clone()]);

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, res_pre_fees);
							assert_eq!(get_disbursements().into_iter().sum::<Balance>(), 0);
							pay_single_fee_and_assert(fee_id, 0);
						});
					}

					#[test]
					fn below_max_charge_sufficient_reserve_scfs() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV;
							let annual_rate = Rate::saturating_from_rational(1, 10);
							let charged_amount = NAV / 10 - 1;

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::charge_fee(
								RuntimeOrigin::signed(DESTINATION),
								fee_id,
								charged_amount
							));

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, res_pre_fees - charged_amount);
							assert_eq!(get_disbursements(), vec![charged_amount]);

							pay_single_fee_and_assert(fee_id, charged_amount);
						});
					}

					#[test]
					fn max_charge_sufficient_reserve_scfs() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV;
							let annual_rate = Rate::saturating_from_rational(1, 10);
							let charged_amount = NAV / 10;

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::charge_fee(
								RuntimeOrigin::signed(DESTINATION),
								fee_id,
								charged_amount
							));

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, res_pre_fees - charged_amount);
							assert_eq!(get_disbursements(), vec![charged_amount]);

							pay_single_fee_and_assert(fee_id, charged_amount);
						});
					}

					#[test]
					fn excess_charge_sufficient_reserve_scfs() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV;
							let annual_rate = Rate::saturating_from_rational(1, 10);
							let max_chargeable_amount = NAV / 10;
							let charged_amount = max_chargeable_amount + 1;

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::charge_fee(
								RuntimeOrigin::signed(DESTINATION),
								fee_id,
								charged_amount
							));

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, res_pre_fees - max_chargeable_amount);
							assert_eq!(get_disbursements(), vec![max_chargeable_amount]);
							assert_pending_fee(fee_id, fee.clone(), 1, 0, max_chargeable_amount);

							pay_single_fee_and_assert(fee_id, max_chargeable_amount);
						});
					}

					#[test]
					fn insufficient_reserve_scfs() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV / 100;
							let annual_rate = Rate::saturating_from_rational(1, 10);
							let charged_amount = NAV / 10;
							let fee_amount = res_pre_fees;

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::charge_fee(
								RuntimeOrigin::signed(DESTINATION),
								fee_id,
								charged_amount
							));

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, 0);
							assert_eq!(get_disbursements(), vec![fee_amount]);
							assert_pending_fee(
								fee_id,
								fee.clone(),
								charged_amount - fee_amount,
								charged_amount - fee_amount,
								fee_amount,
							);

							pay_single_fee_and_assert(fee_id, fee_amount);
						});
					}
				}

				mod amount_per_second {
					use super::*;
					use crate::mock::assert_pending_fee;

					#[test]
					fn empty_charge_scfa() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV;
							let amount_per_second = 1;

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
							});
							add_fees(vec![fee.clone()]);

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, res_pre_fees);
							assert_eq!(get_disbursements().into_iter().sum::<Balance>(), 0);
							pay_single_fee_and_assert(fee_id, 0);
						});
					}

					#[test]
					fn below_max_charge_sufficient_reserve_scfa() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV;
							let amount_per_second = 1;
							let charged_amount = (SECONDS_PER_YEAR - 1).into();

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::charge_fee(
								RuntimeOrigin::signed(DESTINATION),
								fee_id,
								charged_amount
							));

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, res_pre_fees - charged_amount);
							assert_eq!(get_disbursements(), vec![charged_amount]);

							pay_single_fee_and_assert(fee_id, charged_amount);
						});
					}

					#[test]
					fn max_charge_sufficient_reserve_scfa() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV;
							let amount_per_second = 1;
							let charged_amount = SECONDS_PER_YEAR.into();

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::charge_fee(
								RuntimeOrigin::signed(DESTINATION),
								fee_id,
								charged_amount
							));

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, res_pre_fees - charged_amount);
							assert_eq!(get_disbursements(), vec![charged_amount]);

							pay_single_fee_and_assert(fee_id, charged_amount);
						});
					}

					#[test]
					fn excess_charge_sufficient_reserve_scfa() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let res_pre_fees = NAV;
							let amount_per_second = 1;
							let max_chargeable_amount = SECONDS_PER_YEAR.into();
							let charged_amount = max_chargeable_amount + 1;

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::charge_fee(
								RuntimeOrigin::signed(DESTINATION),
								fee_id,
								charged_amount
							));

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, res_pre_fees - max_chargeable_amount);
							assert_eq!(get_disbursements(), vec![max_chargeable_amount]);
							assert_pending_fee(fee_id, fee.clone(), 1, 0, max_chargeable_amount);
							pay_single_fee_and_assert(fee_id, max_chargeable_amount);
						});
					}

					#[test]
					fn insufficient_reserve_scfa() {
						new_test_ext().execute_with(|| {
							config_mocks();
							let fee_id = 1;
							let amount_per_second = 1;
							let res_pre_fees: Balance = (SECONDS_PER_YEAR / 2 + 1).into();
							let charged_amount = SECONDS_PER_YEAR.into();
							let fee_amount = res_pre_fees;

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::charge_fee(
								RuntimeOrigin::signed(DESTINATION),
								fee_id,
								charged_amount
							));

							let res_post_fees = PoolFees::update_active_fees(
								POOL,
								BUCKET,
								NAV,
								res_pre_fees,
								SECONDS_PER_YEAR,
							);

							assert_eq!(res_post_fees, 0);
							assert_eq!(get_disbursements(), vec![fee_amount]);
							assert_pending_fee(
								fee_id,
								fee.clone(),
								charged_amount - fee_amount,
								charged_amount - fee_amount,
								fee_amount,
							);

							pay_single_fee_and_assert(fee_id, fee_amount);
						});
					}
				}
			}
		}
	}

	mod waterfall {
		use super::*;
		use crate::mock::assert_pending_fee;

		#[test]
		fn fixed_charged_charged() {
			new_test_ext().execute_with(|| {
				config_mocks();
				let charged_fee_ids = vec![2, 3];
				let res_pre_fees = NAV;
				let annual_rate = Rate::saturating_from_rational(1, 100);
				let fixed_fee_amount = NAV / 100;
				let amount_per_seconds = vec![2, 1];
				let payable = vec![(2 * SECONDS_PER_YEAR).into(), SECONDS_PER_YEAR.into()];
				let charged_y1 = vec![1, 2 * payable[1]];
				let charged_y2 = vec![payable[0], payable[1]];

				let fees = vec![
					new_fee(PoolFeeType::Fixed {
						limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
					}),
					new_fee(PoolFeeType::ChargedUpTo {
						limit: PoolFeeAmount::AmountPerSecond(amount_per_seconds[0]),
					}),
					new_fee(PoolFeeType::ChargedUpTo {
						limit: PoolFeeAmount::AmountPerSecond(amount_per_seconds[1]),
					}),
				];
				add_fees(fees.clone());

				// Year 1
				assert_ok!(PoolFees::charge_fee(
					RuntimeOrigin::signed(DESTINATION),
					charged_fee_ids[0],
					charged_y1[0]
				));
				assert_ok!(PoolFees::charge_fee(
					RuntimeOrigin::signed(DESTINATION),
					charged_fee_ids[1],
					charged_y1[1]
				));
				let res_post_fees =
					PoolFees::update_active_fees(POOL, BUCKET, NAV, res_pre_fees, SECONDS_PER_YEAR);
				assert_eq!(
					res_post_fees,
					res_pre_fees - fixed_fee_amount - charged_y1[0] - payable[1]
				);
				assert_eq!(
					get_disbursements(),
					vec![fixed_fee_amount, charged_y1[0], payable[1]]
				);
				assert_pending_fee(
					charged_fee_ids[0],
					fees[1].clone(),
					0,
					payable[0] - charged_y1[0],
					charged_y1[0],
				);
				assert_pending_fee(
					charged_fee_ids[1],
					fees[2].clone(),
					payable[1],
					0,
					charged_y1[1] - payable[1],
				);

				// Pay disbursements
				assert_ok!(PoolFees::pay_active_fees(POOL, BUCKET));
				assert_eq!(get_disbursements().into_iter().sum::<Balance>(), 0);
				assert_eq!(
					OrmlTokens::balance(POOL_CURRENCY, &DESTINATION),
					fixed_fee_amount + charged_y1[0] + payable[1]
				);
				System::assert_has_event(
					Event::Paid {
						fee_id: 1,
						amount: fixed_fee_amount,
						destination: DESTINATION,
					}
					.into(),
				);
				System::assert_has_event(
					Event::Paid {
						fee_id: charged_fee_ids[0],
						amount: charged_y1[0],
						destination: DESTINATION,
					}
					.into(),
				);
				System::assert_last_event(
					Event::Paid {
						fee_id: charged_fee_ids[1],
						amount: payable[1],
						destination: DESTINATION,
					}
					.into(),
				);

				// Year 2: Make reserve insufficient to handle all fees (last fee
				// falls short
				let res_pre_fees = fixed_fee_amount + charged_y2[0] + 1;
				assert_ok!(PoolFees::charge_fee(
					RuntimeOrigin::signed(DESTINATION),
					charged_fee_ids[0],
					charged_y2[0]
				));
				assert_ok!(PoolFees::charge_fee(
					RuntimeOrigin::signed(DESTINATION),
					charged_fee_ids[1],
					charged_y2[1]
				));
				let res_post_fees =
					PoolFees::update_active_fees(POOL, BUCKET, NAV, res_pre_fees, SECONDS_PER_YEAR);
				assert_eq!(res_post_fees, 0);
				assert_eq!(
					get_disbursements(),
					vec![fixed_fee_amount, charged_y2[0], 1]
				);
				assert_pending_fee(
					charged_fee_ids[0],
					fees[1].clone(),
					0,
					2 * payable[0] - charged_y1[0] - charged_y2[0],
					charged_y2[0],
				);
				assert_pending_fee(
					charged_fee_ids[1],
					fees[2].clone(),
					2 * payable[1] - 1,
					payable[1] - 1,
					1,
				);

				// Pay disbursements
				assert_ok!(PoolFees::pay_active_fees(POOL, BUCKET));
				assert_eq!(get_disbursements().into_iter().sum::<Balance>(), 0);
				assert_eq!(
					OrmlTokens::balance(POOL_CURRENCY, &DESTINATION),
					2 * fixed_fee_amount + charged_y1[0] + payable[1] + charged_y2[0] + 1
				);
				System::assert_has_event(
					Event::Paid {
						fee_id: 1,
						amount: fixed_fee_amount,
						destination: DESTINATION,
					}
					.into(),
				);
				System::assert_has_event(
					Event::Paid {
						fee_id: charged_fee_ids[0],
						amount: charged_y2[0],
						destination: DESTINATION,
					}
					.into(),
				);
				System::assert_last_event(
					Event::Paid {
						fee_id: charged_fee_ids[1],
						amount: 1,
						destination: DESTINATION,
					}
					.into(),
				);
			});
		}

		// TODO
		// fn charged_fixed_insufficient_reserve
	}
}

// TODO: Test paying without preparation does nothing despite fixed/charged
// being existent
