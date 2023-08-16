use super::*;

fn config_mocks() {
	MockPermissions::mock_has(move |scope, who, role| {
		matches!(scope, PermissionScope::Pool(id) if id == POOL_A)
			&& matches!(role, Role::PoolRole(PoolRole::LoanAdmin))
			&& who == LOAN_ADMIN
	});
}

#[test]
fn without_policy() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		assert_noop!(
			Loans::write_off(RuntimeOrigin::signed(ANY), POOL_A, loan_id),
			Error::<Runtime>::NoValidWriteOffRule
		);

		config_mocks();
		assert_ok!(Loans::admin_write_off(
			RuntimeOrigin::signed(LOAN_ADMIN),
			POOL_A,
			loan_id,
			Rate::from_float(0.1),
			Rate::from_float(0.1)
		));
	});
}

#[test]
fn with_policy_but_not_overdue() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + BLOCK_TIME);

		// The loan maturity date has passed, but the policy can no be applied yet.
		assert_noop!(
			Loans::write_off(RuntimeOrigin::signed(ANY), POOL_A, loan_id),
			Error::<Runtime>::NoValidWriteOffRule
		);
	});
}

#[test]
fn with_valid_maturity() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR / 2);

		// The loan maturity date has no passed.
		assert_noop!(
			Loans::write_off(RuntimeOrigin::signed(ANY), POOL_A, loan_id),
			Error::<Runtime>::NoValidWriteOffRule
		);
	});
}

#[test]
fn with_wrong_loan_id() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		assert_noop!(
			Loans::write_off(RuntimeOrigin::signed(ANY), POOL_A, 0),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);

		config_mocks();
		assert_noop!(
			Loans::admin_write_off(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				0,
				Rate::from_float(POLICY_PERCENTAGE),
				Rate::from_float(POLICY_PENALTY)
			),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);
	});
}

#[test]
fn without_active_loan() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());

		config_mocks();
		assert_noop!(
			Loans::write_off(RuntimeOrigin::signed(ANY), POOL_A, loan_id),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);
		assert_noop!(
			Loans::admin_write_off(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				loan_id,
				Rate::from_float(POLICY_PERCENTAGE),
				Rate::from_float(POLICY_PENALTY)
			),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);
	});
}

#[test]
fn with_wrong_permission() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);

		config_mocks();
		assert_noop!(
			Loans::admin_write_off(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				Rate::from_float(POLICY_PERCENTAGE + 0.1),
				Rate::from_float(POLICY_PENALTY + 0.1)
			),
			BadOrigin
		);
	});
}

#[test]
fn with_success() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);

		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));
	});
}

#[test]
fn with_admin_success() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);

		config_mocks();

		// Write down percentage
		assert_ok!(Loans::admin_write_off(
			RuntimeOrigin::signed(LOAN_ADMIN),
			POOL_A,
			loan_id,
			Rate::from_float(POLICY_PERCENTAGE + 0.1),
			Rate::from_float(POLICY_PENALTY)
		));

		// Write down penalty
		assert_ok!(Loans::admin_write_off(
			RuntimeOrigin::signed(LOAN_ADMIN),
			POOL_A,
			loan_id,
			Rate::from_float(POLICY_PERCENTAGE + 0.1),
			Rate::from_float(POLICY_PENALTY + 0.1)
		));

		// Write up percentage
		assert_ok!(Loans::admin_write_off(
			RuntimeOrigin::signed(LOAN_ADMIN),
			POOL_A,
			loan_id,
			Rate::from_float(POLICY_PERCENTAGE),
			Rate::from_float(POLICY_PENALTY + 0.1)
		));

		// Write up penalty
		assert_ok!(Loans::admin_write_off(
			RuntimeOrigin::signed(LOAN_ADMIN),
			POOL_A,
			loan_id,
			Rate::from_float(POLICY_PERCENTAGE),
			Rate::from_float(POLICY_PENALTY)
		));
	});
}

#[test]
fn with_admin_less_than_policy() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);

		config_mocks();

		// Less percentage
		assert_noop!(
			Loans::admin_write_off(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				loan_id,
				Rate::from_float(POLICY_PERCENTAGE - 0.1),
				Rate::from_float(POLICY_PENALTY)
			),
			Error::<Runtime>::from(WrittenOffError::LessThanPolicy)
		);

		// Less penalty
		assert_noop!(
			Loans::admin_write_off(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				loan_id,
				Rate::from_float(POLICY_PERCENTAGE),
				Rate::from_float(POLICY_PENALTY - 0.1)
			),
			Error::<Runtime>::from(WrittenOffError::LessThanPolicy)
		);
	});
}

#[test]
fn with_policy_change_after() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);

		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));

		util::set_up_policy(POLICY_PERCENTAGE / 2.0, POLICY_PENALTY / 2.0);

		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));

		assert_eq!(
			WriteOffStatus {
				percentage: Rate::from_float(POLICY_PERCENTAGE),
				penalty: Rate::from_float(POLICY_PENALTY),
			},
			util::get_loan(loan_id).write_off_status()
		);
	});
}

#[test]
fn with_policy_change_after_admin() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		config_mocks();
		assert_ok!(Loans::admin_write_off(
			RuntimeOrigin::signed(LOAN_ADMIN),
			POOL_A,
			loan_id,
			Rate::from_float(POLICY_PERCENTAGE + 0.1),
			Rate::from_float(POLICY_PENALTY + 0.1)
		));

		util::set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);

		advance_time(YEAR + DAY);

		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));

		assert_eq!(
			WriteOffStatus {
				percentage: Rate::from_float(POLICY_PERCENTAGE + 0.1),
				penalty: Rate::from_float(POLICY_PENALTY + 0.1),
			},
			util::get_loan(loan_id).write_off_status()
		);
	});
}

#[test]
fn with_percentage_applied_internal() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, 0.0);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);

		let pv = util::current_loan_pv(loan_id);

		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));

		// Because we are using ValuationMethod::OutstandingDebt:
		assert_eq!(
			(pv as f64 * POLICY_PERCENTAGE) as Balance,
			util::current_loan_pv(loan_id)
		);
	});
}

#[test]
fn with_percentage_applied_external() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(POLICY_PERCENTAGE, 0.0);

		let loan_id = util::create_loan(util::base_external_loan());
		let amount = ExternalAmount::new(QUANTITY, PRICE_VALUE);
		util::borrow_loan(loan_id, PricingAmount::External(amount));

		advance_time(YEAR + DAY);

		MockPrices::mock_get(|id, pool_id| {
			assert_eq!(*pool_id, POOL_A);
			assert_eq!(*id, REGISTER_PRICE_ID);
			Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
		});
		let pv = util::current_loan_pv(loan_id);

		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));

		// Because we are using ValuationMethod::OutstandingDebt:
		assert_eq!(
			(pv as f64 * POLICY_PERCENTAGE) as Balance,
			util::current_loan_pv(loan_id)
		);
	});
}

#[test]
fn with_penalty_applied() {
	new_test_ext().execute_with(|| {
		util::set_up_policy(0.0, POLICY_PENALTY);

		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);

		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));

		// Modify an interest rate doesn't have effect in the same instant
		assert_eq!(
			util::current_debt_for(
				util::interest_for(DEFAULT_INTEREST_RATE, YEAR + DAY),
				COLLATERAL_VALUE,
			),
			util::current_loan_debt(loan_id)
		);

		advance_time(YEAR);

		// Because of math arithmetic preccission,
		// we get a difference that makes the test fail
		let precission_error = 2;

		assert_eq!(
			util::current_debt_for(
				util::interest_for(DEFAULT_INTEREST_RATE, YEAR + DAY)
					* util::interest_for(DEFAULT_INTEREST_RATE + POLICY_PENALTY, YEAR),
				COLLATERAL_VALUE,
			) - precission_error,
			util::current_loan_debt(loan_id)
		);
	});
}

#[test]
fn fully() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, PricingAmount::Internal(COLLATERAL_VALUE));

		advance_time(YEAR + DAY);

		config_mocks();
		assert_ok!(Loans::admin_write_off(
			RuntimeOrigin::signed(LOAN_ADMIN),
			POOL_A,
			loan_id,
			Rate::from_float(1.0),
			Rate::from_float(0.0)
		));

		assert_eq!(0, util::current_loan_pv(loan_id));

		advance_time(YEAR);

		assert_eq!(0, util::current_loan_pv(loan_id));
	});
}
