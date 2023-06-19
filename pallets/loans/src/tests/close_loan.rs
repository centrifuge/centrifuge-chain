use super::*;

fn config_mocks() {
	MockPrices::mock_unregister_id(|_, _| Ok(()));
}

#[test]
fn with_wrong_loan_id() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Loans::close(RuntimeOrigin::signed(BORROWER), POOL_A, 0),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);
	});
}

#[test]
fn with_wrong_borrower() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());

		assert_noop!(
			Loans::close(RuntimeOrigin::signed(OTHER_BORROWER), POOL_A, loan_id),
			Error::<Runtime>::NotLoanBorrower
		);

		// Make the loan active and ready to be closed
		util::borrow_loan(loan_id, COLLATERAL_VALUE);
		util::repay_loan(loan_id, COLLATERAL_VALUE);

		assert_noop!(
			Loans::close(RuntimeOrigin::signed(OTHER_BORROWER), POOL_A, loan_id),
			Error::<Runtime>::NotLoanBorrower
		);
	});
}

#[test]
fn without_fully_repaid_internal() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);
		util::repay_loan(loan_id, COLLATERAL_VALUE / 2);

		assert_noop!(
			Loans::close(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id),
			Error::<Runtime>::from(CloseLoanError::NotFullyRepaid)
		);
	});
}

#[test]
fn without_fully_repaid_external() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);
		util::repay_loan(loan_id, amount / 2);

		assert_noop!(
			Loans::close(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id),
			Error::<Runtime>::from(CloseLoanError::NotFullyRepaid)
		);
	});
}

#[test]
fn with_time_after_fully_repaid_internal() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);
		util::repay_loan(loan_id, COLLATERAL_VALUE);

		advance_time(YEAR);

		assert_ok!(Loans::close(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id
		));

		assert_eq!(Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(), BORROWER);
	});
}

#[test]
fn with_fully_repaid_internal() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);
		util::repay_loan(loan_id, COLLATERAL_VALUE);

		assert_ok!(Loans::close(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id
		));

		assert_eq!(Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(), BORROWER);
	});
}

#[test]
fn with_fully_repaid_external() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);
		util::repay_loan(loan_id, amount);

		config_mocks();
		assert_ok!(Loans::close(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id
		));

		assert_eq!(Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(), BORROWER);
	});
}

#[test]
fn just_created() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());

		assert_ok!(Loans::close(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id
		));

		assert_eq!(Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(), BORROWER);
	});
}
