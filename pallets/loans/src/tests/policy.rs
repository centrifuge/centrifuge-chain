use super::*;

fn config_mocks(pool_id: PoolId) {
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
}

#[test]
fn with_wrong_permissions() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		assert_noop!(
			Loans::update_write_off_policy(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				vec![WriteOffRule::new(
					[WriteOffTrigger::PrincipalOverdueDays(1)],
					Rate::from_float(POLICY_PERCENTAGE),
					Rate::from_float(POLICY_PENALTY),
				)]
				.try_into()
				.unwrap(),
			),
			BadOrigin
		);
	});
}

#[test]
fn with_wrong_pool() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_B);

		assert_noop!(
			Loans::update_write_off_policy(
				RuntimeOrigin::signed(POOL_ADMIN),
				POOL_B,
				vec![WriteOffRule::new(
					[WriteOffTrigger::PrincipalOverdueDays(1)],
					Rate::from_float(POLICY_PERCENTAGE),
					Rate::from_float(POLICY_PENALTY),
				)]
				.try_into()
				.unwrap(),
			),
			Error::<Runtime>::PoolNotFound
		);
	});
}

#[test]
fn with_overwrite() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		assert_ok!(Loans::update_write_off_policy(
			RuntimeOrigin::signed(POOL_ADMIN),
			POOL_A,
			vec![WriteOffRule::new(
				[WriteOffTrigger::PrincipalOverdueDays(1)],
				Rate::from_float(POLICY_PERCENTAGE),
				Rate::from_float(POLICY_PENALTY),
			)]
			.try_into()
			.unwrap(),
		));

		assert_ok!(Loans::update_write_off_policy(
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

		config_mocks(POOL_A);
		assert_ok!(Loans::update_write_off_policy(
			RuntimeOrigin::signed(POOL_ADMIN),
			POOL_A,
			vec![WriteOffRule::new(
				[WriteOffTrigger::PriceOutdated(10)],
				Rate::from_float(POLICY_PERCENTAGE),
				Rate::from_float(POLICY_PENALTY)
			),]
			.try_into()
			.unwrap(),
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

		config_mocks(POOL_A);
		assert_ok!(Loans::update_write_off_policy(
			RuntimeOrigin::signed(POOL_ADMIN),
			POOL_A,
			vec![
				WriteOffRule::new(
					[WriteOffTrigger::PriceOutdated(10)],
					Rate::from_float(0.8),
					Rate::from_float(0.8)
				),
				WriteOffRule::new(
					[
						WriteOffTrigger::PrincipalOverdueDays(1),
						WriteOffTrigger::PriceOutdated(0)
					],
					Rate::from_float(0.2),
					Rate::from_float(0.2)
				),
				WriteOffRule::new(
					[WriteOffTrigger::PrincipalOverdueDays(4)],
					Rate::from_float(0.5),
					Rate::from_float(0.5)
				),
				WriteOffRule::new(
					[WriteOffTrigger::PrincipalOverdueDays(9)],
					Rate::from_float(0.3),
					Rate::from_float(0.9)
				),
			]
			.try_into()
			.unwrap(),
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
