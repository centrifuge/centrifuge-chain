use super::*;

fn config_mocks() {
	MockPools::mock_pool_exists(|pool_id| pool_id == POOL_A);
	MockPrices::mock_get(move |id, pool_id| {
		assert_eq!(*pool_id, POOL_A);
		match *id {
			REGISTER_PRICE_ID => Ok((PRICE_VALUE, BLOCK_TIME_MS)),
			_ => Err(PRICE_ID_NO_FOUND),
		}
	});
	MockPrices::mock_collection(|pool_id| {
		assert_eq!(*pool_id, POOL_A);
		Ok(MockDataCollection::new(|id| match *id {
			REGISTER_PRICE_ID => Ok((PRICE_VALUE, BLOCK_TIME_MS)),
			_ => Err(PRICE_ID_NO_FOUND),
		}))
	});
}

fn update_portfolio() {
	assert_ok!(Loans::update_portfolio_valuation(
		RuntimeOrigin::signed(ANY),
		POOL_A
	));
}

fn expected_portfolio(valuation: Balance) {
	assert_eq!(
		valuation,
		PortfolioValuation::<Runtime>::get(POOL_A).value()
	);
}

#[test]
fn empty() {
	new_test_ext().execute_with(|| {
		expected_portfolio(0);
	});
}

#[test]
fn with_wrong_pool() {
	new_test_ext().execute_with(|| {
		config_mocks();
		assert_noop!(
			Loans::update_portfolio_valuation(RuntimeOrigin::signed(ANY), POOL_B),
			Error::<Runtime>::PoolNotFound
		);
	});
}

#[test]
fn without_active_loans() {
	new_test_ext().execute_with(|| {
		util::create_loan(util::base_external_loan());
		util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});

		advance_time(YEAR / 2);

		config_mocks();
		update_portfolio();
		expected_portfolio(0);
	});
}

#[test]
fn with_active_loans() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_1, PrincipalInput::External(amount.clone()));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});
		util::borrow_loan(loan_2, PrincipalInput::Internal(COLLATERAL_VALUE));
		util::repay_loan(loan_2, PrincipalInput::Internal(COLLATERAL_VALUE / 4));

		let valuation = amount.balance().unwrap() + COLLATERAL_VALUE - COLLATERAL_VALUE / 4;

		expected_portfolio(valuation);
		config_mocks();
		update_portfolio();
		expected_portfolio(valuation);

		advance_time(YEAR / 2);

		update_portfolio();
		expected_portfolio(util::current_loan_pv(loan_1) + util::current_loan_pv(loan_2));
	});
}

#[test]
fn with_active_written_off_loans() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_1, PrincipalInput::External(amount));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});
		util::borrow_loan(loan_2, PrincipalInput::Internal(COLLATERAL_VALUE));
		util::repay_loan(loan_2, PrincipalInput::Internal(COLLATERAL_VALUE / 4));

		advance_time(YEAR + DAY);

		util::write_off_loan(loan_1);
		util::write_off_loan(loan_2);

		config_mocks();
		update_portfolio();
		expected_portfolio(util::current_loan_pv(loan_1) + util::current_loan_pv(loan_2));
	});
}

#[test]
fn filled_and_cleaned() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_1, PrincipalInput::External(amount.clone()));

		let loan_2 = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..util::base_internal_loan()
		});
		util::borrow_loan(loan_2, PrincipalInput::Internal(COLLATERAL_VALUE));
		util::repay_loan(loan_2, PrincipalInput::Internal(COLLATERAL_VALUE / 2));

		advance_time(YEAR + DAY);

		util::write_off_loan(loan_1);

		advance_time(YEAR / 2);

		util::repay_loan(loan_1, PrincipalInput::External(amount));
		util::repay_loan(loan_2, PrincipalInput::Internal(COLLATERAL_VALUE / 2));

		advance_time(YEAR / 2);

		config_mocks();
		update_portfolio();
		expected_portfolio(0);

		util::close_loan(loan_1);
		util::close_loan(loan_2);

		expected_portfolio(0);
	});
}

#[test]
fn exact_and_inexact_matches() {
	new_test_ext().execute_with(|| {
		let loan_1 = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_1, PrincipalInput::Internal(COLLATERAL_VALUE));

		advance_time(YEAR / 2);
		config_mocks();
		update_portfolio();

		// repay_loan() should affect to the portfolio valuation with the same value as
		// the absolute valuation of the loan
		util::repay_loan(loan_1, PrincipalInput::Internal(COLLATERAL_VALUE / 2));
		expected_portfolio(util::current_loan_pv(loan_1));
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
		let loan_1 = util::create_loan(loan);

		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_1, PrincipalInput::External(amount.clone()));

		advance_time(YEAR / 2);

		// This is affected by the linear_accrual_price() computation.
		let price_value_after_half_year = PRICE_VALUE + (NOTIONAL - PRICE_VALUE) / 2;

		config_mocks();
		update_portfolio();
		expected_portfolio(QUANTITY.saturating_mul_int(price_value_after_half_year));

		// Suddenty, the oracle set a value
		const MARKET_PRICE_VALUE: Balance = 999;
		MockPrices::mock_collection(|_| {
			Ok(MockDataCollection::new(|_| {
				Ok((MARKET_PRICE_VALUE, BLOCK_TIME_MS))
			}))
		});
		let price_value_after_half_year = MARKET_PRICE_VALUE + (NOTIONAL - MARKET_PRICE_VALUE) / 2;

		update_portfolio();
		expected_portfolio(QUANTITY.saturating_mul_int(price_value_after_half_year));
	});
}

#[test]
fn empty_portfolio_with_current_timestamp() {
	new_test_ext().execute_with(|| {
		assert_eq!(
			PortfolioValuation::<Runtime>::get(POOL_A).last_updated(),
			now().as_secs()
		);
	});
}

#[test]
fn no_linear_pricing_either_settlement_or_oracle() {
	new_test_ext().execute_with(|| {
		let mut external_pricing = util::base_external_pricing();
		external_pricing.with_linear_pricing = false;
		external_pricing.max_price_variation = Rate::one();
		let loan = LoanInfo {
			pricing: Pricing::External(ExternalPricing {
				price_id: UNREGISTER_PRICE_ID,
				..external_pricing
			}),
			..util::base_external_loan()
		};
		let loan_1 = util::create_loan(loan);
		const SETTLEMENT_PRICE: Balance = 970;
		let amount = ExternalAmount::new(QUANTITY, SETTLEMENT_PRICE);
		config_mocks();

		util::borrow_loan(loan_1, PrincipalInput::External(amount.clone()));

		advance_time(YEAR / 2);

		const MARKET_PRICE_VALUE: Balance = 999;
		MockPrices::mock_collection(|_| {
			Ok(MockDataCollection::new(|_| {
				Ok((MARKET_PRICE_VALUE, BLOCK_TIME_MS))
			}))
		});

		update_portfolio();
		expected_portfolio(QUANTITY.saturating_mul_int(MARKET_PRICE_VALUE));

		MockPrices::mock_collection(|pool_id| {
			assert_eq!(*pool_id, POOL_A);
			Ok(MockDataCollection::new(|_| Err(PRICE_ID_NO_FOUND)))
		});

		update_portfolio();
		expected_portfolio(QUANTITY.saturating_mul_int(SETTLEMENT_PRICE));

		MockPrices::mock_collection(|_| {
			Ok(MockDataCollection::new(|_| {
				Ok((MARKET_PRICE_VALUE, BLOCK_TIME_MS))
			}))
		});
		update_portfolio();
		expected_portfolio(QUANTITY.saturating_mul_int(MARKET_PRICE_VALUE));
	});
}

#[test]
fn internal_dcf_with_no_maturity() {
	new_test_ext().execute_with(|| {
		let mut internal = util::dcf_internal_loan();
		internal.schedule.maturity = Maturity::None;

		let loan_id = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..internal
		});

		MockPools::mock_withdraw(|_, _, _| Ok(()));

		assert_noop!(
			Loans::borrow(
				RuntimeOrigin::signed(util::borrower(loan_id)),
				POOL_A,
				loan_id,
				PrincipalInput::Internal(COLLATERAL_VALUE),
			),
			Error::<Runtime>::MaturityDateNeededForValuationMethod
		);
	});
}

#[test]
fn internal_oustanding_debt_with_no_maturity() {
	new_test_ext().execute_with(|| {
		let mut internal = util::base_internal_loan();
		internal.schedule.maturity = Maturity::None;

		let loan_id = util::create_loan(LoanInfo {
			collateral: ASSET_BA,
			..internal
		});
		util::borrow_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks();
		let pv = util::current_loan_pv(loan_id);
		update_portfolio();
		expected_portfolio(pv);

		advance_time(YEAR);

		update_portfolio();
		expected_portfolio(
			Rate::from_float(util::interest_for(DEFAULT_INTEREST_RATE, YEAR))
				.checked_mul_int(COLLATERAL_VALUE)
				.unwrap(),
		);

		util::repay_loan(loan_id, PrincipalInput::Internal(COLLATERAL_VALUE));

		config_mocks();
		update_portfolio();
		expected_portfolio(0);
	});
}
