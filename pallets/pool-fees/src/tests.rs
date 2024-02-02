use cfg_primitives::Balance;
use frame_support::{assert_noop, assert_ok};
use rand::Rng;
use sp_arithmetic::FixedPointNumber;

use super::*;
use crate::mock::{
	add_fees, assert_pending_fee, config_change_mocks, default_chargeable_fees, default_fees,
	default_fixed_fee, new_fee, ExtBuilder, OrmlTokens, PoolFees, Runtime, RuntimeOrigin, System,
	ADMIN, ANY, BUCKET, CHANGE_ID, DESTINATION, EDITOR, ERR_CHANGE_GUARD_RELEASE, NOT_ADMIN,
	NOT_DESTINATION, NOT_EDITOR, POOL,
};

mod extrinsics {
	use super::*;

	mod should_work {
		use super::*;

		#[test]
		fn propose_new_fee_works() {
			ExtBuilder::default().build().execute_with(|| {
				let fees = default_fees();

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
							fee_id: (i + 1) as u64,
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
			ExtBuilder::default().build().execute_with(|| {
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
			ExtBuilder::default().build().execute_with(|| {
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
			ExtBuilder::default().build().execute_with(|| {
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
			ExtBuilder::default().build().execute_with(|| {
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
			ExtBuilder::default().build().execute_with(|| {
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
		use crate::{
			mock::{
				default_chargeable_fee, ExtBuilder, MaxPoolFeesPerBucket, MockChangeGuard,
				MockIsAdmin, MockPools,
			},
			types::Change,
		};

		#[test]
		fn propose_new_fee_wrong_origin() {
			ExtBuilder::default().build().execute_with(|| {
				MockIsAdmin::mock_check(|_| false);
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
			ExtBuilder::default().build().execute_with(|| {
				MockPools::mock_pool_exists(|_| false);
				assert_noop!(
					PoolFees::propose_new_fee(
						RuntimeOrigin::signed(ADMIN),
						POOL,
						BUCKET,
						default_fixed_fee()
					),
					Error::<Runtime>::PoolNotFound
				);
			})
		}

		#[test]
		fn apply_new_fee_changeguard_unreleased() {
			ExtBuilder::default().build().execute_with(|| {
				MockChangeGuard::mock_released(move |_, _| Err(ERR_CHANGE_GUARD_RELEASE));

				// Requires mocking ChangeGuard::release
				assert_noop!(
					PoolFees::apply_new_fee(RuntimeOrigin::signed(ANY), POOL, CHANGE_ID),
					ERR_CHANGE_GUARD_RELEASE
				);
			})
		}

		#[test]
		fn apply_new_fee_missing_pool() {
			ExtBuilder::default().build().execute_with(|| {
				MockPools::mock_pool_exists(|_| false);
				// Requires mocking ChangeGuard::release
				assert_noop!(
					PoolFees::apply_new_fee(RuntimeOrigin::signed(ANY), POOL, CHANGE_ID),
					Error::<Runtime>::PoolNotFound
				);
			})
		}

		#[test]
		fn remove_fee_wrong_origin() {
			ExtBuilder::default().build().execute_with(|| {
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
			ExtBuilder::default().build().execute_with(|| {
				assert_noop!(
					PoolFees::remove_fee(RuntimeOrigin::signed(EDITOR), 1),
					Error::<Runtime>::FeeNotFound
				);
			})
		}

		#[test]
		fn charge_fee_wrong_origin() {
			ExtBuilder::default().build().execute_with(|| {
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
			ExtBuilder::default().build().execute_with(|| {
				assert_noop!(
					PoolFees::charge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1000),
					Error::<Runtime>::FeeNotFound
				);
			})
		}

		#[test]
		fn charge_fee_overflow() {
			ExtBuilder::default().build().execute_with(|| {
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
			ExtBuilder::default().build().execute_with(|| {
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
			ExtBuilder::default().build().execute_with(|| {
				assert_noop!(
					PoolFees::uncharge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1000),
					Error::<Runtime>::FeeNotFound
				);
			})
		}

		#[test]
		fn uncharge_fee_overflow() {
			ExtBuilder::default().build().execute_with(|| {
				add_fees(vec![default_chargeable_fee()]);

				assert_noop!(
					PoolFees::uncharge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1),
					DispatchError::Arithmetic(ArithmeticError::Underflow)
				);
			})
		}

		#[test]
		fn cannot_charge_fixed() {
			ExtBuilder::default().build().execute_with(|| {
				add_fees(vec![default_fixed_fee()]);

				assert_noop!(
					PoolFees::charge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1),
					Error::<Runtime>::CannotBeCharged
				);
			});
		}

		#[test]
		fn cannot_uncharge_fixed() {
			ExtBuilder::default().build().execute_with(|| {
				add_fees(vec![default_fixed_fee()]);

				assert_noop!(
					PoolFees::uncharge_fee(RuntimeOrigin::signed(DESTINATION), 1, 1),
					Error::<Runtime>::CannotBeCharged
				);
			});
		}

		#[test]
		fn max_fees_per_bucket() {
			ExtBuilder::default().build().execute_with(|| {
				while (ActiveFees::<Runtime>::get(POOL, BUCKET).len() as u32)
					< MaxPoolFeesPerBucket::get()
				{
					add_fees(vec![default_fixed_fee()]);
				}
				MockChangeGuard::mock_released(|_, _| {
					Ok(Change::AppendFee(u64::MAX, BUCKET, default_fixed_fee()))
				});

				assert_noop!(
					PoolFees::apply_new_fee(RuntimeOrigin::signed(ANY), POOL, CHANGE_ID),
					Error::<Runtime>::MaxPoolFeesPerBucket
				);
			});
		}

		#[test]
		fn fee_id_already_exists() {
			ExtBuilder::default().build().execute_with(|| {
				add_fees(vec![default_fixed_fee()]);

				MockChangeGuard::mock_released(|_, _| {
					Ok(Change::AppendFee(1u64, BUCKET, default_fixed_fee()))
				});

				assert_noop!(
					PoolFees::apply_new_fee(RuntimeOrigin::signed(ANY), POOL, CHANGE_ID),
					Error::<Runtime>::FeeIdAlreadyExists
				);
			});
		}
	}
}

mod disbursements {
	use cfg_primitives::SECONDS_PER_YEAR;
	use cfg_traits::{EpochTransitionHook, PoolNAV, TimeAsSecs};
	use cfg_types::{
		fixed_point::Rate,
		pools::{PoolFeeAmount, PoolFeeType},
	};
	use frame_support::traits::fungibles::Inspect;

	use super::*;
	use crate::mock::{
		get_disbursements, pay_single_fee_and_assert, MockTime, NAV, POOL_CURRENCY, SECONDS,
	};

	mod single_fee {
		use super::*;

		mod fixed {
			use super::*;

			mod share_of_portfolio_valuation {
				use super::*;

				#[test]
				fn sufficient_reserve_sfs() {
					ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
						MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

						let fee_id = 1;
						let res_pre_fees = NAV;
						let res_post_fees = &mut res_pre_fees.clone();
						let annual_rate = Rate::saturating_from_rational(1, 10);
						let fee_amount = res_pre_fees / 10;

						let fee = new_fee(PoolFeeType::Fixed {
							limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
						});
						add_fees(vec![fee.clone()]);

						// Fees (10% of NAV) consume 10% of reserve
						assert_ok!(PoolFees::on_closing_mutate_reserve(
							POOL,
							NAV + 100,
							res_post_fees
						));
						assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

						assert_eq!(*res_post_fees, res_pre_fees - fee_amount);
						assert_eq!(get_disbursements(), vec![fee_amount]);
						assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));

						pay_single_fee_and_assert(fee_id, fee_amount);
					});
				}

				#[test]
				fn insufficient_reserve_sfs() {
					ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
						MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

						let fee_id = 1;
						let res_pre_fees = NAV / 100;
						let res_post_fees = &mut res_pre_fees.clone();
						let annual_rate = Rate::saturating_from_rational(1, 10);

						let fee = new_fee(PoolFeeType::Fixed {
							limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
						});
						add_fees(vec![fee.clone()]);

						// Fees (10% of NAV) consume entire reserve
						assert_ok!(PoolFees::on_closing_mutate_reserve(
							POOL,
							NAV + 100,
							res_post_fees
						));
						assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

						assert_eq!(*res_post_fees, 0);
						assert_eq!(get_disbursements(), vec![res_pre_fees]);
						assert_eq!(
							PoolFees::nav(POOL),
							Some((NAV / 10 - res_pre_fees, MockTime::now()))
						);

						pay_single_fee_and_assert(fee_id, res_pre_fees);
					});
				}
			}

			mod amount_per_second {
				use super::*;
				#[test]
				fn sufficient_reserve_sfa() {
					ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
						MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

						let fee_id = 1;
						let res_pre_fees: Balance = (2 * SECONDS_PER_YEAR).into();
						let res_post_fees = &mut res_pre_fees.clone();
						let amount_per_second = 1;
						let fee_amount = SECONDS_PER_YEAR.into();

						let fee = new_fee(PoolFeeType::Fixed {
							limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
						});
						add_fees(vec![fee.clone()]);

						// Fees (10% of NAV) consume 10% of reserve
						assert_ok!(PoolFees::on_closing_mutate_reserve(
							POOL,
							NAV + 100,
							res_post_fees
						));
						assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

						assert_eq!(*res_post_fees, res_pre_fees - fee_amount);
						assert_eq!(get_disbursements(), vec![fee_amount]);
						assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));

						pay_single_fee_and_assert(fee_id, fee_amount);
					});
				}

				#[test]
				fn insufficient_reserve_sfa() {
					ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
						MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

						let fee_id = 1;
						let res_pre_fees: Balance = (SECONDS_PER_YEAR / 2).into();
						let res_post_fees = &mut res_pre_fees.clone();
						let amount_per_second = 1;

						let fee = new_fee(PoolFeeType::Fixed {
							limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
						});
						add_fees(vec![fee.clone()]);

						// Fees (10% of NAV) consume entire reserve
						assert_ok!(PoolFees::on_closing_mutate_reserve(
							POOL,
							NAV + 100,
							res_post_fees
						));
						assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

						assert_eq!(*res_post_fees, 0);
						assert_eq!(get_disbursements(), vec![res_pre_fees]);
						assert_eq!(PoolFees::nav(POOL), Some((res_pre_fees, MockTime::now())));

						pay_single_fee_and_assert(fee_id, res_pre_fees);
					});
				}
			}

			#[test]
			fn no_disbursement_without_prep_sfa() {
				ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
					add_fees(vec![default_fixed_fee()]);

					assert_eq!(OrmlTokens::balance(POOL_CURRENCY, &DESTINATION), 0);
					assert_ok!(PoolFees::on_execution_pre_fulfillments(POOL));
					assert_eq!(OrmlTokens::balance(POOL_CURRENCY, &DESTINATION), 0);
				});
			}
		}

		mod charged_up_to {
			use super::*;
			use crate::mock::{default_chargeable_fee, MockPools};

			mod fixed {

				use super::*;

				mod share_of_portfolio {
					use super::*;
					use crate::mock::assert_pending_fee;
					#[test]
					fn empty_charge_scfs() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV;
							let res_post_fees = &mut res_pre_fees.clone();
							let annual_rate = Rate::saturating_from_rational(1, 10);

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::ShareOfPortfolioValuation(annual_rate),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, res_pre_fees);
							assert_eq!(get_disbursements().into_iter().sum::<Balance>(), 0);
							assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));

							pay_single_fee_and_assert(fee_id, 0);
						});
					}

					#[test]
					fn below_max_charge_sufficient_reserve_scfs() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV;
							let res_post_fees = &mut res_pre_fees.clone();
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

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, res_pre_fees - charged_amount);
							assert_eq!(get_disbursements(), vec![charged_amount]);
							assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));

							pay_single_fee_and_assert(fee_id, charged_amount);
						});
					}

					#[test]
					fn max_charge_sufficient_reserve_scfs() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV;
							let res_post_fees = &mut res_pre_fees.clone();
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

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, res_pre_fees - charged_amount);
							assert_eq!(get_disbursements(), vec![charged_amount]);
							assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));

							pay_single_fee_and_assert(fee_id, charged_amount);
						});
					}

					#[test]
					fn excess_charge_sufficient_reserve_scfs() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV;
							let res_post_fees = &mut res_pre_fees.clone();
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

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, res_pre_fees - max_chargeable_amount);
							assert_eq!(get_disbursements(), vec![max_chargeable_amount]);
							assert_pending_fee(fee_id, fee.clone(), 1, 0, max_chargeable_amount);
							assert_eq!(PoolFees::nav(POOL), Some((1, MockTime::now())));

							pay_single_fee_and_assert(fee_id, max_chargeable_amount);
						});
					}

					#[test]
					fn insufficient_reserve_scfs() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV / 100;
							let res_post_fees = &mut res_pre_fees.clone();
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

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, 0);
							assert_eq!(get_disbursements(), vec![fee_amount]);
							assert_pending_fee(
								fee_id,
								fee.clone(),
								charged_amount - fee_amount,
								charged_amount - fee_amount,
								fee_amount,
							);
							assert_eq!(
								PoolFees::nav(POOL),
								Some((charged_amount - fee_amount, MockTime::now()))
							);

							pay_single_fee_and_assert(fee_id, fee_amount);
						});
					}
				}

				mod amount_per_second {
					use cfg_traits::EpochTransitionHook;

					use super::*;
					use crate::mock::assert_pending_fee;

					#[test]
					fn empty_charge_scfa() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV;
							let res_post_fees = &mut res_pre_fees.clone();
							let amount_per_second = 1;

							let fee = new_fee(PoolFeeType::ChargedUpTo {
								limit: PoolFeeAmount::AmountPerSecond(amount_per_second),
							});
							add_fees(vec![fee.clone()]);

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, res_pre_fees);
							assert_eq!(get_disbursements().into_iter().sum::<Balance>(), 0);
							assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));

							pay_single_fee_and_assert(fee_id, 0);
						});
					}

					#[test]
					fn below_max_charge_sufficient_reserve_scfa() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV;
							let res_post_fees = &mut res_pre_fees.clone();
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

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, res_pre_fees - charged_amount);
							assert_eq!(get_disbursements(), vec![charged_amount]);
							assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));

							pay_single_fee_and_assert(fee_id, charged_amount);
						});
					}

					#[test]
					fn max_charge_sufficient_reserve_scfa() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV;
							let res_post_fees = &mut res_pre_fees.clone();
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

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, res_pre_fees - charged_amount);
							assert_eq!(get_disbursements(), vec![charged_amount]);
							assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));

							pay_single_fee_and_assert(fee_id, charged_amount);
						});
					}

					#[test]
					fn excess_charge_sufficient_reserve_scfa() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let res_pre_fees = NAV;
							let res_post_fees = &mut res_pre_fees.clone();
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

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, res_pre_fees - max_chargeable_amount);
							assert_eq!(get_disbursements(), vec![max_chargeable_amount]);
							assert_pending_fee(fee_id, fee.clone(), 1, 0, max_chargeable_amount);
							assert_eq!(PoolFees::nav(POOL), Some((1, MockTime::now())));

							pay_single_fee_and_assert(fee_id, max_chargeable_amount);
						});
					}

					#[test]
					fn insufficient_reserve_scfa() {
						ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
							MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

							let fee_id = 1;
							let amount_per_second = 1;
							let res_pre_fees: Balance = (SECONDS_PER_YEAR / 2 + 1).into();
							let res_post_fees = &mut res_pre_fees.clone();
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

							assert_ok!(PoolFees::on_closing_mutate_reserve(
								POOL,
								NAV + 100,
								res_post_fees
							));
							assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV + 100);

							assert_eq!(*res_post_fees, 0);
							assert_eq!(get_disbursements(), vec![fee_amount]);
							assert_pending_fee(
								fee_id,
								fee.clone(),
								charged_amount - fee_amount,
								charged_amount - fee_amount,
								fee_amount,
							);
							assert_eq!(
								PoolFees::nav(POOL),
								Some((charged_amount - fee_amount, MockTime::now()))
							);

							pay_single_fee_and_assert(fee_id, fee_amount);
						});
					}
				}
			}

			#[test]
			fn no_disbursement_without_prep_scfa() {
				ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
					add_fees(vec![default_chargeable_fee()]);
					assert_ok!(PoolFees::charge_fee(
						RuntimeOrigin::signed(DESTINATION),
						1,
						1000
					));

					assert_eq!(OrmlTokens::balance(POOL_CURRENCY, &DESTINATION), 0);
					assert_ok!(PoolFees::on_execution_pre_fulfillments(POOL));
					assert_eq!(OrmlTokens::balance(POOL_CURRENCY, &DESTINATION), 0);
				});
			}

			#[test]
			fn update_nav_pool_missing() {
				ExtBuilder::default().build().execute_with(|| {
					MockPools::mock_pool_exists(|_| false);
					assert_noop!(
						PoolFees::update_portfolio_valuation(RuntimeOrigin::signed(ANY), POOL),
						Error::<Runtime>::PoolNotFound
					);
				});
			}
		}
	}

	mod nav {
		use cfg_types::portfolio::PortfolioValuationUpdateType;

		use super::*;
		use crate::mock::default_chargeable_fee;

		#[test]
		fn update_empty() {
			ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
				assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));
				assert_ok!(PoolFees::update_portfolio_valuation(
					RuntimeOrigin::signed(ANY),
					POOL
				));
				assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));
				System::assert_last_event(
					Event::PortfolioValuationUpdated {
						pool_id: POOL,
						valuation: 0,
						update_type: PortfolioValuationUpdateType::Exact,
					}
					.into(),
				);
			});
		}

		#[test]
		fn update_single_fixed() {
			ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
				add_fees(vec![default_fixed_fee()]);

				assert_eq!(PoolFees::nav(POOL), Some((0, 0)));
				MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

				assert_eq!(PoolFees::nav(POOL), Some((0, 0)));
				assert_ok!(PoolFees::update_portfolio_valuation(
					RuntimeOrigin::signed(ANY),
					POOL
				));
				assert_eq!(PoolFees::nav(POOL), Some((NAV / 10, MockTime::now())));
				System::assert_last_event(
					Event::PortfolioValuationUpdated {
						pool_id: POOL,
						valuation: NAV / 10,
						update_type: PortfolioValuationUpdateType::Exact,
					}
					.into(),
				);
			});
		}

		#[test]
		fn update_single_charged() {
			ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
				add_fees(vec![default_chargeable_fee()]);
				MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

				assert_eq!(PoolFees::nav(POOL), Some((0, 0)));
				assert_ok!(PoolFees::update_portfolio_valuation(
					RuntimeOrigin::signed(ANY),
					POOL
				));
				assert_eq!(PoolFees::nav(POOL), Some((0, MockTime::now())));
				System::assert_last_event(
					Event::PortfolioValuationUpdated {
						pool_id: POOL,
						valuation: 0,
						update_type: PortfolioValuationUpdateType::Exact,
					}
					.into(),
				);
			});
		}
	}

	mod waterfall {
		use super::*;
		use crate::mock::assert_pending_fee;

		#[test]
		fn fixed_charged_charged() {
			ExtBuilder::default().set_aum(NAV).build().execute_with(|| {
				MockTime::mock_now(|| SECONDS_PER_YEAR * SECONDS);

				let charged_fee_ids = vec![2, 3];
				let res_pre_fees = NAV;
				let res_post_fees = &mut res_pre_fees.clone();
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

				assert_ok!(PoolFees::on_closing_mutate_reserve(
					POOL,
					NAV,
					res_post_fees
				));
				assert_eq!(AssetsUnderManagement::<Runtime>::get(POOL), NAV);
				assert_eq!(
					*res_post_fees,
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
				assert_eq!(PoolFees::nav(POOL), Some((payable[1], MockTime::now())));

				// Pay disbursements
				assert_ok!(PoolFees::on_execution_pre_fulfillments(POOL));
				assert_eq!(get_disbursements().into_iter().sum::<Balance>(), 0);
				assert_eq!(PoolFees::nav(POOL), Some((payable[1], MockTime::now())));
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
				MockTime::mock_now(|| 2 * SECONDS_PER_YEAR * SECONDS);
				let res_pre_fees = fixed_fee_amount + charged_y2[0] + 1;
				let res_post_fees = &mut res_pre_fees.clone();
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
				assert_ok!(PoolFees::on_closing_mutate_reserve(
					POOL,
					NAV,
					res_post_fees
				));
				assert_eq!(*res_post_fees, 0);
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
				assert_eq!(
					PoolFees::nav(POOL),
					Some((2 * payable[1] - 1, MockTime::now()))
				);

				// Pay disbursements
				assert_ok!(PoolFees::on_execution_pre_fulfillments(POOL));
				assert_eq!(get_disbursements().into_iter().sum::<Balance>(), 0);
				assert_eq!(
					PoolFees::nav(POOL),
					Some((2 * payable[1] - 1, MockTime::now()))
				);
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
	}
}
