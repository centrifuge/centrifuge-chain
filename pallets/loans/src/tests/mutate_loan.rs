use super::*;

const DEFAULT_MUTATION: LoanMutation<Rate> = LoanMutation::InterestPayments(InterestPayments::None);

fn config_mocks(loan_id: LoanId, loan_mutation: &LoanMutation<Rate>) {
	MockPermissions::mock_has(|scope, who, role| {
		matches!(scope, PermissionScope::Pool(id) if id == POOL_A)
			&& matches!(role, Role::PoolRole(PoolRole::LoanAdmin))
			&& who == LOAN_ADMIN
	});

	MockChangeGuard::mock_note({
		let loan_mutation = loan_mutation.clone();
		move |pool_id, change| {
			assert_eq!(pool_id, POOL_A);
			assert_eq!(change, Change::Loan(loan_id, loan_mutation.clone()));
			Ok(CHANGE_ID)
		}
	});

	MockChangeGuard::mock_released({
		let loan_mutation = loan_mutation.clone();
		move |pool_id, _| {
			assert_eq!(pool_id, POOL_A);
			Ok(Change::Loan(loan_id, loan_mutation.clone()))
		}
	});
}

#[test]
fn without_active_loan() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());

		config_mocks(loan_id, &DEFAULT_MUTATION);
		assert_noop!(
			Loans::propose_loan_mutation(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				loan_id,
				DEFAULT_MUTATION
			),
			Error::<Runtime>::LoanNotActiveOrNotFound
		);
	});
}

#[test]
fn with_wrong_permissions() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, 0);

		config_mocks(loan_id, &DEFAULT_MUTATION);
		assert_noop!(
			Loans::propose_loan_mutation(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				DEFAULT_MUTATION
			),
			BadOrigin
		);

		assert_noop!(
			Loans::propose_loan_mutation(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_B,
				loan_id,
				DEFAULT_MUTATION
			),
			BadOrigin
		);
	});
}

#[test]
fn with_wrong_dcf_mutation() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, 0);

		let mutation =
			LoanMutation::Internal(InternalMutation::DiscountRate(Rate::from_float(0.5)));

		config_mocks(loan_id, &mutation);
		assert_noop!(
			Loans::propose_loan_mutation(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				loan_id,
				mutation,
			),
			Error::<Runtime>::MutationError(MutationError::DiscountedCashFlowExpected)
		);
	});
}

#[test]
fn with_wrong_interest_rate() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, 0);

		// Too high
		let mutation =
			LoanMutation::Internal(InternalMutation::InterestRate(Rate::from_float(3.0)));

		config_mocks(loan_id, &mutation);
		assert_noop!(
			Loans::propose_loan_mutation(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				loan_id,
				mutation,
			),
			pallet_interest_accrual::Error::<Runtime>::InvalidRate
		);
	});
}

#[test]
fn with_wrong_internal() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_external_loan());
		util::borrow_loan(loan_id, 0);

		let mutation =
			LoanMutation::Internal(InternalMutation::InterestRate(Rate::from_float(0.2)));

		config_mocks(loan_id, &mutation);
		assert_noop!(
			Loans::propose_loan_mutation(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				loan_id,
				mutation,
			),
			Error::<Runtime>::MutationError(MutationError::InternalPricingExpected)
		);
	});
}

#[test]
fn with_successful_proposal() {
	new_test_ext().execute_with(|| {
		let loan_id = util::create_loan(util::base_internal_loan());
		util::borrow_loan(loan_id, 0);

		config_mocks(loan_id, &DEFAULT_MUTATION);

		assert_ok!(Loans::propose_loan_mutation(
			RuntimeOrigin::signed(LOAN_ADMIN),
			POOL_A,
			loan_id,
			DEFAULT_MUTATION
		));
	});
}

#[test]
fn with_successful_mutation_application() {
	new_test_ext().execute_with(|| {
		let loan = LoanInfo {
			schedule: RepaymentSchedule {
				maturity: Maturity::Fixed((now() + YEAR).as_secs()),
				interest_payments: InterestPayments::None,
				pay_down_schedule: PayDownSchedule::None,
			},
			pricing: Pricing::Internal(InternalPricing {
				interest_rate: Rate::from_float(0.1),
				valuation_method: ValuationMethod::DiscountedCashFlow(DiscountedCashFlow {
					probability_of_default: Rate::from_float(0.1),
					loss_given_default: Rate::from_float(0.1),
					discount_rate: Rate::from_float(0.1), // Too high
				}),
				..util::base_internal_pricing()
			}),
			..util::base_internal_loan()
		};

		let loan_id = util::create_loan(loan);
		util::borrow_loan(loan_id, COLLATERAL_VALUE / 2);

		let mutations = vec![
			// LoanMutation::InterestPayments(..), No changes, only one variant
			// LoanMutation::PayDownSchedule(..), No changes, only one variant
			LoanMutation::Maturity(Maturity::Fixed((now() + YEAR * 2).as_secs())),
			LoanMutation::Internal(InternalMutation::InterestRate(Rate::from_float(0.5))),
			LoanMutation::Internal(InternalMutation::ProbabilityOfDefault(Rate::from_float(
				0.5,
			))),
			LoanMutation::Internal(InternalMutation::LossGivenDefault(Rate::from_float(0.5))),
			LoanMutation::Internal(InternalMutation::DiscountRate(Rate::from_float(0.5))),
			LoanMutation::Internal(InternalMutation::ValuationMethod(
				ValuationMethod::OutstandingDebt,
			)),
		];

		for mutation in mutations {
			config_mocks(loan_id, &mutation);

			let pre_pv = util::current_loan_pv(loan_id);
			let pre_loan = util::get_loan(loan_id);

			assert_ok!(Loans::propose_loan_mutation(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				loan_id,
				mutation
			));

			let mid_pv = util::current_loan_pv(loan_id);
			let mid_loan = util::get_loan(loan_id);

			// Proposing changes no modify neither the PV or the loan
			assert_eq!(pre_pv, mid_pv);
			assert_eq!(pre_loan, mid_loan);

			assert_ok!(Loans::apply_change(
				RuntimeOrigin::signed(LOAN_ADMIN),
				POOL_A,
				CHANGE_ID,
			));

			let post_pv = util::current_loan_pv(loan_id);
			let post_loan = util::get_loan(loan_id);

			// Applying changes modify both the PV or the loan
			assert_ne!(mid_pv, post_pv);
			assert_ne!(mid_loan, post_loan);
		}
	});
}
