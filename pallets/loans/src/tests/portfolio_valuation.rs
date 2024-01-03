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
		config_mocks();
		update_portfolio();
		expected_portfolio(QUANTITY.saturating_mul_int(PRICE_VALUE));

		// Suddenty, the oracle set a value
		MockPrices::mock_collection(|_| {
			Ok(MockDataCollection::new(|_| {
				Ok((PRICE_VALUE * 8, BLOCK_TIME_MS))
			}))
		});

		update_portfolio();
		expected_portfolio(QUANTITY.saturating_mul_int(PRICE_VALUE * 8));
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
