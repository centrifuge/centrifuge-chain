use crate::mock::*;
use common_traits::fees::{Fee, Fees as FeesTrait};
use frame_support::{assert_noop, assert_ok};

const FEE_KEY: TestFeeKey = TestFeeKey::Key1;
const FEE_VALUE: u64 = 42;

fn set_default_fee() {
	assert_ok!(Fees::set_fee(
		Origin::signed(Admin::get()),
		TestFeeKey::Key1,
		FEE_VALUE
	));
}

#[test]
fn fee_was_never_set() {
	new_test_ext().execute_with(|| {
		assert!(Fees::fee(FEE_KEY).is_none());
		assert_eq!(Fees::fee_value(FEE_KEY), 0);
	});
}

#[test]
fn fee_is_set() {
	new_test_ext().execute_with(|| {
		set_default_fee();

		assert_eq!(Fees::fee(FEE_KEY), Some(FEE_VALUE));
		assert_eq!(Fees::fee_value(FEE_KEY), FEE_VALUE);
	});
}

#[test]
fn fee_to_author() {
	new_test_ext().execute_with(|| {
		set_default_fee();

		assert_ok!(Fees::fee_to_author(&USER_ACCOUNT, Fee::Key(FEE_KEY)));

		let author_balance = Balances::free_balance(&Authorship::author().unwrap());
		let user_balance = Balances::free_balance(&USER_ACCOUNT);

		assert_eq!(author_balance, FEE_VALUE);
		assert_eq!(user_balance, USER_INITIAL_BALANCE - FEE_VALUE);

		// Try to perform an action over an user with insufficient balance.
		assert_noop!(
			Fees::fee_to_author(&USER_ACCOUNT, Fee::Key(FEE_KEY)),
			pallet_balances::Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn fee_to_treasury() {
	new_test_ext().execute_with(|| {
		set_default_fee();

		assert_ok!(Fees::fee_to_treasury(&USER_ACCOUNT, Fee::Key(FEE_KEY)));

		let treasury_balance = Balances::free_balance(&Treasury::account_id());
		let user_balance = Balances::free_balance(&USER_ACCOUNT);

		assert_eq!(treasury_balance, FEE_VALUE);
		assert_eq!(user_balance, USER_INITIAL_BALANCE - FEE_VALUE);

		// Try to perform an action over an user with insufficient balance.
		assert_noop!(
			Fees::fee_to_treasury(&USER_ACCOUNT, Fee::Key(FEE_KEY)),
			pallet_balances::Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn fee_to_burn() {
	new_test_ext().execute_with(|| {
		set_default_fee();

		assert_ok!(Fees::fee_to_burn(&USER_ACCOUNT, Fee::Key(FEE_KEY)));

		let user_balance = Balances::free_balance(&USER_ACCOUNT);

		assert_eq!(user_balance, USER_INITIAL_BALANCE - FEE_VALUE);

		// Try to perform an action over an user with insufficient balance.
		assert_noop!(
			Fees::fee_to_burn(&USER_ACCOUNT, Fee::Key(FEE_KEY)),
			pallet_balances::Error::<Test>::InsufficientBalance
		);
	});
}
