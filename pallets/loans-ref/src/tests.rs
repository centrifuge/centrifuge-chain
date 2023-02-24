use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

use super::{
	mock::*,
	types::{BorrowLoanError, CreateLoanError, LoanInfo, MaxBorrowAmount, WriteOffState},
	valuation::{DiscountedCashFlows, ValuationMethod},
	Error, LastLoanId,
};

mod create_loan {
	use super::*;

	pub fn config_mocks(pool_id: PoolId) {
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

	const AMOUNT: Balance = 100;

	pub fn config_mocks(withdraw_amount: u128) {
		MockPools::mock_withdraw(move |pool_id, to, amount| {
			assert_eq!(to, BORROWER);
			assert_eq!(pool_id, POOL_A);
			assert_eq!(withdraw_amount, amount);
			Ok(())
		});
	}

	fn create_successful_loan(max_borrow_amount: MaxBorrowAmount<Rate>) -> LoanId {
		create_loan::config_mocks(POOL_A);

		assert_ok!(Loans::create(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			LoanInfo::new(ASSET_AA)
				.maturity(now())
				.collateral_value(AMOUNT)
				.max_borrow_amount(max_borrow_amount)
		));

		LastLoanId::<Runtime>::get(POOL_A)
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

		advance_block_time(DAY_IN_BLOCKS);

		assert_ok!(Loans::write_off(RuntimeOrigin::signed(0), POOL_A, loan_id));
	}

	#[test]
	fn with_success() {
		let borrow_inputs = [
			(
				AMOUNT,
				MaxBorrowAmount::UpToTotalBorrowed {
					advance_rate: Rate::from_float(1.0),
				},
			),
			(
				AMOUNT / 2,
				MaxBorrowAmount::UpToTotalBorrowed {
					advance_rate: Rate::from_float(0.5),
				},
			),
			(
				0,
				MaxBorrowAmount::UpToTotalBorrowed {
					advance_rate: Rate::from_float(0.0),
				},
			),
			(
				AMOUNT,
				MaxBorrowAmount::UpToOutstandingDebt {
					advance_rate: Rate::from_float(1.0),
				},
			),
			(
				AMOUNT / 2,
				MaxBorrowAmount::UpToOutstandingDebt {
					advance_rate: Rate::from_float(0.5),
				},
			),
			(
				0,
				MaxBorrowAmount::UpToOutstandingDebt {
					advance_rate: Rate::from_float(0.0),
				},
			),
		];

		for (amount, max_borrow_amount) in borrow_inputs {
			new_test_ext().execute_with(|| {
				config_mocks(amount);

				let loan_id = create_successful_loan(max_borrow_amount);

				assert_ok!(Loans::borrow(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					amount
				));
			});
		}
	}

	#[test]
	fn with_wrong_amounts() {
		let borrow_inputs = [
			(
				AMOUNT + 1,
				MaxBorrowAmount::UpToTotalBorrowed {
					advance_rate: Rate::from_float(1.0),
				},
			),
			(
				AMOUNT / 2 + 1,
				MaxBorrowAmount::UpToTotalBorrowed {
					advance_rate: Rate::from_float(0.5),
				},
			),
			(
				1,
				MaxBorrowAmount::UpToTotalBorrowed {
					advance_rate: Rate::from_float(0.0),
				},
			),
			(
				AMOUNT + 1,
				MaxBorrowAmount::UpToOutstandingDebt {
					advance_rate: Rate::from_float(1.0),
				},
			),
			(
				AMOUNT / 2 + 1,
				MaxBorrowAmount::UpToOutstandingDebt {
					advance_rate: Rate::from_float(0.5),
				},
			),
			(
				1,
				MaxBorrowAmount::UpToOutstandingDebt {
					advance_rate: Rate::from_float(0.0),
				},
			),
		];

		for (amount, max_borrow_amount) in borrow_inputs {
			new_test_ext().execute_with(|| {
				config_mocks(amount);

				let loan_id = create_successful_loan(max_borrow_amount);

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
			config_mocks(AMOUNT / 2);

			let loan_id = create_successful_loan(MaxBorrowAmount::UpToOutstandingDebt {
				advance_rate: Rate::from_float(1.0),
			});

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				AMOUNT / 2
			));

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				AMOUNT / 2
			));

			// At this point the loan has been totally borrowed.

			config_mocks(1);

			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, 1),
				Error::<Runtime>::from(BorrowLoanError::MaxAmountExceeded)
			);
		});
	}

	#[test]
	fn with_wrong_loan_id() {
		new_test_ext().execute_with(|| {
			config_mocks(AMOUNT);

			let loan_id = create_successful_loan(MaxBorrowAmount::UpToOutstandingDebt {
				advance_rate: Rate::from_float(1.0),
			});

			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id + 1, AMOUNT),
				Error::<Runtime>::LoanNotFound
			);
		});
	}

	#[test]
	fn with_wrong_params() {
		new_test_ext().execute_with(|| {
			config_mocks(AMOUNT);

			let loan_id = create_successful_loan(MaxBorrowAmount::UpToOutstandingDebt {
				advance_rate: Rate::from_float(1.0),
			});

			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER_2), POOL_A, loan_id, AMOUNT),
				Error::<Runtime>::NotLoanBorrower
			);
		});
	}

	#[test]
	fn with_maturity_passed() {
		new_test_ext().execute_with(|| {
			config_mocks(AMOUNT);

			let loan_id = create_successful_loan(MaxBorrowAmount::UpToOutstandingDebt {
				advance_rate: Rate::from_float(1.0),
			});

			advance_block_time(1);

			// It's ok because should be written off to avoid borrowing
			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				AMOUNT
			));
		});
	}

	#[test]
	fn has_been_written_off() {
		new_test_ext().execute_with(|| {
			config_mocks(AMOUNT / 2);

			let loan_id = create_successful_loan(MaxBorrowAmount::UpToOutstandingDebt {
				advance_rate: Rate::from_float(1.0),
			});

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				AMOUNT / 2
			));

			write_off_loan(loan_id);

			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, AMOUNT / 2),
				Error::<Runtime>::from(BorrowLoanError::WrittenOffRestriction)
			);
		});
	}
}
