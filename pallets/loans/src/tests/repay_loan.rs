use super::*;

pub fn config_mocks(deposit_amount: Balance) {
	MockPools::mock_deposit(move |pool_id, to, amount| {
		assert_eq!(to, BORROWER);
		assert_eq!(pool_id, POOL_A);
		assert_eq!(deposit_amount, amount);
		Ok(())
	});
	MockPrices::mock_get(|id| {
		assert_eq!(*id, REGISTER_PRICE_ID);
		Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
	});
}

#[test]
fn without_borrow_first() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());

		config_mocks(COLLATERAL_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidAmount {
					principal: COLLATERAL_VALUE,
					interest: u128::MAX,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);
	});
}

#[test]
fn with_wrong_loan_id() {
	new_test_ext().execute_with(|| {
		config_mocks(COLLATERAL_VALUE);

		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				0,
				RepaidAmount {
					principal: COLLATERAL_VALUE,
					interest: u128::MAX,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);
	});
}

#[test]
fn from_other_borrower() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(OTHER_BORROWER),
				POOL_A,
				loan_id,
				RepaidAmount {
					principal: COLLATERAL_VALUE,
					interest: u128::MAX,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::NotLoanBorrower
		);
	});
}

#[test]
fn has_been_written_off() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		advance_time(YEAR + DAY);
		util::write_off_loan(loan_id);

		config_mocks(util::current_loan_debt(loan_id));
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE,
				interest: u128::MAX,
				unscheduled: 0,
			},
		));
	});
}

#[test]
fn with_success_partial() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE / 2);

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE / 2,
				interest: 0,
				unscheduled: 0,
			},
		));
		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn with_success_total() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE,
				interest: 0,
				unscheduled: 0,
			},
		));
		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn with_more_than_required() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE);

		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidAmount {
					principal: COLLATERAL_VALUE * 2,
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::from(RepayLoanError::MaxPrincipalAmountExceeded)
		);

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE,
				interest: u128::MAX, // Here there is no limit
				unscheduled: 0,
			},
		));

		// At this point, it's fully repaid. It can not be repaid more
		config_mocks(0);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidAmount {
					principal: 1, // All was already repaid
					interest: 0,
					unscheduled: 0,
				}
			),
			Error::<Runtime>::from(RepayLoanError::MaxPrincipalAmountExceeded)
		);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: 0,
				interest: u128::MAX, //Discarded
				unscheduled: 0,
			},
		));
	});
}

#[test]
fn with_restriction_full_once() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(LoanInfo {
			restrictions: LoanRestrictions {
				borrows: BorrowRestrictions::FullOnce,
				repayments: RepayRestrictions::Full,
			},
			..util::base_internal_loan()
		});
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE / 2);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidAmount {
					principal: COLLATERAL_VALUE / 2,
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::from(RepayLoanError::Restriction) // Full amount
		);

		config_mocks(COLLATERAL_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE,
				interest: 0,
				unscheduled: 0,
			},
		));

		config_mocks(0);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: 0,
				interest: 0,
				unscheduled: 0,
			}
		));
	});
}

#[test]
fn twice_internal() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE / 2,
				interest: 0,
				unscheduled: 0,
			},
		));
		assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_id));

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE / 2,
				interest: 0,
				unscheduled: 0,
			},
		));
		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn twice_external() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);

		config_mocks(amount / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: amount / 2,
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(
			NOTIONAL.saturating_mul_int(QUANTITY / 2),
			util::current_loan_debt(loan_id)
		);

		let remaining = PRICE_VALUE.saturating_mul_int(QUANTITY / 2);
		config_mocks(remaining);

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: remaining,
				interest: 0,
				unscheduled: 0,
			},
		));
		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn twice_internal_with_elapsed_time() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE / 2,
				interest: 0,
				unscheduled: 0,
			},
		));

		advance_time(YEAR / 2);

		assert_eq!(
			util::current_debt_for(
				util::interest_for(DEFAULT_INTEREST_RATE, YEAR / 2),
				COLLATERAL_VALUE / 2,
			),
			util::current_loan_debt(loan_id)
		);

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE / 2,
				interest: 0,
				unscheduled: 0,
			},
		));

		// Because of the interest, it has no fully repaid, we need an extra payment.
		let still_to_pay = util::current_loan_debt(loan_id);
		assert_ne!(0, still_to_pay);

		config_mocks(still_to_pay);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: 0,
				interest: still_to_pay,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn twice_external_with_elapsed_time() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);

		config_mocks(amount / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: amount / 2,
				interest: 0,
				unscheduled: 0,
			},
		));

		advance_time(YEAR / 2);

		assert_eq!(
			util::current_debt_for(
				util::interest_for(DEFAULT_INTEREST_RATE, YEAR / 2),
				NOTIONAL.saturating_mul_int(QUANTITY / 2),
			),
			util::current_loan_debt(loan_id)
		);

		let remaining = PRICE_VALUE.saturating_mul_int(QUANTITY / 2);
		config_mocks(remaining);

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: remaining,
				interest: 0,
				unscheduled: 0,
			},
		));

		// Because of the interest, it has no fully repaid, we need an extra payment.
		let still_to_pay = util::current_loan_debt(loan_id);
		assert_ne!(0, still_to_pay);

		config_mocks(still_to_pay);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: 0,
				interest: still_to_pay,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn outstanding_debt_rate_no_increase_if_fully_repaid() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(LoanInfo {
			pricing: Pricing::Internal(InternalPricing {
				max_borrow_amount: util::outstanding_debt_rate(1.0),
				..util::base_internal_pricing()
			}),
			..util::base_internal_loan()
		});
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		advance_time(YEAR / 2);

		config_mocks(util::current_loan_debt(loan_id));
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: COLLATERAL_VALUE,
				interest: u128::MAX,
				unscheduled: 0,
			},
		));

		advance_time(YEAR);

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn external_pricing_same() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);

		config_mocks(amount);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: amount,
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn external_pricing_goes_up() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);

		config_mocks(amount * 2);
		MockPrices::mock_get(|_| Ok((PRICE_VALUE * 2.into(), now().as_secs())));

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: amount * 2,
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn external_pricing_goes_down() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);

		config_mocks(amount / 2);
		MockPrices::mock_get(|_| Ok((PRICE_VALUE / 2.into(), now().as_secs())));

		let amount = (PRICE_VALUE / 2.into()).saturating_mul_int(QUANTITY);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: amount,
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn external_pricing_with_wrong_quantity() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);

		// It's not multiple of PRICE_VALUE
		config_mocks(amount - 1);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidAmount {
					principal: amount - 1,
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::AmountNotMultipleOfPrice
		);
	});
}

#[test]
fn with_unscheduled_repayment() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(1234);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidAmount {
				principal: 0,
				interest: 0,
				unscheduled: 1234,
			},
		));

		// Nothing repaid with unscheduled amount,
		// so I still have the whole amount as debt
		assert_eq!(COLLATERAL_VALUE, util::current_loan_debt(loan_id));
	});
}
