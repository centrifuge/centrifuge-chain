use cfg_traits::fees::{Fee, Fees as FeesTrait};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

use crate::mock::*;

const FEE_KEY: u8 = 1;
const FEE_VALUE: u64 = 42;

fn set_default_fee() {
	assert_ok!(Fees::set_fee(
		RuntimeOrigin::signed(Admin::get()),
		FEE_KEY,
		FEE_VALUE
	));
}

#[test]
fn ensure_origin() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Fees::set_fee(RuntimeOrigin::signed(2), FEE_KEY, FEE_VALUE),
			BadOrigin
		);
	});
}

#[test]
fn fee_was_never_set() {
	new_test_ext().execute_with(|| {
		assert_eq!(Fees::fee(FEE_KEY), DefaultFeeValue::get());
		assert_eq!(Fees::fee_value(FEE_KEY), DefaultFeeValue::get());
	});
}

#[test]
fn fee_is_set() {
	new_test_ext().execute_with(|| {
		set_default_fee();

		assert_eq!(Fees::fee(FEE_KEY), FEE_VALUE);
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
			pallet_balances::Error::<Runtime>::InsufficientBalance
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
			pallet_balances::Error::<Runtime>::InsufficientBalance
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
			pallet_balances::Error::<Runtime>::InsufficientBalance
		);
	});
}
