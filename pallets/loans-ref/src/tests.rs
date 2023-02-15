use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{
	assert_ok,
	traits::tokens::nonfungibles::{Create, Mutate},
};

use super::{
	mock::*,
	types::{
		BorrowRestrictions, InterestPayments, LoanRestrictions, Maturity, MaxBorrowAmount,
		PayDownSchedule, RepayRestrictions, RepaymentSchedule,
	},
	valuation::ValuationMethod,
};

const ASSET_COLLECTION_OWNER: AccountId = 1;
const COLLECTION_A: CollectionId = 2;
const ITEM_A: ItemId = 3;
const BORROWER: AccountId = 2;
const POOL_A: PoolId = 1;
const POOL_A_ACCOUNT: AccountId = 11;

#[test]
fn create_loan() {
	new_test_ext().execute_with(|| {
		Time::set_timestamp(BLOCK_TIME);

		Uniques::create_collection(&COLLECTION_A, &BORROWER, &ASSET_COLLECTION_OWNER).unwrap();
		Uniques::mint_into(&COLLECTION_A, &ITEM_A, &BORROWER).unwrap();

		MockPermissions::expect_has(|scope, who, role| {
			matches!(scope, PermissionScope::Pool(POOL_A));
			assert_eq!(who, BORROWER);
			matches!(role, Role::PoolRole(PoolRole::Borrower));
			true
		});

		MockPools::expect_pool_exists(|pool_id| {
			assert_eq!(pool_id, POOL_A);
			true
		});

		MockPools::expect_account_for(|pool_id| {
			assert_eq!(pool_id, POOL_A);
			POOL_A_ACCOUNT
		});

		assert_ok!(Loans::create(
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
					advance_rate: Rate::from_float(0.5)
				},
				borrows: BorrowRestrictions::WrittenOff,
				repayments: RepayRestrictions::None,
			},
			Rate::from_float(0.03)
		));
	});
}
