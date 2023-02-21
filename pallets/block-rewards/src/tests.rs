use cfg_types::tokens::CurrencyId;
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use sp_runtime::traits::BadOrigin;

use super::*;
use crate::mock::*;

const USER_A: u64 = 2;

const GROUP_COLLATORS: u32 = 1;
const REWARD: u64 = 100;

#[test]
fn check_special_privileges() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			BlockRewards::set_collator_reward(RuntimeOrigin::signed(USER_A), 10),
			BadOrigin
		);
		assert_noop!(
			BlockRewards::set_total_reward(RuntimeOrigin::signed(USER_A), 100),
			BadOrigin
		);
	});
}

#[test]
fn collator_reward_change() {
	ExtBuilder::default().build().execute_with(|| {
		// EPOCH 0
		assert_ok!(BlockRewards::set_collator_reward(
			RuntimeOrigin::root(),
			REWARD
		));
		assert_eq!(
			NextEpochChanges::<Test>::get().collator_reward,
			Some(REWARD)
		);
		assert_eq!(ActiveEpochData::<Test>::get().collator_reward, 0);

		advance_session();

		// EPOCH 1
		assert_eq!(NextEpochChanges::<Test>::get().collator_reward, None);
		assert_eq!(ActiveEpochData::<Test>::get().collator_reward, REWARD);

		advance_session();

		// EPOCH 2
		assert_eq!(ActiveEpochData::<Test>::get().collator_reward, REWARD);
	});
}

#[test]
fn total_reward_change_isolated() {
	ExtBuilder::default().build().execute_with(|| {
		// EPOCH 0
		assert_ok!(BlockRewards::set_total_reward(
			RuntimeOrigin::root(),
			REWARD
		));
		assert_eq!(NextEpochChanges::<Test>::get().total_reward, Some(REWARD));
		assert_eq!(ActiveEpochData::<Test>::get().total_reward, 0);

		advance_session();

		// EPOCH 1
		assert_eq!(NextEpochChanges::<Test>::get().total_reward, None);
		assert_eq!(ActiveEpochData::<Test>::get().total_reward, REWARD);

		advance_session();

		// EPOCH 2
		assert_eq!(ActiveEpochData::<Test>::get().total_reward, REWARD);
	});
}

#[test]
fn total_reward_change_over_epochs() {
	ExtBuilder::default().build().execute_with(|| {
		// EPOCH 0
		assert_ok!(BlockRewards::set_collator_reward(
			RuntimeOrigin::root(),
			REWARD
		));
		assert_ok!(BlockRewards::set_total_reward(
			RuntimeOrigin::root(),
			REWARD
		));
		assert_eq!(
			NextEpochChanges::<Test>::get().collator_reward,
			Some(REWARD)
		);
		assert_eq!(ActiveEpochData::<Test>::get().collator_reward, 0);
		assert_eq!(NextEpochChanges::<Test>::get().total_reward, Some(REWARD));
		assert_eq!(ActiveEpochData::<Test>::get().total_reward, 0);

		advance_session();

		// EPOCH 1
		assert_eq!(NextEpochChanges::<Test>::get().collator_reward, None);
		assert_eq!(ActiveEpochData::<Test>::get().collator_reward, REWARD);
		assert_eq!(NextEpochChanges::<Test>::get().total_reward, None);
		assert_eq!(ActiveEpochData::<Test>::get().total_reward, REWARD);

		// Total reward update must be at least 2 * collator_reward since collator size increases by one
		assert_eq!(ActiveEpochData::<Test>::get().num_collators, 1);
		assert_eq!(NextEpochChanges::<Test>::get().num_collators, Some(2));
		assert_noop!(
			BlockRewards::set_total_reward(RuntimeOrigin::root(), 2 * REWARD - 1),
			Error::<Test>::InsufficientTotalReward
		);
		assert_ok!(BlockRewards::set_total_reward(
			RuntimeOrigin::root(),
			2 * REWARD
		));
		assert_eq!(
			NextEpochChanges::<Test>::get().total_reward,
			Some(2 * REWARD)
		);

		advance_session();

		// EPOCH 2
		assert_eq!(NextEpochChanges::<Test>::get().collator_reward, None);
		assert_eq!(ActiveEpochData::<Test>::get().collator_reward, REWARD);
		assert_eq!(NextEpochChanges::<Test>::get().total_reward, None);
		assert_eq!(ActiveEpochData::<Test>::get().total_reward, 2 * REWARD);

		// Total reward update must be at least 3 * collator_reward since collator size increases by one
		assert_eq!(ActiveEpochData::<Test>::get().num_collators, 2);
		assert_eq!(NextEpochChanges::<Test>::get().num_collators, Some(3));
		assert_noop!(
			BlockRewards::set_total_reward(RuntimeOrigin::root(), 3 * REWARD - 1),
			Error::<Test>::InsufficientTotalReward
		);
	});
}

#[test]
fn joining_leaving_collators() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(NextEpochChanges::<Test>::get().collators.inc.is_empty());
		assert!(NextEpochChanges::<Test>::get().collators.out.is_empty());
		assert_staked(&1);
		assert_eq!(
			<Test as Config>::Currency::total_issuance(STAKE_CURRENCY_ID),
			DEFAULT_COLLATOR_STAKE as u64
		);

		advance_session();

		// EPOCH 1
		assert_eq!(
			NextEpochChanges::<Test>::get().collators.out.into_inner(),
			vec![1]
		);
		assert_eq!(
			NextEpochChanges::<Test>::get().collators.inc.into_inner(),
			vec![2, 3]
		);
		assert_staked(&1);
		assert_not_staked(&2);
		assert_not_staked(&3);
		assert_eq!(
			<Test as Config>::Currency::total_issuance(STAKE_CURRENCY_ID),
			DEFAULT_COLLATOR_STAKE as u64
		);

		advance_session();

		// EPOCH 2
		assert_eq!(
			NextEpochChanges::<Test>::get().collators.out.into_inner(),
			vec![2]
		);
		assert_eq!(
			NextEpochChanges::<Test>::get().collators.inc.into_inner(),
			vec![4, 5]
		);
		assert_not_staked(&1);
		assert_staked(&2);
		assert_staked(&3);
		assert_not_staked(&4);
		assert_not_staked(&5);
		assert_eq!(
			<Test as Config>::Currency::total_issuance(STAKE_CURRENCY_ID),
			2 * DEFAULT_COLLATOR_STAKE as u64
		);

		advance_session();

		// EPOCH 3
		assert_eq!(
			NextEpochChanges::<Test>::get().collators.out.into_inner(),
			vec![3]
		);
		assert_eq!(
			NextEpochChanges::<Test>::get().collators.inc.into_inner(),
			vec![6, 7]
		);
		assert_not_staked(&2);
		assert_staked(&3);
		assert_staked(&4);
		assert_staked(&5);
		assert_not_staked(&6);
		assert_not_staked(&7);
		assert_eq!(
			<Test as Config>::Currency::total_issuance(STAKE_CURRENCY_ID),
			3 * DEFAULT_COLLATOR_STAKE as u64
		);
	});
}

#[test]
fn single_claim_reward() {
	ExtBuilder::default()
		.set_collator_reward(REWARD)
		.set_total_reward(10 * REWARD)
		.build()
		.execute_with(|| {
			assert!(<Test as Config>::Rewards::is_ready(COLLATOR_GROUP_ID));
			assert_eq!(
				<Test as Config>::Rewards::group_stake(COLLATOR_GROUP_ID),
				DEFAULT_COLLATOR_STAKE as u64
			);
			assert_eq!(ActiveEpochData::<Test>::get().collator_reward, REWARD);
			assert_eq!(ActiveEpochData::<Test>::get().total_reward, 10 * REWARD);
			assert_eq!(mock::RewardRemainderUnbalanced::get(), 0);

			// Advance two epochs for rewards to be non-zero
			advance_session();

			// EPOCH 1 as one collator
			assert_eq!(
				<Test as Config>::Rewards::group_stake(COLLATOR_GROUP_ID),
				1 * DEFAULT_COLLATOR_STAKE as u64
			);
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					(<Test as Config>::Domain::get(), STAKE_CURRENCY_ID),
					&1
				),
				Ok(REWARD)
			);
			assert_ok!(BlockRewards::claim_reward(RuntimeOrigin::signed(2), 1));
			System::assert_last_event(mock::RuntimeEvent::Rewards(
				pallet_rewards::Event::RewardClaimed {
					group_id: COLLATOR_GROUP_ID,
					domain_id: <Test as Config>::Domain::get(),
					currency_id: STAKE_CURRENCY_ID,
					account_id: 1,
					amount: REWARD,
				},
			));
			// Only non-treasury rewards are taken into account
			assert_eq!(Tokens::total_issuance(CurrencyId::Native), REWARD);
			assert_eq!(Balances::total_balance(&TREASURY_ADDRESS), 9 * REWARD);
			assert_eq!(Balances::total_issuance(), 9 * REWARD);
			assert_ok!(Tokens::ensure_can_withdraw(CurrencyId::Native, &1, REWARD));

			// TODO: Maybe in another test
			// Re-claiming possible but amount is zero
			assert_ok!(BlockRewards::claim_reward(RuntimeOrigin::signed(3), 1));
			System::assert_last_event(mock::RuntimeEvent::Rewards(
				pallet_rewards::Event::RewardClaimed {
					group_id: COLLATOR_GROUP_ID,
					domain_id: <Test as Config>::Domain::get(),
					currency_id: STAKE_CURRENCY_ID,
					account_id: 1,
					amount: 0,
				},
			));
		});
}

// TODO: Remainder distribution for total_reward < n * collator_reward
