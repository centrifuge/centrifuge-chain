mod base;

use frame_support::{assert_noop, assert_ok};

use super::{mock::*, *};

pub const GROUP_A: u32 = 1;
pub const GROUP_B: u32 = 2;

pub const DOM_1_CURRENCY_A: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::A);
pub const DOM_1_CURRENCY_B: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::B);
pub const DOM_1_CURRENCY_C: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::C);

pub const REWARD: u64 = 100;

// -------------------------------------------------------------
//  Common tests that not make use any mechanism under the hood
// -------------------------------------------------------------

use super::mock::RewardsMockedMechanism as Rewards;

#[test]
fn stake_insufficient_balance() {
	let _m = mechanism::mock::lock();

	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
		assert_noop!(
			Rewards::deposit_stake(DOM_1_CURRENCY_A, &USER_A, USER_INITIAL_BALANCE + 1),
			TokenError::NoFunds
		);
	});
}

#[test]
fn use_currency_without_group() {
	let _m = mechanism::mock::lock();

	new_test_ext().execute_with(|| {
		assert_noop!(
			Rewards::deposit_stake(DOM_1_CURRENCY_A, &USER_A, 0),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Rewards::withdraw_stake(DOM_1_CURRENCY_A, &USER_A, 0),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Rewards::compute_reward(DOM_1_CURRENCY_A, &USER_A),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Rewards::claim_reward(DOM_1_CURRENCY_A, &USER_A),
			Error::<Test>::CurrencyWithoutGroup
		);
	});
}

#[test]
fn move_currency_same_group_error() {
	let _m = mechanism::mock::lock();

	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
		assert_noop!(
			Rewards::attach_currency(DOM_1_CURRENCY_A, GROUP_A),
			Error::<Test>::CurrencyInSameGroup
		);
	});
}
