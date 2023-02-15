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

#[test]
fn create_successful_loan() {
	new_test_ext().execute_with(|| {
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
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(NO_BORROWER), POOL_A, loan_info()),
			BadOrigin
		);
	});
}

#[test]
fn create_loan_not_pool() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_B, loan_info()),
			Error::<Runtime>::PoolNotFound
		);
	});
}
