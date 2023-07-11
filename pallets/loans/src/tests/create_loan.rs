use super::*;

fn config_mocks(pool_id: PoolId) {
	MockPermissions::mock_has(move |scope, who, role| {
		matches!(scope, PermissionScope::Pool(id) if pool_id == id)
			&& matches!(role, Role::PoolRole(PoolRole::Borrower))
			&& who == BORROWER
	});
	MockPools::mock_pool_exists(|pool_id| pool_id == POOL_A);
	MockPools::mock_account_for(|pool_id| {
		if pool_id == POOL_A {
			POOL_A_ACCOUNT
		} else {
			POOL_OTHER_ACCOUNT
		}
	});
	MockPrices::mock_get(|id| match *id {
		REGISTER_PRICE_ID => Ok((PRICE_VALUE, BLOCK_TIME.as_secs())),
		_ => Err("Should never be dispatched".into()),
	});
}

#[test]
fn with_wrong_permissions() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		let loan = util::base_internal_loan();
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(NO_BORROWER), POOL_A, loan),
			BadOrigin
		);
	});
}

#[test]
fn with_wrong_pool() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_B);

		let loan = util::base_internal_loan();
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_B, loan),
			Error::<Runtime>::PoolNotFound
		);
	});
}

#[test]
fn with_wrong_assets() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		let loan = LoanInfo {
			collateral: NO_ASSET,
			..util::base_internal_loan()
		};
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::NFTOwnerNotFound
		);

		let loan = LoanInfo {
			collateral: ASSET_AB,
			..util::base_internal_loan()
		};
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::NotNFTOwner
		);

		let loan = util::base_internal_loan();
		assert_ok!(Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan));

		// Using the same NFT no longer works, because the pool owns it.
		let loan = util::base_internal_loan();
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::NotNFTOwner
		);
	});
}

#[test]
fn with_wrong_schedule() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		let loan = LoanInfo {
			schedule: RepaymentSchedule {
				maturity: Maturity::fixed(now().as_secs()),
				interest_payments: InterestPayments::None,
				pay_down_schedule: PayDownSchedule::None,
			},
			..util::base_internal_loan()
		};
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::from(CreateLoanError::InvalidRepaymentSchedule)
		);
	});
}

#[test]
fn with_wrong_valuation() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		let loan = LoanInfo {
			pricing: Pricing::Internal(InternalPricing {
				valuation_method: ValuationMethod::DiscountedCashFlow(DiscountedCashFlow {
					probability_of_default: Rate::from_float(0.0),
					loss_given_default: Rate::from_float(0.0),
					discount_rate: Rate::from_float(1.1), // Too high
				}),
				..util::base_internal_pricing()
			}),
			..util::base_internal_loan()
		};

		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::from(CreateLoanError::InvalidValuationMethod)
		);
	});
}

#[test]
fn with_wrong_interest_rate() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		let loan = LoanInfo {
			interest_rate: Rate::from_float(3.0), // Too high
			..util::base_internal_loan()
		};

		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			pallet_interest_accrual::Error::<Runtime>::InvalidRate
		);
	});
}

#[test]
fn with_unregister_price_id() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		let loan = LoanInfo {
			pricing: Pricing::External(ExternalPricing {
				price_id: UNREGISTER_PRICE_ID,
				..util::base_external_pricing()
			}),
			..util::base_external_loan()
		};

		assert_ok!(Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan));
	});
}

#[test]
fn with_success_internal_pricing() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		let loan = util::base_internal_loan();
		assert_ok!(Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan));

		assert_eq!(
			Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(),
			POOL_A_ACCOUNT
		);
	});
}

#[test]
fn with_success_external_pricing() {
	new_test_ext().execute_with(|| {
		config_mocks(POOL_A);

		let loan = util::base_external_loan();
		assert_ok!(Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan));

		assert_eq!(
			Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(),
			POOL_A_ACCOUNT
		);
	});
}
