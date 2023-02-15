use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::{traits::BadOrigin, DispatchResult};

use super::{
	mock::*,
	types::{
		BorrowRestrictions, InterestPayments, LoanRestrictions, Maturity, MaxBorrowAmount,
		PayDownSchedule, RepayRestrictions, RepaymentSchedule,
	},
	valuation::ValuationMethod,
	Error,
};

const POOL_A: PoolId = 1;
const POOL_A_ACCOUNT: AccountId = 11;

#[derive(Clone, Copy)]
enum ExpectationChange {
	NoPermission,
	NoPool,
}

fn default_mock_expectations_with(change: Option<ExpectationChange>) {
	MockPermissions::expect_has(move |scope, who, role| {
		matches!(scope, PermissionScope::Pool(POOL_A));
		assert_eq!(who, BORROWER);
		matches!(role, Role::PoolRole(PoolRole::Borrower));

		match change {
			Some(ExpectationChange::NoPermission) => false,
			_ => true,
		}
	});

	MockPools::expect_pool_exists(move |pool_id| {
		assert_eq!(pool_id, POOL_A);

		match change {
			Some(ExpectationChange::NoPool) => false,
			_ => true,
		}
	});

	MockPools::expect_account_for(|pool_id| {
		assert_eq!(pool_id, POOL_A);

		POOL_A_ACCOUNT
	});
}

fn create_basic_loan() -> DispatchResult {
	Loans::create(
		RuntimeOrigin::signed(BORROWER),
		POOL_A,
		RepaymentSchedule {
			maturity: Maturity::Fixed(BLOCK_TIME),
			interest_payments: InterestPayments::None,
			pay_down_schedule: PayDownSchedule::None,
		},
		(COLLECTION_A, ITEM_A),
		1000,
		ValuationMethod::OutstandingDebt,
		LoanRestrictions {
			max_borrow_amount: MaxBorrowAmount::UpToTotalBorrowed {
				advance_rate: Rate::from_float(0.5),
			},
			borrows: BorrowRestrictions::WrittenOff,
			repayments: RepayRestrictions::None,
		},
		Rate::from_float(0.03),
	)
}

#[test]
fn create_successful_loan() {
	new_test_ext().execute_with(|| {
		default_mock_expectations_with(None);
		assert_ok!(create_basic_loan());
	});
}

#[test]
fn create_loan_bad_permission() {
	new_test_ext().execute_with(|| {
		default_mock_expectations_with(Some(ExpectationChange::NoPermission));
		assert_noop!(create_basic_loan(), BadOrigin);
	});
}

#[test]
fn create_loan_not_pool() {
	new_test_ext().execute_with(|| {
		default_mock_expectations_with(Some(ExpectationChange::NoPool));
		assert_noop!(create_basic_loan(), Error::<Runtime>::PoolNotFound);
	});
}
