use std::time::Duration;

use cfg_mocks::pallet_mock_data::util::MockDataCollection;
use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::{traits::BadOrigin, DispatchError};

use super::{
	loan::{ActiveLoan, LoanInfo},
	mock::*,
	pallet::{ActiveLoans, Error, LastLoanId, PortfolioValuation},
	pricing::{
		external::ExternalPricing,
		internal::{InternalPricing, MaxBorrowAmount},
		ActivePricing, Pricing,
	},
	types::{
		policy::{WriteOffRule, WriteOffStatus, WriteOffTrigger},
		valuation::{DiscountedCashFlow, ValuationMethod},
		BorrowLoanError, BorrowRestrictions, CloseLoanError, CreateLoanError, InterestPayments,
		LoanRestrictions, Maturity, PayDownSchedule, RepayLoanError, RepayRestrictions,
		RepaymentSchedule, WrittenOffError,
	},
};

const COLLATERAL_VALUE: Balance = 10000;
const DEFAULT_INTEREST_RATE: f64 = 0.5;
const POLICY_PERCENTAGE: f64 = 0.5;
const POLICY_PENALTY: f64 = 0.5;
const REGISTER_PRICE_ID: PriceId = 42;
const UNREGISTER_PRICE_ID: PriceId = 88;
const PRICE_VALUE: Balance = 1000;
const QUANTITY: Balance = 20;

/// Used where the error comes from other pallet impl. unknown from the tests
const DEPENDENCY_ERROR: DispatchError = DispatchError::Other("dependency error");

mod util {
	use super::*;

	pub fn total_borrowed_rate(value: f64) -> MaxBorrowAmount<Rate> {
		MaxBorrowAmount::UpToTotalBorrowed {
			advance_rate: Rate::from_float(value),
		}
	}

	pub fn outstanding_debt_rate(value: f64) -> MaxBorrowAmount<Rate> {
		MaxBorrowAmount::UpToOutstandingDebt {
			advance_rate: Rate::from_float(value),
		}
	}

	pub fn get_loan(loan_id: LoanId) -> ActiveLoan<Runtime> {
		ActiveLoans::<Runtime>::get(POOL_A)
			.into_iter()
			.find(|(id, _)| *id == loan_id)
			.unwrap()
			.1
	}

	pub fn current_loan_debt(loan_id: LoanId) -> Balance {
		match get_loan(loan_id).pricing() {
			ActivePricing::Internal(pricing) => pricing.calculate_debt().unwrap(),
			ActivePricing::External(pricing) => pricing.calculate_debt().unwrap(),
		}
	}

	pub fn current_loan_pv(loan_id: LoanId) -> Balance {
		get_loan(loan_id).present_value().unwrap()
	}

	pub fn interest_for(rate: f64, elapsed: Duration) -> f64 {
		(1.0 + rate / YEAR.as_secs() as f64).powi(elapsed.as_secs() as i32)
	}

	pub fn current_debt_for(interest: f64, balance: Balance) -> Balance {
		(interest * balance as f64) as Balance
	}

	pub fn set_up_policy(percentage: f64, penalty: f64) {
		MockPermissions::mock_has(|_, _, _| true);
		MockPools::mock_pool_exists(|_| true);

		Loans::update_write_off_policy(
			RuntimeOrigin::signed(0),
			POOL_A,
			vec![WriteOffRule::new(
				[WriteOffTrigger::PrincipalOverdueDays(1)],
				Rate::from_float(percentage),
				Rate::from_float(penalty),
			)]
			.try_into()
			.unwrap(),
		)
		.expect("successful policy");

		MockPermissions::mock_has(|_, _, _| panic!("no has() mock"));
		MockPools::mock_pool_exists(|_| panic!("no pool_exists() mock"));
	}

	pub fn base_internal_pricing() -> InternalPricing<Runtime> {
		InternalPricing {
			collateral_value: COLLATERAL_VALUE,
			interest_rate: Rate::from_float(DEFAULT_INTEREST_RATE),
			max_borrow_amount: util::total_borrowed_rate(1.0),
			valuation_method: ValuationMethod::OutstandingDebt,
		}
	}

	pub fn base_internal_loan() -> LoanInfo<Runtime> {
		LoanInfo {
			schedule: RepaymentSchedule {
				maturity: Maturity::Fixed((now() + YEAR).as_secs()),
				interest_payments: InterestPayments::None,
				pay_down_schedule: PayDownSchedule::None,
			},
			collateral: ASSET_AA,
			pricing: Pricing::Internal(base_internal_pricing()),
			restrictions: LoanRestrictions {
				borrows: BorrowRestrictions::NotWrittenOff,
				repayments: RepayRestrictions::None,
			},
		}
	}

	pub fn base_external_loan() -> LoanInfo<Runtime> {
		LoanInfo {
			schedule: RepaymentSchedule {
				maturity: Maturity::Fixed((now() + YEAR).as_secs()),
				interest_payments: InterestPayments::None,
				pay_down_schedule: PayDownSchedule::None,
			},
			collateral: ASSET_AA,
			pricing: Pricing::External(ExternalPricing {
				price_id: REGISTER_PRICE_ID,
				max_borrow_quantity: QUANTITY,
			}),
			restrictions: LoanRestrictions {
				borrows: BorrowRestrictions::NotWrittenOff,
				repayments: RepayRestrictions::None,
			},
		}
	}

	pub fn create_loan(loan: LoanInfo<Runtime>) -> LoanId {
		MockPermissions::mock_has(|_, _, _| true);
		MockPools::mock_pool_exists(|_| true);
		MockPools::mock_account_for(|_| POOL_A_ACCOUNT);
		MockPrices::mock_get(|_| Ok((PRICE_VALUE, BLOCK_TIME.as_secs())));

		Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan).expect("successful creation");

		MockPermissions::mock_has(|_, _, _| panic!("no has() mock"));
		MockPools::mock_pool_exists(|_| panic!("no pool_exists() mock"));
		MockPools::mock_account_for(|_| panic!("no account_for() mock"));
		MockPrices::mock_get(|_| panic!("no get() mock"));

		LastLoanId::<Runtime>::get(POOL_A)
	}

	pub fn borrow_loan(loan_id: LoanId, borrow_amount: Balance) {
		MockPools::mock_withdraw(|_, _, _| Ok(()));
		MockPrices::mock_get(|_| Ok((PRICE_VALUE, BLOCK_TIME.as_secs())));
		MockPrices::mock_register_id(|_, _| Ok(()));

		Loans::borrow(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			borrow_amount,
		)
		.expect("successful borrowing");

		MockPools::mock_withdraw(|_, _, _| panic!("no withdraw() mock"));
		MockPrices::mock_get(|_| panic!("no get() mock"));
		MockPrices::mock_register_id(|_, _| panic!("no register_id() mock"));
	}

	pub fn repay_loan(loan_id: LoanId, repay_amount: Balance) {
		MockPools::mock_deposit(|_, _, _| Ok(()));
		MockPrices::mock_get(|_| Ok((PRICE_VALUE, BLOCK_TIME.as_secs())));

		Loans::repay(
			RuntimeOrigin::signed(BORROWER),
			POOL_A,
			loan_id,
			repay_amount,
			0,
		)
		.expect("successful repaying");

		MockPools::mock_deposit(|_, _, _| panic!("no deposit() mock"));
		MockPrices::mock_get(|_| panic!("no get() mock"));
	}

	pub fn write_off_loan(loan_id: LoanId) {
		set_up_policy(POLICY_PERCENTAGE, POLICY_PENALTY);
		MockPrices::mock_get(|_| Ok((PRICE_VALUE, BLOCK_TIME.as_secs())));

		Loans::write_off(RuntimeOrigin::signed(ANY), POOL_A, loan_id)
			.expect("successful write off");

		MockPrices::mock_get(|_| panic!("no get() mock"));
	}

	pub fn close_loan(loan_id: LoanId) {
		MockPrices::mock_unregister_id(|_, _| Ok(()));

		Loans::close(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id)
			.expect("successful clossing");

		MockPrices::mock_get(|_| panic!("no unregister_id() mock"));
	}
}

mod create_loan {
	use super::*;

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
		MockPrices::mock_get(|id| match *id {
			REGISTER_PRICE_ID => Ok((PRICE_VALUE, BLOCK_TIME.as_secs())),
			_ => Err(DEPENDENCY_ERROR),
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
					maturity: Maturity::Fixed(now().as_secs()),
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
				pricing: Pricing::Internal(InternalPricing {
					interest_rate: Rate::from_float(3.0), // Too high
					..util::base_internal_pricing()
				}),
				..util::base_internal_loan()
			};

			assert_noop!(
				Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
				pallet_interest_accrual::Error::<Runtime>::InvalidRate
			);
		});
	}

	#[test]
	fn with_wrong_price_id() {
		new_test_ext().execute_with(|| {
			config_mocks(POOL_A);

			let loan = LoanInfo {
				pricing: Pricing::External(ExternalPricing {
					price_id: UNREGISTER_PRICE_ID,
					max_borrow_quantity: QUANTITY,
				}),
				..util::base_external_loan()
			};

			assert_noop!(
				Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
				DEPENDENCY_ERROR
			);
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
}

mod borrow_loan {
	use super::*;

	fn config_mocks(withdraw_amount: Balance) {
		MockPools::mock_withdraw(move |pool_id, to, amount| {
			assert_eq!(to, BORROWER);
			assert_eq!(pool_id, POOL_A);
			assert_eq!(withdraw_amount, amount);
			Ok(())
		});
		MockPrices::mock_get(|id| {
			assert_eq!(*id, REGISTER_PRICE_ID);
			Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
		});
		MockPrices::mock_register_id(|id, pool_id| {
			assert_eq!(*id, REGISTER_PRICE_ID);
			assert_eq!(*pool_id, POOL_A);
			Ok(())
		});
	}

	#[test]
	fn with_wrong_loan_id() {
		new_test_ext().execute_with(|| {
			config_mocks(COLLATERAL_VALUE);

			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, 0, COLLATERAL_VALUE),
				Error::<Runtime>::LoanNotActiveOrNotFound
			);
		});
	}

	#[test]
	fn from_other_borrower() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());

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
	fn with_restriction_no_written_off() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());

			config_mocks(COLLATERAL_VALUE / 2);
			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));

			advance_time(YEAR + DAY);
			util::write_off_loan(loan_id);

			assert_noop!(
				Loans::borrow(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE / 2
				),
				Error::<Runtime>::from(BorrowLoanError::Restriction)
			);
		});
	}

	#[test]
	fn with_restriction_full_once() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(LoanInfo {
				restrictions: LoanRestrictions {
					borrows: BorrowRestrictions::FullOnce,
					repayments: RepayRestrictions::FullOnce,
				},
				..util::base_internal_loan()
			});

			config_mocks(COLLATERAL_VALUE / 2);
			assert_noop!(
				Loans::borrow(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE / 2 // Must be full value
				),
				Error::<Runtime>::from(BorrowLoanError::Restriction)
			);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE
			));

			// Borrow was already done
			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, 0),
				Error::<Runtime>::from(BorrowLoanError::Restriction)
			);
		});
	}

	#[test]
	fn with_maturity_passed() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());

			advance_time(YEAR);

			config_mocks(COLLATERAL_VALUE);
			assert_noop!(
				Loans::borrow(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE
				),
				Error::<Runtime>::from(BorrowLoanError::MaturityDatePassed)
			);
		});
	}

	#[test]
	fn with_wrong_big_amount_internal_pricing() {
		let borrow_inputs = [
			(COLLATERAL_VALUE + 1, util::total_borrowed_rate(1.0)),
			(COLLATERAL_VALUE / 2 + 1, util::total_borrowed_rate(0.5)),
			(1, util::total_borrowed_rate(0.0)),
			(COLLATERAL_VALUE + 1, util::outstanding_debt_rate(1.0)),
			(COLLATERAL_VALUE / 2 + 1, util::outstanding_debt_rate(0.5)),
			(1, util::outstanding_debt_rate(0.0)),
		];

		for (amount, max_borrow_amount) in borrow_inputs {
			new_test_ext().execute_with(|| {
				let loan_id = util::create_loan(LoanInfo {
					pricing: Pricing::Internal(InternalPricing {
						max_borrow_amount,
						..util::base_internal_pricing()
					}),
					..util::base_internal_loan()
				});

				config_mocks(amount);
				assert_noop!(
					Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, amount),
					Error::<Runtime>::from(BorrowLoanError::MaxAmountExceeded)
				);
			});
		}
	}

	#[test]
	fn with_correct_amount_internal_pricing() {
		let borrow_inputs = [
			(COLLATERAL_VALUE, util::total_borrowed_rate(1.0)),
			(COLLATERAL_VALUE / 2, util::total_borrowed_rate(0.5)),
			(0, util::total_borrowed_rate(0.0)),
			(COLLATERAL_VALUE, util::outstanding_debt_rate(1.0)),
			(COLLATERAL_VALUE / 2, util::outstanding_debt_rate(0.5)),
			(0, util::outstanding_debt_rate(0.0)),
		];

		for (amount, max_borrow_amount) in borrow_inputs {
			new_test_ext().execute_with(|| {
				let loan_id = util::create_loan(LoanInfo {
					pricing: Pricing::Internal(InternalPricing {
						max_borrow_amount,
						..util::base_internal_pricing()
					}),
					..util::base_internal_loan()
				});

				config_mocks(amount);
				assert_ok!(Loans::borrow(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					amount
				));
				assert_eq!(amount, util::current_loan_debt(loan_id));
			});
		}
	}

	#[test]
	fn with_wrong_big_amount_external_pricing() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());

			let amount = PRICE_VALUE * QUANTITY + 1;
			config_mocks(amount);

			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, amount),
				Error::<Runtime>::from(BorrowLoanError::MaxAmountExceeded)
			);
		});
	}

	#[test]
	fn with_wrong_quantity_amount_external_pricing() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());

			// It's not multiple of PRICE_VALUE
			let amount = PRICE_VALUE * QUANTITY - 1;
			config_mocks(amount);

			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, amount),
				Error::<Runtime>::AmountNotMultipleOfPrice
			);
		});
	}

	#[test]
	fn with_correct_amount_external_pricing() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());

			let amount = PRICE_VALUE * QUANTITY;
			config_mocks(amount);

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				amount
			),);
		});
	}

	#[test]
	fn twice() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());

			config_mocks(COLLATERAL_VALUE / 2);

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));
			assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_id));

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));
			assert_eq!(COLLATERAL_VALUE, util::current_loan_debt(loan_id));

			// At this point the loan has been fully borrowed.
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
			let loan_id = util::create_loan(util::base_internal_loan());

			config_mocks(COLLATERAL_VALUE / 2);

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));
			assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_id));

			advance_time(YEAR / 2);

			assert_eq!(
				util::current_debt_for(
					util::interest_for(DEFAULT_INTEREST_RATE, YEAR / 2),
					COLLATERAL_VALUE / 2,
				),
				util::current_loan_debt(loan_id)
			);

			assert_ok!(Loans::borrow(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2
			));

			// At this point the loan has been fully borrowed.
			let extra = 1;
			assert_noop!(
				Loans::borrow(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, extra),
				Error::<Runtime>::from(BorrowLoanError::MaxAmountExceeded)
			);
		});
	}
}

mod repay_loan {
	use super::*;

	pub fn config_mocks(deposit_amount: Balance) {
		MockPools::mock_deposit(move |pool_id, to, amount| {
			assert_eq!(to, BORROWER);
			assert_eq!(pool_id, POOL_A);
			assert_eq!(deposit_amount, amount);
			Ok(())
		});
		MockPrices::mock_get(|id| {
			assert_eq!(*id, REGISTER_PRICE_ID);
			Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
		});
	}

	#[test]
	fn without_borrow_first() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());

			config_mocks(COLLATERAL_VALUE);
			assert_noop!(
				Loans::repay(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE,
					0,
				),
				Error::<Runtime>::LoanNotActiveOrNotFound
			);
		});
	}

	#[test]
	fn with_wrong_loan_id() {
		new_test_ext().execute_with(|| {
			config_mocks(COLLATERAL_VALUE);

			assert_noop!(
				Loans::repay(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					0,
					COLLATERAL_VALUE,
					0
				),
				Error::<Runtime>::LoanNotActiveOrNotFound
			);
		});
	}

	#[test]
	fn from_other_borrower() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE);
			assert_noop!(
				Loans::repay(
					RuntimeOrigin::signed(OTHER_BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE,
					0
				),
				Error::<Runtime>::NotLoanBorrower
			);
		});
	}

	#[test]
	fn has_been_written_off() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			advance_time(YEAR + DAY);
			util::write_off_loan(loan_id);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE,
				0
			));
		});
	}

	#[test]
	fn with_success_partial() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE / 2);

			config_mocks(COLLATERAL_VALUE / 2);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2,
				0
			));
			assert_eq!(0, util::current_loan_debt(loan_id));
		});
	}

	#[test]
	fn with_success_total() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE,
				0
			));
			assert_eq!(0, util::current_loan_debt(loan_id));
		});
	}

	#[test]
	fn with_more_than_required() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE * 2,
				0
			));
		});
	}

	#[test]
	fn with_restriction_full_once() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(LoanInfo {
				restrictions: LoanRestrictions {
					borrows: BorrowRestrictions::FullOnce,
					repayments: RepayRestrictions::FullOnce,
				},
				..util::base_internal_loan()
			});
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE / 2);
			assert_noop!(
				Loans::repay(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					COLLATERAL_VALUE / 2,
					0
				),
				Error::<Runtime>::from(RepayLoanError::Restriction) // Full amount
			);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE,
				0
			));

			let extra = 1;
			config_mocks(0);
			assert_noop!(
				Loans::repay(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id, extra, 0),
				Error::<Runtime>::from(RepayLoanError::Restriction) // Only once
			);
		});
	}

	#[test]
	fn twice() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE / 2);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2,
				0
			));
			assert_eq!(COLLATERAL_VALUE / 2, util::current_loan_debt(loan_id));

			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2,
				0
			));
			assert_eq!(0, util::current_loan_debt(loan_id));

			// At this point the loan has been fully repaid.
			let extra = 1;
			config_mocks(0);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				extra,
				0
			));
		});
	}

	#[test]
	fn twice_with_elapsed_time() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE / 2);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2,
				0
			));

			advance_time(YEAR / 2);

			assert_eq!(
				util::current_debt_for(
					util::interest_for(DEFAULT_INTEREST_RATE, YEAR / 2),
					COLLATERAL_VALUE / 2,
				),
				util::current_loan_debt(loan_id)
			);

			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE / 2,
				0
			));

			// Because of the interest, it has no fully repaid, we need an extra payment.
			let still_to_pay = util::current_loan_debt(loan_id);
			assert_ne!(0, still_to_pay);

			config_mocks(still_to_pay);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				still_to_pay,
				0
			));

			assert_eq!(0, util::current_loan_debt(loan_id));
		});
	}

	#[test]
	fn outstanding_debt_rate_no_increase_if_fully_repaid() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(LoanInfo {
				pricing: Pricing::Internal(InternalPricing {
					max_borrow_amount: util::outstanding_debt_rate(1.0),
					..util::base_internal_pricing()
				}),
				..util::base_internal_loan()
			});
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				COLLATERAL_VALUE,
				0
			));

			advance_time(YEAR);

			assert_eq!(0, util::current_loan_debt(loan_id));
		});
	}

	#[test]
	fn external_pricing_same() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_id, PRICE_VALUE * QUANTITY);

			config_mocks(PRICE_VALUE * QUANTITY);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				PRICE_VALUE * QUANTITY,
				0
			));

			assert_eq!(0, util::current_loan_debt(loan_id));
		});
	}

	#[test]
	fn external_pricing_goes_up() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_id, PRICE_VALUE * QUANTITY);

			config_mocks((PRICE_VALUE * 2) * QUANTITY);
			MockPrices::mock_get(|_| Ok((PRICE_VALUE * 2, now().as_secs())));

			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				(PRICE_VALUE * 2) * QUANTITY,
				0
			));

			assert_eq!(0, util::current_loan_debt(loan_id));
		});
	}

	#[test]
	fn external_pricing_goes_down() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_id, PRICE_VALUE * QUANTITY);

			config_mocks(PRICE_VALUE / 2 * QUANTITY);
			MockPrices::mock_get(|_| Ok((PRICE_VALUE / 2, now().as_secs())));

			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				PRICE_VALUE * QUANTITY,
				0
			));

			assert_eq!(0, util::current_loan_debt(loan_id));
		});
	}

	#[test]
	fn external_pricing_with_wrong_quantity() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_id, PRICE_VALUE * QUANTITY);

			// It's not multiple of PRICE_VALUE
			config_mocks(PRICE_VALUE * QUANTITY - 1);
			assert_noop!(
				Loans::repay(
					RuntimeOrigin::signed(BORROWER),
					POOL_A,
					loan_id,
					PRICE_VALUE * QUANTITY - 1,
					0
				),
				Error::<Runtime>::AmountNotMultipleOfPrice
			);
		});
	}

	#[test]
	fn with_unchecked_repayment() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

			config_mocks(COLLATERAL_VALUE);
			assert_ok!(Loans::repay(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id,
				0,
				COLLATERAL_VALUE,
			),);

			// Nothing repaid with unchecked amount,
			// so I still have the whole amount as debt
			assert_eq!(COLLATERAL_VALUE, util::current_loan_debt(loan_id));
		});
	}
}

mod write_off_loan {
	use super::*;

	fn config_mocks() {
		MockPermissions::mock_has(move |scope, who, role| {
			let valid = matches!(scope, PermissionScope::Pool(id) if id == POOL_A)
				&& matches!(role, Role::PoolRole(PoolRole::LoanAdmin))
				&& who == LOAN_ADMIN;

			valid
		});
	}

	#[test]
	fn without_policy() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
	fn with_no_active_loan() {
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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, PRICE_VALUE * QUANTITY);

			advance_time(YEAR + DAY);

			MockPrices::mock_get(|id| {
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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
			util::borrow_loan(loan_id, COLLATERAL_VALUE);

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
}

mod close_loan {
	use super::*;

	fn config_mocks() {
		MockPrices::mock_unregister_id(|_, _| Ok(()));
	}

	#[test]
	fn with_wrong_loan_id() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Loans::close(RuntimeOrigin::signed(BORROWER), POOL_A, 0),
				Error::<Runtime>::LoanNotActiveOrNotFound
			);
		});
	}

	#[test]
	fn with_wrong_borrower() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());

			assert_noop!(
				Loans::close(RuntimeOrigin::signed(OTHER_BORROWER), POOL_A, loan_id),
				Error::<Runtime>::NotLoanBorrower
			);

			// Make the loan active and ready to be closed
			util::borrow_loan(loan_id, COLLATERAL_VALUE);
			util::repay_loan(loan_id, COLLATERAL_VALUE);

			assert_noop!(
				Loans::close(RuntimeOrigin::signed(OTHER_BORROWER), POOL_A, loan_id),
				Error::<Runtime>::NotLoanBorrower
			);
		});
	}

	#[test]
	fn without_fully_repaid_internal() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);
			util::repay_loan(loan_id, COLLATERAL_VALUE / 2);

			assert_noop!(
				Loans::close(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id),
				Error::<Runtime>::from(CloseLoanError::NotFullyRepaid)
			);
		});
	}

	#[test]
	fn without_fully_repaid_external() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_id, PRICE_VALUE * QUANTITY);
			util::repay_loan(loan_id, (PRICE_VALUE / 2) * QUANTITY);

			assert_noop!(
				Loans::close(RuntimeOrigin::signed(BORROWER), POOL_A, loan_id),
				Error::<Runtime>::from(CloseLoanError::NotFullyRepaid)
			);
		});
	}

	#[test]
	fn with_time_after_fully_repaid_internal() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);
			util::repay_loan(loan_id, COLLATERAL_VALUE);

			advance_time(YEAR);

			assert_ok!(Loans::close(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id
			));

			assert_eq!(Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(), BORROWER);
		});
	}

	#[test]
	fn with_fully_repaid_internal() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_id, COLLATERAL_VALUE);
			util::repay_loan(loan_id, COLLATERAL_VALUE);

			assert_ok!(Loans::close(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id
			));

			assert_eq!(Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(), BORROWER);
		});
	}

	#[test]
	fn with_fully_repaid_external() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_id, PRICE_VALUE * QUANTITY);
			util::repay_loan(loan_id, PRICE_VALUE * QUANTITY);

			config_mocks();
			assert_ok!(Loans::close(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id
			));

			assert_eq!(Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(), BORROWER);
		});
	}

	#[test]
	fn just_created() {
		new_test_ext().execute_with(|| {
			let loan_id = util::create_loan(util::base_internal_loan());

			assert_ok!(Loans::close(
				RuntimeOrigin::signed(BORROWER),
				POOL_A,
				loan_id
			));

			assert_eq!(Uniques::owner(ASSET_AA.0, ASSET_AA.1).unwrap(), BORROWER);
		});
	}
}

mod policy {
	use super::*;

	fn config_mocks(pool_id: PoolId) {
		MockPermissions::mock_has(move |scope, who, role| {
			let valid = matches!(scope, PermissionScope::Pool(id) if pool_id == id)
				&& matches!(role, Role::PoolRole(PoolRole::PoolAdmin))
				&& who == POOL_ADMIN;
			valid
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
			util::borrow_loan(loan_id, PRICE_VALUE * QUANTITY);

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
}

mod portfolio_valuation {
	use super::*;

	fn config_mocks() {
		MockPools::mock_pool_exists(|pool_id| pool_id == POOL_A);
		MockPrices::mock_get(|id| {
			assert_eq!(*id, REGISTER_PRICE_ID);
			Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
		});
		MockPrices::mock_collection(|pool_id| {
			assert_eq!(*pool_id, POOL_A);
			MockDataCollection::new(|id| {
				assert_eq!(*id, REGISTER_PRICE_ID);
				Ok((PRICE_VALUE, BLOCK_TIME.as_secs()))
			})
		});
	}

	fn update_portfolio() {
		config_mocks();
		assert_ok!(Loans::update_portfolio_valuation(
			RuntimeOrigin::signed(ANY),
			POOL_A
		));
	}

	fn expected_portfolio(valuation: Balance) {
		assert_eq!(
			PortfolioValuation::<Runtime>::get(POOL_A).value(),
			valuation
		);
	}

	#[test]
	fn empty() {
		new_test_ext().execute_with(|| {
			expected_portfolio(0);
		});
	}

	#[test]
	fn with_wrong_pool() {
		new_test_ext().execute_with(|| {
			config_mocks();
			assert_noop!(
				Loans::update_portfolio_valuation(RuntimeOrigin::signed(ANY), POOL_B),
				Error::<Runtime>::PoolNotFound
			);
		});
	}

	#[test]
	fn with_no_active_loans() {
		new_test_ext().execute_with(|| {
			util::create_loan(util::base_external_loan());
			util::create_loan(LoanInfo {
				collateral: ASSET_BA,
				..util::base_internal_loan()
			});

			advance_time(YEAR / 2);

			update_portfolio();
			expected_portfolio(0);
		});
	}

	#[test]
	fn with_active_loans() {
		new_test_ext().execute_with(|| {
			let loan_1 = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_1, PRICE_VALUE * QUANTITY);

			let loan_2 = util::create_loan(LoanInfo {
				collateral: ASSET_BA,
				..util::base_internal_loan()
			});
			util::borrow_loan(loan_2, COLLATERAL_VALUE);
			util::repay_loan(loan_2, COLLATERAL_VALUE / 4);

			let valuation = PRICE_VALUE * QUANTITY + COLLATERAL_VALUE - COLLATERAL_VALUE / 4;

			expected_portfolio(valuation);
			update_portfolio();
			expected_portfolio(valuation);

			advance_time(YEAR / 2);

			update_portfolio();
			expected_portfolio(util::current_loan_pv(loan_1) + util::current_loan_pv(loan_2));
		});
	}

	#[test]
	fn with_active_written_off_loans() {
		new_test_ext().execute_with(|| {
			let loan_1 = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_1, PRICE_VALUE * QUANTITY);

			let loan_2 = util::create_loan(LoanInfo {
				collateral: ASSET_BA,
				..util::base_internal_loan()
			});
			util::borrow_loan(loan_2, COLLATERAL_VALUE);
			util::repay_loan(loan_2, COLLATERAL_VALUE / 4);

			advance_time(YEAR + DAY);

			util::write_off_loan(loan_1);
			util::write_off_loan(loan_2);

			update_portfolio();
			expected_portfolio(util::current_loan_pv(loan_1) + util::current_loan_pv(loan_2));
		});
	}

	#[test]
	fn filled_and_cleaned() {
		new_test_ext().execute_with(|| {
			let loan_1 = util::create_loan(util::base_external_loan());
			util::borrow_loan(loan_1, PRICE_VALUE * QUANTITY);

			let loan_2 = util::create_loan(LoanInfo {
				collateral: ASSET_BA,
				..util::base_internal_loan()
			});
			util::borrow_loan(loan_2, COLLATERAL_VALUE);
			util::repay_loan(loan_2, COLLATERAL_VALUE / 4);

			advance_time(YEAR + DAY);

			util::write_off_loan(loan_1);

			advance_time(YEAR / 2);

			util::repay_loan(loan_1, PRICE_VALUE * QUANTITY);
			util::repay_loan(loan_2, COLLATERAL_VALUE * 2);

			advance_time(YEAR / 2);

			update_portfolio();
			expected_portfolio(0);

			util::close_loan(loan_1);
			util::close_loan(loan_2);

			expected_portfolio(0);
		});
	}

	#[test]
	fn exact_and_inexact_matches() {
		new_test_ext().execute_with(|| {
			let loan_1 = util::create_loan(util::base_internal_loan());
			util::borrow_loan(loan_1, COLLATERAL_VALUE);

			advance_time(YEAR / 2);
			update_portfolio();

			// repay_loan() should affect to the portfolio valuation with the same value as
			// the absolute valuation of the loan
			util::repay_loan(loan_1, COLLATERAL_VALUE / 2);
			expected_portfolio(util::current_loan_pv(loan_1));
		});
	}

	#[test]
	fn empty_portfolio_with_current_timestamp() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				PortfolioValuation::<Runtime>::get(POOL_A).last_updated(),
				now().as_secs()
			);
		});
	}
}
