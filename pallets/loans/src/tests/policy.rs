use super::*;

fn config_mocks(pool_id: PoolId, policy: &BoundedVec<WriteOffRule<Rate>, MaxWriteOffPolicySize>) {
	MockPermissions::mock_has(move |scope, who, role| {
		matches!(scope, PermissionScope::Pool(id) if pool_id == id)
			&& matches!(role, Role::PoolRole(PoolRole::PoolAdmin))
			&& who == POOL_ADMIN
	});
	MockPools::mock_pool_exists(|pool_id| pool_id == POOL_A);
	MockPrices::mock_get(|id| {
		assert_eq!(*id, REGISTER_PRICE_ID);
		Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
	});
	MockChangeGuard::mock_note({
		let policy = policy.clone();
		move |pool_id, change| {
			assert_eq!(pool_id, POOL_A);
			assert_eq!(change, Change::Policy(policy.clone()));
			Ok(CHANGE_ID)
		}
	});
	MockChangeGuard::mock_released({
		let policy = policy.clone();
		move |pool_id, change_id| {
			assert_eq!(pool_id, POOL_A);
			assert_eq!(change_id, CHANGE_ID);
			Ok(Change::Policy(policy.clone()))
		}
	});
}

#[test]
fn with_wrong_permissions() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A, &vec![].try_into().unwrap());

		assert_noop!(
			Loans::propose_write_off_policy(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				vec![].try_into().unwrap()
			),
			BadOrigin
		);
	});
}

#[test]
fn with_wrong_pool() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_B, &vec![].try_into().unwrap());

		assert_noop!(
			Loans::propose_write_off_policy(
				RuntimeOrigin::signed(POOL_ADMIN),
				POOL_B,
				vec![].try_into().unwrap()
			),
			Error::<Runtime>::PoolNotFound
		);
	});
}

#[test]
fn apply_without_released() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A, &vec![].try_into().unwrap());
		MockChangeGuard::mock_released(|_, _| Err("err".into()));

		assert_noop!(
			Loans::apply_write_off_policy(RuntimeOrigin::signed(ANY), POOL_A, CHANGE_ID),
			DispatchError::Other("err")
		);
	});
}

#[test]
fn with_wrong_loan_mutation_change() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A, &vec![].try_into().unwrap());

		MockChangeGuard::mock_released(|_, _| {
			Ok(Change::Loan(
				1,
				LoanMutation::PayDownSchedule(PayDownSchedule::None),
			))
		});

		assert_noop!(
			Loans::apply_write_off_policy(RuntimeOrigin::signed(ANY), POOL_A, CHANGE_ID),
			Error::<Runtime>::UnrelatedChangeId
		);
	});
}

#[test]
fn with_successful_overwriting() {
	new_test_ext().execute_with(|| {
		let policy: BoundedVec<_, _> = vec![WriteOffRule::new(
			[WriteOffTrigger::PrincipalOverdue(1)],
			Rate::from_float(POLICY_PERCENTAGE),
			Rate::from_float(POLICY_PENALTY),
		)]
		.try_into()
		.unwrap();

		config_mocks(POOL_A, &policy.clone());

		assert_ok!(Loans::propose_write_off_policy(
			RuntimeOrigin::signed(POOL_ADMIN),
			POOL_A,
			policy
		));

		config_mocks(POOL_A, &vec![].try_into().unwrap());

		assert_ok!(Loans::propose_write_off_policy(
			RuntimeOrigin::signed(POOL_ADMIN),
			POOL_A,
			vec![].try_into().unwrap(),
		));
	});
}

#[test]
fn with_price_outdated() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		let amount = PRICE_VALUE.saturating_mul_int(QUANTITY);
		util::borrow_loan(loan_id, amount);

		let policy: BoundedVec<_, _> = vec![WriteOffRule::new(
			[WriteOffTrigger::PriceOutdated(10)],
			Rate::from_float(POLICY_PERCENTAGE),
			Rate::from_float(POLICY_PENALTY),
		)]
		.try_into()
		.unwrap();

		config_mocks(POOL_A, &policy.clone());
		assert_ok!(Loans::propose_write_off_policy(
			RuntimeOrigin::signed(POOL_ADMIN),
			POOL_A,
			policy,
		));
		assert_ok!(Loans::apply_write_off_policy(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			CHANGE_ID
		));

		advance_time(Duration::from_secs(9));
		assert_noop!(
			Loans::write_off(RuntimeOrigin::signed(ANY), POOL_A, loan_id),
			Error::<Runtime>::NoValidWriteOffRule
		);

		advance_time(Duration::from_secs(1));
		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));

		assert_eq!(
			util::get_loan(loan_id).write_off_status(),
			WriteOffStatus {
				percentage: Rate::from_float(POLICY_PERCENTAGE),
				penalty: Rate::from_float(0.0),
			}
		);
	});
}

#[test]
fn with_success() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, COLLATERAL_VALUE);

		let policy: BoundedVec<_, _> = vec![
			WriteOffRule::new(
				[WriteOffTrigger::PriceOutdated(10)],
				Rate::from_float(0.8),
				Rate::from_float(0.8),
			),
			WriteOffRule::new(
				[
					WriteOffTrigger::PrincipalOverdue(1),
					WriteOffTrigger::PriceOutdated(0),
				],
				Rate::from_float(0.2),
				Rate::from_float(0.2),
			),
			WriteOffRule::new(
				[WriteOffTrigger::PrincipalOverdue(4)],
				Rate::from_float(0.5),
				Rate::from_float(0.5),
			),
			WriteOffRule::new(
				[WriteOffTrigger::PrincipalOverdue(9)],
				Rate::from_float(0.3),
				Rate::from_float(0.9),
			),
		]
		.try_into()
		.unwrap();

		config_mocks(POOL_A, &policy);

		assert_ok!(Loans::apply_write_off_policy(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			CHANGE_ID
		));

		// Check if a loan is correctly writen off
		advance_time(YEAR + DAY * 10);
		assert_ok!(Loans::write_off(
			RuntimeOrigin::signed(ANY),
			POOL_A,
			loan_id
		));

		// It returns the third rule because is the overdue rule with higher write off
		// percentage.
		assert_eq!(
			util::get_loan(loan_id).write_off_status(),
			WriteOffStatus {
				percentage: Rate::from_float(0.5),
				penalty: Rate::from_float(0.5),
			}
		);
	});
}
