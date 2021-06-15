use crate::mock::*;
use frame_support::{assert_noop, assert_ok, dispatch::DispatchError};
use sp_runtime::traits::{BadOrigin, Hash};

#[test]
fn can_change_fee() {
	new_test_ext().execute_with(|| {
		assert_noop!(Fees::can_change_fee(Origin::signed(2)), BadOrigin);
		assert_ok!(Fees::can_change_fee(Origin::signed(1)));
	});
}

#[test]
fn multiple_new_fees_are_setable() {
	new_test_ext().execute_with(|| {
		let fee_key1 = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let fee_key2 = <Test as frame_system::Config>::Hashing::hash_of(&2);

		let price1: <Test as pallet_balances::Config>::Balance = 666;
		let price2: <Test as pallet_balances::Config>::Balance = 777;

		assert_noop!(
			Fees::set_fee(Origin::signed(2), fee_key1, price1),
			BadOrigin
		);
		assert_ok!(Fees::set_fee(Origin::signed(1), fee_key1, price1));
		assert_ok!(Fees::set_fee(Origin::signed(1), fee_key2, price2));

		let loaded_fee1 = Fees::fee(fee_key1).unwrap();
		assert_eq!(loaded_fee1.price, price1);

		let loaded_fee2 = Fees::fee(fee_key2).unwrap();
		assert_eq!(loaded_fee2.price, price2);
	});
}

#[test]
fn fee_is_re_setable() {
	new_test_ext().execute_with(|| {
		let fee_key = <Test as frame_system::Config>::Hashing::hash_of(&1);

		let initial_price: <Test as pallet_balances::Config>::Balance = 666;
		assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, initial_price));

		let loaded_fee = Fees::fee(fee_key).unwrap();
		assert_eq!(loaded_fee.price, initial_price);

		let new_price: <Test as pallet_balances::Config>::Balance = 777;
		assert_noop!(
			Fees::set_fee(Origin::signed(2), fee_key, new_price),
			BadOrigin
		);
		assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, new_price));
		let again_loaded_fee = Fees::fee(fee_key).unwrap();
		assert_eq!(again_loaded_fee.price, new_price);
	});
}

#[test]
fn fee_payment_errors_if_not_set() {
	new_test_ext().execute_with(|| {
		let fee_key = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let fee_price: <Test as pallet_balances::Config>::Balance = 90000;
		let author_old_balance = <pallet_balances::Pallet<Test>>::free_balance(&100);

		assert_noop!(
			Fees::pay_fee(1, fee_key),
			DispatchError::Module {
				index: 3,
				error: 0,
				message: Some("FeeNotFoundForKey"),
			}
		);

		assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

		// initial time paying will succeed as sufficient balance + fee is set
		assert_ok!(Fees::pay_fee(1, fee_key));

		let author_new_balance = <pallet_balances::Pallet<Test>>::free_balance(&100);
		assert_eq!(author_new_balance - author_old_balance, fee_price);

		// second time paying will lead to account having insufficient balance
		assert_noop!(
			Fees::pay_fee(1, fee_key),
			DispatchError::Module {
				index: 2,
				error: 2,
				message: Some("InsufficientBalance"),
			}
		);
	});
}

#[test]
fn fee_payment_errors_if_insufficient_balance() {
	new_test_ext().execute_with(|| {
		let fee_key = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let fee_price: <Test as pallet_balances::Config>::Balance = 90000;

		assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

		// account 3 is not endowed in the test setup
		assert_noop!(
			Fees::pay_fee(3, fee_key),
			DispatchError::Module {
				index: 2,
				error: 2,
				message: Some("InsufficientBalance"),
			}
		);
	});
}

#[test]
fn fee_payment_subtracts_fees_from_account() {
	new_test_ext().execute_with(|| {
		let fee_key = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let fee_price: <Test as pallet_balances::Config>::Balance = 90000;
		assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));

		// account 1 is endowed in test setup
		// initial time paying will succeed as sufficient balance + fee is set
		assert_ok!(Fees::pay_fee(1, fee_key));

		//second time paying will lead to account having insufficient balance
		assert_noop!(
			Fees::pay_fee(1, fee_key),
			DispatchError::Module {
				index: 2,
				error: 2,
				message: Some("InsufficientBalance"),
			}
		);
	});
}

#[test]
fn fee_is_gettable() {
	new_test_ext().execute_with(|| {
		let fee_key = <Test as frame_system::Config>::Hashing::hash_of(&1);
		let fee_price: <Test as pallet_balances::Config>::Balance = 90000;

		//First run, the fee is not set yet and should return None
		match Fees::price_of(fee_key) {
			Some(_x) => assert!(false, "Should not have a fee set yet"),
			None => assert!(true),
		}

		//After setting the fee, the correct fee should be returned
		assert_ok!(Fees::set_fee(Origin::signed(1), fee_key, fee_price));
		//First run, the fee is not set yet and should return None
		match Fees::price_of(fee_key) {
			Some(x) => assert_eq!(fee_price, x),
			None => assert!(false, "Fee should have been set"),
		}
	});
}

#[test]
fn fee_burn_fee_from_account() {
	new_test_ext().execute_with(|| {
		let account_current_balance = <pallet_balances::Pallet<Test>>::free_balance(100);
		let fee_amount: u64 = 10;

		//first time has enough funds to burn
		assert_ok!(Fees::burn_fee(&100, fee_amount));
		let account_new_balance = <pallet_balances::Pallet<Test>>::free_balance(100);
		assert_eq!(account_current_balance - fee_amount, account_new_balance);

		//second time burn will lead to account having insufficient balance
		assert_noop!(
			Fees::burn_fee(&100, account_new_balance + 1),
			DispatchError::Module {
				index: 2,
				error: 2,
				message: Some("InsufficientBalance"),
			}
		);
	});
}
