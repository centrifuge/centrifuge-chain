use super::*;

pub fn config_mocks(deposit_amount: Balance) {
	config_mocks_with_price(deposit_amount, PRICE_VALUE)
}

pub fn config_mocks_with_price(deposit_amount: Balance, price: Balance) {
	MockPools::mock_deposit(move |pool_id, to, amount| {
		assert_eq!(to, BORROWER);
		assert_eq!(pool_id, POOL_A);
		assert_eq!(deposit_amount, amount);
		Ok(())
	});
	MockPrices::mock_get(move |id, pool_id| {
		assert_eq!(*pool_id, POOL_A);
		assert_eq!(*id, REGISTER_PRICE_ID);
		Ok((price, BLOCK_TIME.as_secs()))
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
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE),
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
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE),
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
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(OTHER_BORROWER),
				POOL_A,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE),
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
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);
		util::write_off_loan(loan_id);

		config_mocks(util::current_loan_debt(loan_id));
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE),
				interest: u128::MAX,
				unscheduled: 0,
			},
		));
	});
}

#[test]
fn with_wrong_external_pricing() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks(0);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::External(ExternalAmount::empty()),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::MismatchedPricingMethod
		);
	});
}

#[test]
fn with_wrong_internal_pricing() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		config_mocks(0);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(0),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::MismatchedPricingMethod
		);
	});
}

#[test]
fn with_success_half_amount() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE / 2));

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE / 2),
				interest: 0,
				unscheduled: 0,
			},
		));
		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn with_success_total_amount() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE),
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
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE);

		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE * 2),
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
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE),
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
				RepaidPricingAmount {
					principal: PricingAmount::Internal(1), // All was already repaid
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
			RepaidPricingAmount {
				principal: PricingAmount::Internal(0),
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
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE / 2);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::Internal(COLLATERAL_VALUE / 2),
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
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE),
				interest: 0,
				unscheduled: 0,
			},
		));

		config_mocks(0);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(0),
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
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE / 2),
				interest: 0,
				unscheduled: 0,
			},
		));
		assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_id));

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE / 2),
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
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		let amount = ExternalAmount::new(QUANTITY / 2.into(), PRICE_VALUE);
		config_mocks(amount.balance().unwrap());
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(
			(QUANTITY / 2.into()).saturating_mul_int(NOTIONAL),
			util::current_loan_debt(loan_id)
		);

		let remaining = ExternalAmount::new(QUANTITY / 2.into(), PRICE_VALUE);
		config_mocks(remaining.balance().unwrap());

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(remaining),
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
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE / 2),
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
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE / 2),
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
			RepaidPricingAmount {
				principal: PricingAmount::Internal(0),
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
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		let amount = ExternalAmount::new(QUANTITY / 2.into(), PRICE_VALUE);
		config_mocks(amount.balance().unwrap());
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		advance_time(YEAR / 2);

		assert_eq!(
			util::current_debt_for(
				util::interest_for(DEFAULT_INTEREST_RATE, YEAR / 2),
				(QUANTITY / 2.into()).saturating_mul_int(NOTIONAL),
			),
			util::current_loan_debt(loan_id)
		);

		let remaining = ExternalAmount::new(QUANTITY / 2.into(), PRICE_VALUE);
		config_mocks(remaining.balance().unwrap());

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(remaining),
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
			RepaidPricingAmount {
				principal: PricingAmount::External(ExternalAmount::empty()),
				interest: still_to_pay,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn current_debt_rate_no_increase_if_fully_repaid() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(LoanInfo {
			pricing: Pricing::Internal(InternalPricing {
				max_borrow_amount: util::outstanding_debt_rate(1.0),
				..util::base_internal_pricing()
			}),
			..util::base_internal_loan()
		});
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR / 2);

		config_mocks(util::current_loan_debt(loan_id));
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(COLLATERAL_VALUE),
				interest: u128::MAX,
				unscheduled: 0,
			},
		));

		advance_time(YEAR);

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn external_pricing_goes_up() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE * 2);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE * 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(amount),
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
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE / 2);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(amount),
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
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		let amount = ExternalAmount::new(Quantity::from_float(0.5), PRICE_VALUE);
		config_mocks(amount.balance().unwrap());
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::External(amount),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::AmountNotNaturalNumber
		);
	});
}

#[test]
fn with_unscheduled_repayment_internal() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks(1234);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::Internal(0),
				interest: 0,
				unscheduled: 1234,
			},
		));

		// Nothing repaid with unscheduled amount,
		// so I still have the whole amount as debt
		assert_eq!(COLLATERAL_VALUE, util::current_loan_debt(loan_id));
	});
}

#[test]
fn with_unscheduled_repayment_external() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		config_mocks(1234);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(ExternalAmount::empty()),
				interest: 0,
				unscheduled: 1234,
			},
		));

		// Nothing repaid with unscheduled amount,
		// so I still have the whole amount as debt
		assert_eq!(
			(QUANTITY).saturating_mul_int(NOTIONAL),
			util::current_loan_debt(loan_id)
		);
	});
}

#[test]
fn with_incorrect_settlement_price_external_pricing() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		// Higher
		let amount = ExternalAmount::new(
			QUANTITY,
			PRICE_VALUE + (MAX_VARIATION_PRICE.mul_floor(PRICE_VALUE) + 1),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::External(amount),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::SettlementPriceExceedsSlippage
		);

		// Lower
		let amount = ExternalAmount::new(
			QUANTITY,
			PRICE_VALUE - (MAX_VARIATION_PRICE.mul_floor(PRICE_VALUE) + 1),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidPricingAmount {
					principal: PricingAmount::External(amount),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::SettlementPriceExceedsSlippage
		);
	});
}

#[test]
fn with_correct_settlement_price_external_pricing() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		// Higher
		let amount = ExternalAmount::new(
			QUANTITY / 3.into(),
			PRICE_VALUE + MAX_VARIATION_PRICE.mul_floor(PRICE_VALUE),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		// Same
		let amount = ExternalAmount::new(QUANTITY / 3.into(), PRICE_VALUE);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		// Lower
		let amount = ExternalAmount::new(
			QUANTITY / 3.into(),
			PRICE_VALUE - MAX_VARIATION_PRICE.mul_floor(PRICE_VALUE),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidPricingAmount {
				principal: PricingAmount::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}
