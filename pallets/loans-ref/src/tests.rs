use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

use super::{
	mock::*,
	types::{CreateLoanError, LoanInfo},
	valuation::{DiscountedCashFlows, ValuationMethod},
	Error,
};

fn mock_expectations_for_create(pool_id: PoolId) {
	MockPermissions::expect_has(move |scope, who, role| {
		let valid = matches!(scope, PermissionScope::Pool(id) if pool_id == id)
			&& matches!(role, Role::PoolRole(PoolRole::Borrower))
			&& who == BORROWER;

		valid
	});
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
fn create_loan_with_success() {
	new_test_ext().execute_with(|| {
		mock_expectations_for_create(POOL_A);

		let loan = LoanInfo::new(ASSET_AA).with_maturity(Time::now());
		assert_ok!(Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan));
	});
}

#[test]
fn create_loan_with_wrong_permissions() {
	new_test_ext().execute_with(|| {
		mock_expectations_for_create(POOL_A);

		let loan = LoanInfo::new(ASSET_AA).with_maturity(Time::now());
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(NO_BORROWER), POOL_A, loan),
			BadOrigin
		);
	});
}

#[test]
fn create_loan_with_wrong_pool() {
	new_test_ext().execute_with(|| {
		mock_expectations_for_create(POOL_B);

		let loan = LoanInfo::new(ASSET_AA).with_maturity(Time::now());
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_B, loan),
			Error::<Runtime>::PoolNotFound
		);
	});
}

#[test]
fn create_loan_with_wrong_assets() {
	new_test_ext().execute_with(|| {
		mock_expectations_for_create(POOL_A);

		let loan = LoanInfo::new(NO_ASSET).with_maturity(Time::now());
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::NFTOwnerNotFound
		);

		let loan = LoanInfo::new(ASSET_AB).with_maturity(Time::now());
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::NotNFTOwner
		);

		let loan = LoanInfo::new(ASSET_AA).with_maturity(Time::now());
		assert_ok!(Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan));

		// Using the same NFT no longer works, because the pool owns it.
		let loan = LoanInfo::new(ASSET_AA).with_maturity(Time::now());
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::NotNFTOwner
		);
	});
}

#[test]
fn create_loan_with_wrong_schedule() {
	new_test_ext().execute_with(|| {
		mock_expectations_for_create(POOL_A);

		let loan = LoanInfo::new(ASSET_AA).with_maturity(Time::now() - SLOT_MS);
		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::from(CreateLoanError::InvalidRepaymentSchedule)
		);
	});
}

#[test]
fn create_loan_with_wrong_valuation() {
	new_test_ext().execute_with(|| {
		mock_expectations_for_create(POOL_A);

		let loan = LoanInfo::new(ASSET_AA)
			.with_maturity(Time::now())
			.with_valuation_method(ValuationMethod::DiscountedCashFlows(
				DiscountedCashFlows::default().with_discount_rate(Rate::from_float(0.9)),
			));

		assert_noop!(
			Loans::create(RuntimeOrigin::signed(BORROWER), POOL_A, loan),
			Error::<Runtime>::from(CreateLoanError::InvalidValuationMethod)
		);
	});
}
