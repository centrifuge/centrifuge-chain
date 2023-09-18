use super::*;

fn config_mocks(
	transfer: (
		LoanId,
		LoanId,
		RepaidPricingAmount<Runtime>,
		PricingAmount<Runtime>,
	),
) {
	MockPrices::mock_get(|id, pool_id| {
		assert_eq!(*id, REGISTER_PRICE_ID);
		assert_eq!(*pool_id, POOL_A);
		Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
	});
	MockPrices::mock_register_id(|id, pool_id| {
		assert_eq!(*pool_id, POOL_A);
		assert_eq!(*id, REGISTER_PRICE_ID);
		Ok(())
	});
	MockChangeGuard::mock_note({
		let (loan_1, loan_2, repay, borrow) = transfer.clone();
		move |pool_id, change| {
			assert_eq!(pool_id, POOL_A);
			assert_eq!(
				change,
				Change::TransferDebt(loan_1, loan_2, repay.clone(), borrow.clone())
			);
			Ok(CHANGE_ID)
		}
	});
	MockChangeGuard::mock_released({
		let (loan_1, loan_2, repay, borrow) = transfer.clone();
		move |pool_id, change_id| {
			assert_eq!(pool_id, POOL_A);
			assert_eq!(change_id, CHANGE_ID);
			Ok(Change::TransferDebt(
				loan_1,
				loan_2,
				repay.clone(),
				borrow.clone(),
			))
		}
	});
}

#[test]
fn with_wrong_borrower() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_1, PricingAmount::Internal(COLLATERAL_VALUE));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});

		assert_noop!(
			Loans::propose_transfer_debt(
				RuntimeOrigin::signed(OTHER_BORROWER),
				POOL_A,
				loan_1,
				loan_2,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE),
					interest: 0,
					unscheduled: 0,
				},
				PricingAmount::Internal(COLLATERAL_VALUE),
			),
			Error::<Runtime>::NotLoanBorrower
		);
	});
}

#[test]
fn with_wrong_loans() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		assert_noop!(
			Loans::propose_transfer_debt(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				0, // Does not exists
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE),
					interest: 0,
					unscheduled: 0,
				},
				PricingAmount::Internal(COLLATERAL_VALUE),
			),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);

		assert_noop!(
			Loans::propose_transfer_debt(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				0, // Does not exists
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE),
					interest: 0,
					unscheduled: 0,
				},
				PricingAmount::Internal(COLLATERAL_VALUE),
			),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);
	});
}

#[test]
fn with_same_loan() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		assert_noop!(
			Loans::propose_transfer_debt(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE),
					interest: 0,
					unscheduled: 0,
				},
				PricingAmount::Internal(COLLATERAL_VALUE),
			),
			Error::<Runtime>::TransferDebtToSameLoan
		);
	});
}

#[test]
fn with_mismatch_internal_internal_amounts() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_1, PricingAmount::Internal(COLLATERAL_VALUE));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});

		assert_noop!(
			Loans::propose_transfer_debt(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_1,
				loan_2,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE / 2),
					interest: 0,
					unscheduled: 0,
				},
				PricingAmount::Internal(COLLATERAL_VALUE / 3),
			),
			Error::<Runtime>::TransferDebtAmountMismatched
		);
	});
}

#[test]
fn with_mismatch_external_internal_amounts() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_1, PricingAmount::External(amount));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});

		let repay_amount = ExternalAmount::new(QUANTITY, PRICE_VALUE + 2);

		MockPrices::mock_get(|id, pool_id| {
			assert_eq!(*id, REGISTER_PRICE_ID);
			assert_eq!(*pool_id, POOL_A);
			Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
		});
		assert_noop!(
			Loans::propose_transfer_debt(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_1,
				loan_2,
				RepaidPricingAmount {
					principal: PricingAmount::External(repay_amount),
					interest: 0,
					unscheduled: 0,
				},
				PricingAmount::Internal(COLLATERAL_VALUE),
			),
			Error::<Runtime>::TransferDebtAmountMismatched
		);
	});
}

#[test]
fn with_mismatch_internal_external_amounts() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_1, PricingAmount::Internal(COLLATERAL_VALUE));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_external_loan()
		});

		let borrow_amount = ExternalAmount::new(QUANTITY, PRICE_VALUE * 3);

		assert_noop!(
			Loans::propose_transfer_debt(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_1,
				loan_2,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE),
					interest: 0,
					unscheduled: 0,
				},
				PricingAmount::External(borrow_amount),
			),
			Error::<Runtime>::TransferDebtAmountMismatched
		);
	});
}

#[test]
fn with_mismatch_external_external_amounts() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_1, PricingAmount::External(amount));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_external_loan()
		});

		let repay_amount = ExternalAmount::new(QUANTITY, PRICE_VALUE + 2);
		let borrow_amount = ExternalAmount::new(QUANTITY, PRICE_VALUE + 3);

		MockPrices::mock_get(|id, pool_id| {
			assert_eq!(*id, REGISTER_PRICE_ID);
			assert_eq!(*pool_id, POOL_A);
			Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
		});
		assert_noop!(
			Loans::propose_transfer_debt(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_1,
				loan_2,
				RepaidPricingAmount {
					principal: PricingAmount::External(repay_amount),
					interest: 0,
					unscheduled: 0,
				},
				PricingAmount::External(borrow_amount),
			),
			Error::<Runtime>::TransferDebtAmountMismatched
		);
	});
}

#[test]
fn apply_without_released() {
	new_test_ext().execute_with(|| {
		MockChangeGuard::mock_released(|_, _| Err("err".into()));

		assert_noop!(
			Loans::apply_transfer_debt(RuntimeOrigin::signed(ANY), POOL_A, CHANGE_ID),
			DispatchError::Other("err")
		);
	});
}

#[test]
fn with_success_internals() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_1, PricingAmount::Internal(COLLATERAL_VALUE));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});

		let repay_amount = RepaidPricingAmount {
			principal: PricingAmount::Internal(COLLATERAL_VALUE),
			interest: 0,
			unscheduled: 0,
		};
		let borrow_amount = PricingAmount::Internal(COLLATERAL_VALUE);

		config_mocks((loan_1, loan_2, repay_amount.clone(), borrow_amount.clone()));
		assert_ok!(Loans::propose_transfer_debt(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_1,
			loan_2,
			repay_amount,
			borrow_amount,
		));

		assert_ok!(Loans::apply_transfer_debt(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			CHANGE_ID,
		));

		assert_eq!(0, util::current_loan_debt(loan_1));
		assert_eq!(COLLATERAL_VALUE, util::current_loan_debt(loan_2));
	});
}

#[test]
fn with_success_externals() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_1, PricingAmount::External(amount));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_external_loan()
		});

		let repay_amount = RepaidPricingAmount {
			principal: PricingAmount::External(ExternalAmount::new(QUANTITY, PRICE_VALUE)),
			interest: 0,
			unscheduled: 0,
		};
		let borrow_amount = PricingAmount::External(ExternalAmount::new(QUANTITY, PRICE_VALUE));

		config_mocks((loan_1, loan_2, repay_amount.clone(), borrow_amount.clone()));
		assert_ok!(Loans::propose_transfer_debt(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_1,
			loan_2,
			repay_amount,
			borrow_amount,
		));

		assert_ok!(Loans::apply_transfer_debt(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			CHANGE_ID,
		));

		assert_eq!(0, util::current_loan_debt(loan_1));
		assert_eq!(
			QUANTITY.saturating_mul_int(NOTIONAL),
			util::current_loan_debt(loan_2)
		);
	});
}

#[test]
fn with_transfer_roundtrip() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_1, PricingAmount::Internal(COLLATERAL_VALUE / 2));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});

		let repay_amount = RepaidPricingAmount {
			principal: PricingAmount::Internal(COLLATERAL_VALUE / 2),
			interest: 0,
			unscheduled: 0,
		};
		let borrow_amount = PricingAmount::Internal(COLLATERAL_VALUE / 2);

		config_mocks((loan_1, loan_2, repay_amount.clone(), borrow_amount.clone()));
		assert_ok!(Loans::propose_transfer_debt(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_1,
			loan_2,
			repay_amount.clone(),
			borrow_amount.clone(),
		));

		assert_ok!(Loans::apply_transfer_debt(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			CHANGE_ID,
		));

		assert_eq!(0, util::current_loan_debt(loan_1));
		assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_2));

		config_mocks((loan_2, loan_1, repay_amount.clone(), borrow_amount.clone()));
		assert_ok!(Loans::propose_transfer_debt(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_2,
			loan_1,
			repay_amount,
			borrow_amount,
		));

		assert_ok!(Loans::apply_transfer_debt(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			CHANGE_ID,
		));

		assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_1));
		assert_eq!(0, util::current_loan_debt(loan_2));
	});
}
