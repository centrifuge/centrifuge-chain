use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

use super::{
	mock::*,
	types::{
		BorrowRestrictions, InterestPayments, LoanInfo, LoanRestrictions, Maturity,
		MaxBorrowAmount, PayDownSchedule, RepayRestrictions, RepaymentSchedule,
	},
	valuation::ValuationMethod,
	Error,
};

fn loan_info() -> LoanInfo<Asset, Balance, Rate> {
	LoanInfo {
		schedule: RepaymentSchedule {
			maturity: Maturity::Fixed(BLOCK_TIME),
			interest_payments: InterestPayments::None,
			pay_down_schedule: PayDownSchedule::None,
		},
		collateral: (COLLECTION_A, ITEM_A),
		collateral_value: 1000,
		valuation_method: ValuationMethod::OutstandingDebt,
		restrictions: LoanRestrictions {
			max_borrow_amount: MaxBorrowAmount::UpToTotalBorrowed {
				advance_rate: Rate::from_float(0.5),
			},
			borrows: BorrowRestrictions::WrittenOff,
			repayments: RepayRestrictions::None,
		},
		interest_rate: Rate::from_float(0.03),
	}
}

fn mock_permissions_expectations(pool_id: PoolId) {
	MockPermissions::expect_has(move |scope, who, role| {
		let valid = matches!(scope, PermissionScope::Pool(id) if pool_id == id)
			&& matches!(role, Role::PoolRole(PoolRole::Borrower))
			&& who == BORROWER;

		valid
	});
}

fn mock_pools_expectations() {
	MockPools::expect_pool_exists(|pool_id| pool_id == POOL_A);
	MockPools::expect_account_for(|pool_id| {
		if pool_id == POOL_A {
			POOL_A_ACCOUNT
		} else {
			POOL_OTHER_ACCOUNT
		}
	});
}

#[test]
fn create_successful_loan() {
	new_test_ext().execute_with(|| {
		mock_permissions_expectations(POOL_A);
		mock_pools_expectations();

		assert_ok!(Loans::create(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_info()
		));
	});
}

#[test]
fn create_loan_bad_permission() {
	new_test_ext().execute_with(|| {
		mock_permissions_expectations(POOL_A);
		mock_pools_expectations();

		assert_noop!(
			Loans::create(RuntimeOrigin::signed(NO_BORROWER), POOL_A, loan_info()),
			BadOrigin
		);
	});
}

#[test]
fn create_loan_over_inexistent_pool() {
	new_test_ext().execute_with(|| {
		mock_permissions_expectations(POOL_B);
		mock_pools_expectations();

		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_B, loan_info()),
			Error::<Runtime>::PoolNotFound
		);
	});
}
