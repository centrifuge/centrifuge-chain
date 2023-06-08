
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
				COLLATERAL_VALUE,
				0,
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
				COLLATERAL_VALUE,
				0
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
				COLLATERAL_VALUE,
				0
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

		config_mocks(COLLATERAL_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			COLLATERAL_VALUE,
			0
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
			COLLATERAL_VALUE / 2,
			0
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
			COLLATERAL_VALUE,
			0
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
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			COLLATERAL_VALUE * 2,
			0
		));
	});
}

#[test]
fn with_restriction_full_once() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(LoanInfo {
			restrictions: LoanRestrictions {
				borrows: BorrowRestrictions::FullOnce,
				repayments: RepayRestrictions::FullOnce,
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
				COLLATERAL_VALUE / 2,
				0
			),
			Error::<Runtime>::from(RepayLoanError::Restriction) // Full amount
		);

		config_mocks(COLLATERAL_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			COLLATERAL_VALUE,
			0
		));

		let extra = 1;
		config_mocks(0);
		assert_noop!(
			Loans::repay(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, extra, 0),
			Error::<Runtime>::from(RepayLoanError::Restriction) // Only once
		);
	});
}

#[test]
fn twice() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			COLLATERAL_VALUE / 2,
			0
		));
		assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_id));

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			COLLATERAL_VALUE / 2,
			0
		));
		assert_eq!(0, util::current_loan_debt(loan_id));

		// At this point the loan has been fully repaid.
		let extra = 1;
		config_mocks(0);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			extra,
			0
		));
	});
}

#[test]
fn twice_with_elapsed_time() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			COLLATERAL_VALUE / 2,
			0
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
			COLLATERAL_VALUE / 2,
			0
		));

		// Because of the interest, it has no fully repaid, we need an extra payment.
		let still_to_pay = util::current_loan_debt(loan_id);
		assert_ne!(0, still_to_pay);

		config_mocks(still_to_pay);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			still_to_pay,
			0
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

		config_mocks(COLLATERAL_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			COLLATERAL_VALUE,
			0
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
			amount,
			0
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
			amount * 2,
			0
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

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			amount,
			0
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
				amount - 1,
				0
			),
			Error::<Runtime>::AmountNotMultipleOfPrice
		);
	});
}

#[test]
fn with_unchecked_repayment() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		config_mocks(COLLATERAL_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			0,
			COLLATERAL_VALUE,
		),);

		// Nothing repaid with unchecked amount,
		// so I still have the whole amount as debt
		assert_eq!(COLLATERAL_VALUE, util::current_loan_debt(loan_id));
	});
}
