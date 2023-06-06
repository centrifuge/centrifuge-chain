use cfg_types::tokens::{CurrencyId, StakingCurrency};
use frame_support::{assert_noop, assert_ok, traits::fungibles};
use sp_runtime::traits::BadOrigin;

use super::*;
use crate::mock::*;

const REWARD: u128 = 100;

#[test]
fn check_special_privileges() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			BlockRewards::set_collator_reward(RuntimeOrigin::signed(2), 10),
			BadOrigin
		);
		assert_noop!(
			BlockRewards::set_total_reward(RuntimeOrigin::signed(2), 100),
			BadOrigin
		);
	});
}

#[test]
fn collator_reward_change() {
	ExtBuilder::default()
		.set_total_reward(REWARD)
		.build()
		.execute_with(|| {
			// EPOCH 0
			assert_ok!(BlockRewards::set_collator_reward(
				RuntimeOrigin::root(),
				REWARD
			));
			assert_eq!(
				NextSessionChanges::<Test>::get().collator_reward,
				Some(REWARD)
			);
			assert_eq!(ActiveSessionData::<Test>::get().collator_reward, 0);

			advance_session();

			// EPOCH 1
			assert_eq!(NextSessionChanges::<Test>::get().collator_reward, None);
			assert_eq!(ActiveSessionData::<Test>::get().collator_reward, REWARD);

			advance_session();

			// EPOCH 2
			assert_eq!(ActiveSessionData::<Test>::get().collator_reward, REWARD);
		});
}

#[test]
fn collator_reward_change_throws() {
	ExtBuilder::default()
		.set_total_reward(1)
		.set_collator_reward(0)
		.build()
		.execute_with(|| {
			assert_ok!(BlockRewards::set_collator_reward(RuntimeOrigin::root(), 1));
			assert_noop!(
				BlockRewards::set_collator_reward(RuntimeOrigin::root(), 2),
				Error::<Test>::InsufficientTotalReward
			);
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
		assert_eq!(NextSessionChanges::<Test>::get().total_reward, Some(REWARD));
		assert_eq!(ActiveSessionData::<Test>::get().total_reward, 0);

		advance_session();

		// EPOCH 1
		assert_eq!(NextSessionChanges::<Test>::get().total_reward, None);
		assert_eq!(ActiveSessionData::<Test>::get().total_reward, REWARD);

		advance_session();

		// EPOCH 2
		assert_eq!(ActiveSessionData::<Test>::get().total_reward, REWARD);
	});
}

#[test]
fn total_reward_change_over_sessions() {
	ExtBuilder::default()
		.set_total_reward(REWARD)
		.build()
		.execute_with(|| {
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
				NextSessionChanges::<Test>::get().collator_reward,
				Some(REWARD)
			);
			assert_eq!(ActiveSessionData::<Test>::get().collator_reward, 0);
			assert_eq!(NextSessionChanges::<Test>::get().total_reward, Some(REWARD));
			assert_eq!(ActiveSessionData::<Test>::get().total_reward, REWARD);

			advance_session();

			// EPOCH 1
			assert_eq!(NextSessionChanges::<Test>::get().collator_reward, None);
			assert_eq!(ActiveSessionData::<Test>::get().collator_reward, REWARD);
			assert_eq!(NextSessionChanges::<Test>::get().total_reward, None);
			assert_eq!(ActiveSessionData::<Test>::get().total_reward, REWARD);

			// Total reward update must be at least 2 * collator_reward since collator size
			// increases by one
			assert_eq!(ActiveSessionData::<Test>::get().collator_count, 1);
			assert_eq!(NextSessionChanges::<Test>::get().collator_count, Some(2));
			assert_noop!(
				BlockRewards::set_total_reward(RuntimeOrigin::root(), 2 * REWARD - 1),
				Error::<Test>::InsufficientTotalReward
			);
			assert_ok!(BlockRewards::set_total_reward(
				RuntimeOrigin::root(),
				2 * REWARD
			));
			assert_eq!(
				NextSessionChanges::<Test>::get().total_reward,
				Some(2 * REWARD)
			);

			advance_session();

			// EPOCH 2
			assert_eq!(NextSessionChanges::<Test>::get().collator_reward, None);
			assert_eq!(ActiveSessionData::<Test>::get().collator_reward, REWARD);
			assert_eq!(NextSessionChanges::<Test>::get().total_reward, None);
			assert_eq!(ActiveSessionData::<Test>::get().total_reward, 2 * REWARD);

			// Total reward update must be at least 3 * collator_reward since collator size
			// increases by one
			assert_eq!(ActiveSessionData::<Test>::get().collator_count, 2);
			assert_eq!(NextSessionChanges::<Test>::get().collator_count, Some(3));
			assert_noop!(
				BlockRewards::set_total_reward(RuntimeOrigin::root(), 3 * REWARD - 1),
				Error::<Test>::InsufficientTotalReward
			);
		});
}

#[test]
fn joining_leaving_collators() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(NextSessionChanges::<Test>::get().collators.inc.is_empty());
		assert!(NextSessionChanges::<Test>::get().collators.out.is_empty());
		assert_staked(&1);
		assert_eq!(
			<Tokens as fungibles::Inspect<AccountId>>::total_issuance(CurrencyId::Staking(
				StakingCurrency::BlockRewards
			)),
			<Test as Config>::StakeAmount::get() as u128
		);

		advance_session();

		// EPOCH 1
		assert_eq!(
			NextSessionChanges::<Test>::get().collators.out.into_inner(),
			vec![1]
		);
		assert_eq!(
			NextSessionChanges::<Test>::get().collators.inc.into_inner(),
			vec![2, 3]
		);
		assert_staked(&1);
		assert_not_staked(&2);
		assert_not_staked(&3);
		assert_eq!(
			<Tokens as fungibles::Inspect::<AccountId>>::total_issuance(CurrencyId::Staking(
				StakingCurrency::BlockRewards
			)),
			<Test as Config>::StakeAmount::get() as u128
		);

		advance_session();

		// EPOCH 2
		assert_eq!(
			NextSessionChanges::<Test>::get().collators.out.into_inner(),
			vec![2]
		);
		assert_eq!(
			NextSessionChanges::<Test>::get().collators.inc.into_inner(),
			vec![4, 5]
		);
		assert_not_staked(&1);
		assert_staked(&2);
		assert_staked(&3);
		assert_not_staked(&4);
		assert_not_staked(&5);
		assert_eq!(
			<Tokens as fungibles::Inspect::<AccountId>>::total_issuance(CurrencyId::Staking(
				StakingCurrency::BlockRewards
			)),
			2 * <Test as Config>::StakeAmount::get() as u128
		);

		advance_session();

		// EPOCH 3
		assert_eq!(
			NextSessionChanges::<Test>::get().collators.out.into_inner(),
			vec![3]
		);
		assert_eq!(
			NextSessionChanges::<Test>::get().collators.inc.into_inner(),
			vec![6, 7]
		);
		assert_not_staked(&2);
		assert_staked(&3);
		assert_staked(&4);
		assert_staked(&5);
		assert_not_staked(&6);
		assert_not_staked(&7);
		assert_eq!(
			<Tokens as fungibles::Inspect::<AccountId>>::total_issuance(CurrencyId::Staking(
				StakingCurrency::BlockRewards
			)),
			3 * <Test as Config>::StakeAmount::get() as u128
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
			assert!(<Test as Config>::Rewards::is_ready(
				<Test as Config>::StakeGroupId::get()
			));
			assert_eq!(
				<Test as Config>::Rewards::group_stake(<Test as Config>::StakeGroupId::get()),
				<Test as Config>::StakeAmount::get() as u128
			);
			assert_eq!(ActiveSessionData::<Test>::get().collator_reward, REWARD);
			assert_eq!(ActiveSessionData::<Test>::get().total_reward, 10 * REWARD);
			assert_eq!(mock::RewardRemainderUnbalanced::get(), 0);

			// EPOCH 0 -> EPOCH 1
			advance_session();

			// EPOCH 1 has one collator
			assert_eq!(
				<Test as Config>::Rewards::group_stake(<Test as Config>::StakeGroupId::get()),
				1 * <Test as Config>::StakeAmount::get() as u128
			);
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&1
				),
				Ok(REWARD)
			);
			assert_ok!(BlockRewards::claim_reward(RuntimeOrigin::signed(2), 1));
			System::assert_last_event(mock::RuntimeEvent::Rewards(
				pallet_rewards::Event::RewardClaimed {
					group_id: <Test as Config>::StakeGroupId::get(),
					currency_id: <Test as Config>::StakeCurrencyId::get(),
					account_id: 1,
					amount: REWARD,
				},
			));
			assert_eq!(Balances::total_balance(&TREASURY_ADDRESS), 9 * REWARD);
			assert_eq!(
				Balances::total_issuance(),
				10 * REWARD + ExistentialDeposit::get()
			);
			assert_eq!(Balances::free_balance(&1), REWARD);
		});
}

#[test]
fn collator_rewards_greater_than_remainder() {
	ExtBuilder::default()
		.set_collator_reward(REWARD)
		.set_total_reward(2 * REWARD)
		.build()
		.execute_with(|| {
			// EPOCH 0 -> EPOCH 1
			advance_session();

			// EPOCH 0 had one collator [1].
			// Thus, equal distribution of total_reward to collator and Treasury.
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&1
				),
				Ok(REWARD)
			);
			assert_eq!(
				Balances::total_issuance(),
				2 * REWARD + ExistentialDeposit::get()
			);
			assert_eq!(Balances::total_balance(&TREASURY_ADDRESS), REWARD);

			// EPOCH 1 -> EPOCH 2
			advance_session();

			// EPOCH 1 had one collator [1].
			// Thus, equal distribution of total_reward to collator and Treasury.
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&1
				),
				Ok(2 * REWARD)
			);
			assert_eq!(Balances::total_balance(&TREASURY_ADDRESS), 2 * REWARD);
			assert_eq!(
				Balances::total_issuance(),
				4 * REWARD + ExistentialDeposit::get()
			);

			// EPOCH 2 -> EPOCH 3
			advance_session();

			// EPOCH 2 had two collators [2, 3].
			// Thus, both consume the entire total_reward.
			// Additionally, 1 should not have higher claimable reward.
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&1
				),
				Ok(2 * REWARD)
			);
			for collator in [2, 3].iter() {
				assert_eq!(
					<Test as Config>::Rewards::compute_reward(
						<Test as Config>::StakeCurrencyId::get(),
						collator
					),
					Ok(REWARD)
				);
			}
			assert_eq!(Balances::total_balance(&TREASURY_ADDRESS), 2 * REWARD);
			assert_eq!(
				Balances::total_issuance(),
				6 * REWARD + ExistentialDeposit::get()
			);

			// EPOCH 3 -> EPOCH 4
			advance_session();

			// EPOCH 3 had three collators [3, 4, 5].
			// Thus, all three consume the entire total_reward
			// and reseive less than collator_reward each.
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&3
				),
				Ok(REWARD + 2 * REWARD / 3)
			);
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&4
				),
				Ok(2 * REWARD / 3)
			);
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&5
				),
				Ok(2 * REWARD / 3)
			);
			assert_eq!(Balances::total_balance(&TREASURY_ADDRESS), 2 * REWARD);
			assert_eq!(
				Balances::total_issuance(),
				8 * REWARD + ExistentialDeposit::get()
			);
		});
}

#[test]
fn late_claiming_works() {
	ExtBuilder::default()
		.set_collator_reward(REWARD)
		.set_total_reward(2 * REWARD)
		.set_run_to_block(100)
		.build()
		.execute_with(|| {
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&2
				),
				Ok(REWARD)
			);
			assert_ok!(BlockRewards::claim_reward(RuntimeOrigin::signed(1), 2));
			System::assert_last_event(mock::RuntimeEvent::Rewards(
				pallet_rewards::Event::RewardClaimed {
					group_id: <Test as Config>::StakeGroupId::get(),
					currency_id: <Test as Config>::StakeCurrencyId::get(),
					account_id: 2,
					amount: REWARD,
				},
			));
		});
}

#[test]
fn duplicate_claiming_works_but_ineffective() {
	ExtBuilder::default()
		.set_collator_reward(REWARD)
		.set_total_reward(2 * REWARD)
		.set_run_to_block(100)
		.build()
		.execute_with(|| {
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&2
				),
				Ok(REWARD)
			);
			assert_ok!(BlockRewards::claim_reward(RuntimeOrigin::signed(3), 2));
			System::assert_last_event(mock::RuntimeEvent::Rewards(
				pallet_rewards::Event::RewardClaimed {
					group_id: <Test as Config>::StakeGroupId::get(),
					currency_id: <Test as Config>::StakeCurrencyId::get(),
					account_id: 2,
					amount: REWARD,
				},
			));

			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&2
				),
				Ok(0)
			);
			assert_ok!(BlockRewards::claim_reward(RuntimeOrigin::signed(1), 2));
			System::assert_last_event(mock::RuntimeEvent::Rewards(
				pallet_rewards::Event::RewardClaimed {
					group_id: <Test as Config>::StakeGroupId::get(),
					currency_id: <Test as Config>::StakeCurrencyId::get(),
					account_id: 2,
					amount: 0,
				},
			));
		});
}
