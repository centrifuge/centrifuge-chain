use std::time::Duration;

use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::{traits::BadOrigin, FixedPointNumber};

use super::{
	mock::*,
	types::{BorrowLoanError, CreateLoanError, LoanInfo, MaxBorrowAmount, WriteOffState},
	valuation::{DiscountedCashFlows, ValuationMethod},
	ActiveLoans, Error, LastLoanId,
};

const COLLATERAL_VALUE: Balance = 100;
const DEFAULT_INTEREST_RATE: f64 = 0.5;

fn total_borrowed_rate(value: f64) -> MaxBorrowAmount<Rate> {
	MaxBorrowAmount::UpToTotalBorrowed {
		advance_rate: Rate::from_float(value),
	}
}

fn outstanding_debt_rate(value: f64) -> MaxBorrowAmount<Rate> {
	MaxBorrowAmount::UpToOutstandingDebt {
		advance_rate: Rate::from_float(value),
	}
}

fn current_debt(loan_id: LoanId) -> Balance {
	ActiveLoans::<Runtime>::get(POOL_A)
		.iter()
		.find(|(loan, _)| loan.loan_id() == loan_id)
		.unwrap()
		.0
		.debt(None)
		.unwrap()
}

fn compute_debt_for(amount: Balance, elapsed: Duration) -> Balance {
	// amount * (1 + rate_sec) ^ elapsed
	((1.0 + DEFAULT_INTEREST_RATE / YEAR.as_secs() as f64).powi(elapsed.as_secs() as i32)
		* amount as f64) as Balance
}

mod create_loan {
	use super::*;

	pub fn do_it(max_borrow_amount: MaxBorrowAmount<Rate>) -> LoanId {
		config_mocks(POOL_A);

		assert_ok!(Loans::create(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			LoanInfo::new(ASSET_AA)
				.maturity(now() + YEAR)
				.interest_rate(Rate::from_float(DEFAULT_INTEREST_RATE))
				.collateral_value(COLLATERAL_VALUE)
				.max_borrow_amount(max_borrow_amount)
		));

		LastLoanId::<Runtime>::get(POOL_A)
	}

	fn config_mocks(pool_id: PoolId) {
		MockPermissions::mock_has(move |scope, who, role| {
			let valid = matches!(scope, PermissionScope::Pool(id) if pool_id == id)
				&& matches!(role, Role::PoolRole(PoolRole::Borrower))
				&& who == BORROWER;

			valid
		});
		MockPools::mock_pool_exists(|pool_id| pool_id == POOL_A);
		MockPools::mock_account_for(|pool_id| {
			if pool_id == POOL_A {
				POOL_A_ACCOUNT
			} else {
				POOL_OTHER_ACCOUNT
			}
		});
	}

	#[test]
	fn with_success() {
		new_test_ext().execute_with(|| {
			config_mocks(POOL_A);

			let loan = LoanInfo::new(ASSET_AA).maturity(now());
			assert_ok!(Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan));
		});
	}

	#[test]
	fn with_wrong_permissions() {
		new_test_ext().execute_with(|| {
			config_mocks(POOL_A);

			let loan = LoanInfo::new(ASSET_AA).maturity(now());
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

			let loan = LoanInfo::new(ASSET_AA).maturity(now());
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

			let loan = LoanInfo::new(NO_ASSET).maturity(now());
			assert_noop!(
				Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
				Error::<Runtime>::NFTOwnerNotFound
			);

			let loan = LoanInfo::new(ASSET_AB).maturity(now());
			assert_noop!(
				Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
				Error::<Runtime>::NotNFTOwner
			);

			let loan = LoanInfo::new(ASSET_AA).maturity(now());
			assert_ok!(Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan));

			// Using the same NFT no longer works, because the pool owns it.
			let loan = LoanInfo::new(ASSET_AA).maturity(now());
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

			let loan = LoanInfo::new(ASSET_AA).maturity(now() - BLOCK_TIME);
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

			let loan = LoanInfo::new(ASSET_AA).maturity(now()).valuation_method(
				ValuationMethod::DiscountedCashFlows(
					DiscountedCashFlows::default().discount_rate(Rate::from_float(0.9)),
				),
			);

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

			let loan = LoanInfo::new(ASSET_AA)
				.maturity(now())
				.interest_rate(Rate::from_float(1.1));

			assert_noop!(
				Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
				pallet_interest_accrual::Error::<Runtime>::InvalidRate
			);
		});
	}
}

mod borrow_loan {
	use super::*;

	pub fn do_it(loan_id: LoanId, borrow_amount: Balance) {
		config_mocks(borrow_amount);

		assert_ok!(Loans::borrow(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			borrow_amount
		));
	}

	fn config_mocks(withdraw_amount: Balance) {
		MockPools::mock_withdraw(move |pool_id, to, amount| {
			assert_eq!(to, BORROWER);
			assert_eq!(pool_id, POOL_A);
			assert_eq!(withdraw_amount, amount);
			Ok(())
		});
	}

	fn write_off_loan(loan_id: LoanId) {
		MockPermissions::mock_has(|_, _, _| true);

		assert_ok!(Loans::update_write_off_policy(
			RuntimeOrigin::signed(0),
			POOL_A,
			vec![WriteOffState {
				overdue_days: 1,
				percentage: Rate::from_float(0.1),
				penalty: Rate::from_float(0.0)
			}]
			.try_into()
			.unwrap()
		));

		advance_time(YEAR + DAY);

		assert_ok!(Loans::write_off(RuntimeOrigin::signed(0), POOL_A, loan_id));
	}

	#[test]
	fn with_correct_amounts() {
		let borrow_inputs = [
			(COLLATERAL_VALUE, total_borrowed_rate(1.0)),
			(COLLATERAL_VALUE / 2, total_borrowed_rate(0.5)),
			(0, total_borrowed_rate(0.0)),
			(COLLATERAL_VALUE, outstanding_debt_rate(1.0)),
			(COLLATERAL_VALUE / 2, outstanding_debt_rate(0.5)),
			(0, outstanding_debt_rate(0.0)),
		];

		for (amount, max_borrow_amount) in borrow_inputs {
			new_test_ext().execute_with(|| {
				let loan_id = create_loan::do_it(max_borrow_amount);

				config_mocks(amount);
				assert_ok!(Loans::borrow(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					amount
				));
				assert_eq!(amount, current_debt(loan_id));
			});
		}
	}

	#[test]
	fn with_wrong_amounts() {
		let borrow_inputs = [
			(COLLATERAL_VALUE + 1, total_borrowed_rate(1.0)),
			(COLLATERAL_VALUE / 2 + 1, total_borrowed_rate(0.5)),
			(1, total_borrowed_rate(0.0)),
			(COLLATERAL_VALUE + 1, outstanding_debt_rate(1.0)),
			(COLLATERAL_VALUE / 2 + 1, outstanding_debt_rate(0.5)),
			(1, outstanding_debt_rate(0.0)),
		];

		for (amount, max_borrow_amount) in borrow_inputs {
			new_test_ext().execute_with(|| {
				let loan_id = create_loan::do_it(max_borrow_amount);

				config_mocks(amount);
				assert_noop!(
					Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, amount),
					Error::<Runtime>::from(BorrowLoanError::MaxAmountExceeded)
				);
			});
		}
	}

	#[test]
	fn twice() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));

			config_mocks(COLLATERAL_VALUE / 2);

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));
			assert_eq!(COLLATERAL_VALUE / 2, current_debt(loan_id));

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));
			assert_eq!(COLLATERAL_VALUE, current_debt(loan_id));

			// At this point the loan has been totally borrowed.
			let extra = 1;
			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, extra),
				Error::<Runtime>::from(BorrowLoanError::MaxAmountExceeded)
			);
		});
	}

	#[test]
	fn twice_with_elapsed_time() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));

			config_mocks(COLLATERAL_VALUE / 2);

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));
			assert_eq!(COLLATERAL_VALUE / 2, current_debt(loan_id));

			advance_time(YEAR / 2);

			assert_eq!(
				compute_debt_for(COLLATERAL_VALUE / 2, YEAR / 2),
				current_debt(loan_id)
			);

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));

			// At this point the loan has been totally borrowed.
			let extra = 1;
			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, extra),
				Error::<Runtime>::from(BorrowLoanError::MaxAmountExceeded)
			);
		});
	}

	#[test]
	fn with_wrong_loan_id() {
		new_test_ext().execute_with(|| {
			config_mocks(COLLATERAL_VALUE);

			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, 0, COLLATERAL_VALUE),
				Error::<Runtime>::LoanNotFound
			);
		});
	}

	#[test]
	fn from_other_borrower() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));

			config_mocks(COLLATERAL_VALUE);

			assert_noop!(
				Loans::borrow(
					RuntimeOrigin::signed(OTHER_BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE
				),
				Error::<Runtime>::NotLoanBorrower
			);
		});
	}

	#[test]
	fn with_maturity_passed() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));

			advance_time(YEAR + BLOCK_TIME);

			config_mocks(COLLATERAL_VALUE);

			// It's ok because should be written off to avoid borrowing
			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE
			));
		});
	}

	#[test]
	fn has_been_written_off() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));

			config_mocks(COLLATERAL_VALUE / 2);

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));

			write_off_loan(loan_id);

			assert_noop!(
				Loans::borrow(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE / 2
				),
				Error::<Runtime>::from(BorrowLoanError::WrittenOffRestriction)
			);
		});
	}
}

mod repay_loan {
	use super::*;

	const COLLATERAL_VALUE: Balance = 100;

	pub fn config_mocks(deposit_amount: Balance) {
		MockPools::mock_deposit(move |pool_id, to, amount| {
			assert_eq!(to, BORROWER);
			assert_eq!(pool_id, POOL_A);
			assert_eq!(deposit_amount, amount);
			Ok(())
		});
	}

	#[test]
	fn with_success() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));
			borrow_loan::do_it(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE
			));
			assert_eq!(0, current_debt(loan_id));
		});
	}

	#[test]
	fn with_more_than_required() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));
			borrow_loan::do_it(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE * 2
			));
		});
	}

	#[test]
	fn twice() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));
			borrow_loan::do_it(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE / 2);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));
			assert_eq!(COLLATERAL_VALUE / 2, current_debt(loan_id));

			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));
			assert_eq!(0, current_debt(loan_id));

			// At this point the loan has been fully repaid.
			let extra = 1;
			config_mocks(0);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				extra
			));
		});
	}

	#[test]
	fn twice_with_elapsed_time() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));
			borrow_loan::do_it(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE / 2);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));

			advance_time(YEAR / 2);

			assert_eq!(
				compute_debt_for(COLLATERAL_VALUE / 2, YEAR / 2),
				current_debt(loan_id)
			);

			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));

			// Because of the interest, it has no fully repaid, we need an extra payment.
			assert_ne!(0, current_debt(loan_id));

			config_mocks(current_debt(loan_id));
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				compute_debt_for(COLLATERAL_VALUE / 2, YEAR / 2)
					+ compute_debt_for(COLLATERAL_VALUE / 2, Duration::ZERO)
					- COLLATERAL_VALUE
			));

			assert_eq!(0, current_debt(loan_id));
		});
	}

	#[test]
	fn without_borrow_first() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));

			config_mocks(COLLATERAL_VALUE);
			assert_noop!(
				Loans::repay(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE
				),
				Error::<Runtime>::LoanNotActive
			);
		});
	}

	#[test]
	fn with_wrong_loan_id() {
		new_test_ext().execute_with(|| {
			config_mocks(COLLATERAL_VALUE);

			assert_noop!(
				Loans::repay(RuntimeOrigin::signed(BORROWER), POOL_A, 0, COLLATERAL_VALUE),
				Error::<Runtime>::LoanNotFound
			);
		});
	}

	#[test]
	fn from_other_borrower() {
		new_test_ext().execute_with(|| {
			let loan_id = create_loan::do_it(total_borrowed_rate(1.0));
			borrow_loan::do_it(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE);
			assert_noop!(
				Loans::repay(
					RuntimeOrigin::signed(OTHER_BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE
				),
				Error::<Runtime>::NotLoanBorrower
			);
		});
	}
}

mod write_off_loan {
	use super::*;

	//TODO
}

mod close_loan {
	use super::*;

	//TODO
}
