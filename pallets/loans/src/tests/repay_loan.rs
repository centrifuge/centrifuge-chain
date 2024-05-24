use sp_arithmetic::traits::Saturating;

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
		match *id {
			REGISTER_PRICE_ID => Ok((price, BLOCK_TIME_MS)),
			_ => Err(PRICE_ID_NO_FOUND),
		}
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
				RepaidInput {
					principal: PrincipalInput::Internal(COLLATERAL_VALUE),
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
				RepaidInput {
					principal: PrincipalInput::Internal(COLLATERAL_VALUE),
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(OTHER_BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::Internal(COLLATERAL_VALUE),
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);
		util::write_off_loan(loan_id);

		config_mocks(util::current_loan_debt(loan_id));
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE),
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks(0);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::External(ExternalAmount::empty()),
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
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		config_mocks(0);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::Internal(0),
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE / 2));

		let amount = RepaidInput {
			principal: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
			interest: 1234, /* Will not be used */
			unscheduled: 0,
		};

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			amount.clone()
		));
		assert_eq!(0, util::current_loan_debt(loan_id));

		System::assert_last_event(RuntimeEvent::Loans(Event::Repaid {
			pool_id: POOL_A,
			loan_id: loan_id,
			amount: RepaidInput {
				interest: 0,
				..amount
			},
		}));
	});
}

#[test]
fn with_success_total_amount() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE),
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE);

		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::Internal(COLLATERAL_VALUE * 2),
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
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE),
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
				RepaidInput {
					principal: PrincipalInput::Internal(1), // All was already repaid
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
			RepaidInput {
				principal: PrincipalInput::Internal(0),
				interest: u128::MAX, //Discarded
				unscheduled: 0,
			},
		));
	});
}

#[test]
fn with_more_than_required_external() {
	new_test_ext().execute_with(|| {
		let variation = Rate::one();
		let mut pricing = util::base_external_pricing();
		pricing.max_price_variation = variation;
		let mut info = util::base_external_loan();
		info.pricing = Pricing::External(pricing);

		let loan_id = util::create_loan(info);
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		let amount = ExternalAmount::new(
			QUANTITY.saturating_mul(Quantity::from_rational(2, 1)),
			PRICE_VALUE + variation.checked_mul_int(PRICE_VALUE).unwrap(),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);

		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::External(amount),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::from(RepayLoanError::MaxPrincipalAmountExceeded)
		);

		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE * 2);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount.clone()),
				interest: 0,
				unscheduled: 0,
			},
		));

		config_mocks_with_price(0, PRICE_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::External(amount),
					interest: 0,
					unscheduled: 0,
				}
			),
			Error::<Runtime>::from(RepayLoanError::MaxPrincipalAmountExceeded)
		);

		MockPrices::mock_unregister_id(move |id, pool_id| {
			assert_eq!(*pool_id, POOL_A);
			assert_eq!(*id, REGISTER_PRICE_ID);
			Ok(())
		});

		assert_ok!(Loans::close(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE / 2);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
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
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE),
				interest: 0,
				unscheduled: 0,
			},
		));

		config_mocks(0);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::Internal(0),
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
				interest: 0,
				unscheduled: 0,
			},
		));
		assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_id));

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
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
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		let amount = ExternalAmount::new(QUANTITY / 2.into(), PRICE_VALUE);
		config_mocks(amount.balance().unwrap());
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount),
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
			RepaidInput {
				principal: PrincipalInput::External(remaining),
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks(COLLATERAL_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
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
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
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
			RepaidInput {
				principal: PrincipalInput::Internal(0),
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
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		let amount = ExternalAmount::new(QUANTITY / 2.into(), PRICE_VALUE);
		config_mocks(amount.balance().unwrap());
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount),
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
			RepaidInput {
				principal: PrincipalInput::External(remaining),
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
			RepaidInput {
				principal: PrincipalInput::External(ExternalAmount::empty()),
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
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		advance_time(YEAR / 2);

		config_mocks(util::current_loan_debt(loan_id));
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE),
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
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE * 2);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE * 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount),
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
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE / 2);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE / 2);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn with_unscheduled_repayment_internal() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks(1234);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::Internal(0),
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
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		config_mocks(1234);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(ExternalAmount::empty()),
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
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		// Much higher
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE + PRICE_VALUE + 1);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::External(amount),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::SettlementPriceExceedsVariation
		);

		// Higher
		let amount = ExternalAmount::new(
			QUANTITY,
			PRICE_VALUE + (MAX_PRICE_VARIATION.saturating_mul_int(PRICE_VALUE) + 1),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::External(amount),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::SettlementPriceExceedsVariation
		);

		// Lower
		let amount = ExternalAmount::new(
			QUANTITY,
			PRICE_VALUE - (MAX_PRICE_VARIATION.saturating_mul_int(PRICE_VALUE) + 1),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_noop!(
			Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				RepaidInput {
					principal: PrincipalInput::External(amount),
					interest: 0,
					unscheduled: 0,
				},
			),
			Error::<Runtime>::SettlementPriceExceedsVariation
		);
	});
}

#[test]
fn with_correct_settlement_price_external_pricing() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		// Higher
		let amount = ExternalAmount::new(
			QUANTITY / 3.into(),
			PRICE_VALUE + MAX_PRICE_VARIATION.saturating_mul_int(PRICE_VALUE),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(
			(QUANTITY / 3.into()).saturating_mul_int(PRICE_VALUE) * 2,
			util::current_loan_pv(loan_id)
		);

		// Same
		let amount = ExternalAmount::new(QUANTITY / 3.into(), PRICE_VALUE);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(
			(QUANTITY / 3.into()).saturating_mul_int(PRICE_VALUE),
			util::current_loan_pv(loan_id)
		);

		// Lower
		let amount = ExternalAmount::new(
			QUANTITY / 3.into(),
			PRICE_VALUE - MAX_PRICE_VARIATION.saturating_mul_int(PRICE_VALUE),
		);
		config_mocks_with_price(amount.balance().unwrap(), PRICE_VALUE);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(0, util::current_loan_pv(loan_id));
		assert_eq!(0, util::current_loan_debt(loan_id));
	});
}

#[test]
fn with_unregister_price_id_and_oracle_not_required() {
	new_test_ext().execute_with(|| {
		let loan = LoanInfo {
			pricing: Pricing::External(ExternalPricing {
				price_id: UNREGISTER_PRICE_ID,
				..util::base_external_pricing()
			}),
			..util::base_external_loan()
		};

		let loan_id = util::create_loan(loan);
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		let amount = ExternalAmount::new(QUANTITY / 2.into(), PRICE_VALUE * 2);
		config_mocks_with_price(amount.balance().unwrap(), 0 /* unused */);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			RepaidInput {
				principal: PrincipalInput::External(amount),
				interest: 0,
				unscheduled: 0,
			},
		));

		assert_eq!(
			(QUANTITY / 2.into()).saturating_mul_int(PRICE_VALUE * 2),
			util::current_loan_pv(loan_id)
		);
	});
}

#[test]
fn with_external_pricing() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(LoanInfo {
			pricing: Pricing::External(ExternalPricing {
				price_id: UNREGISTER_PRICE_ID,
				..util::base_external_pricing()
			}),
			..util::base_external_loan()
		});

		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PrincipalInput::External(amount));

		let amount = ExternalAmount::new(Quantity::one(), PRICE_VALUE);
		config_mocks(amount.balance().unwrap());

		let repay_amount = RepaidInput {
			principal: PrincipalInput::External(amount),
			interest: 0,
			unscheduled: 0,
		};

		let current_price = || {
			ActiveLoanInfo::try_from((POOL_A, util::get_loan(loan_id)))
				.unwrap()
				.current_price
				.unwrap()
		};

		// Repay and check time without advance time
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			repay_amount.clone()
		));
		assert_eq!(current_price(), PRICE_VALUE);

		// In the middle of the line
		advance_time(YEAR / 2);
		assert_eq!(current_price(), PRICE_VALUE + (NOTIONAL - PRICE_VALUE) / 2);

		// BEFORE: the loan not yet overdue
		advance_time(YEAR / 2 - DAY);
		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			repay_amount.clone()
		));
		assert!(current_price() < NOTIONAL);

		// EXACT: the loan is just at matuyrity date
		advance_time(DAY);

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			repay_amount.clone()
		));
		assert_eq!(current_price(), NOTIONAL);

		// AFTER: the loan overpassing maturity date
		advance_time(DAY);

		assert_ok!(Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			repay_amount.clone()
		));
		assert_eq!(current_price(), NOTIONAL);
	});
}
