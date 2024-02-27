use cfg_primitives::{CFG, SECONDS_PER_YEAR};
use cfg_types::{
	fixed_point::Rate,
	tokens::{CurrencyId, StakingCurrency},
};
use frame_support::{assert_noop, assert_ok, traits::fungibles};
use num_traits::One;
use sp_runtime::traits::BadOrigin;

use super::*;
use crate::mock::*;

// The Reward amount
// NOTE: This value needs to be > ExistentialDeposit, otherwise the tests will
// fail as it's not allowed to transfer a value below the ED threshold.
const REWARD: u128 = 100 * CFG + ExistentialDeposit::get();

#[test]
fn check_special_privileges() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			BlockRewards::set_collator_reward_per_session(RuntimeOrigin::signed(2), 10),
			BadOrigin
		);
		assert_noop!(
			BlockRewards::set_annual_treasury_inflation_rate(RuntimeOrigin::signed(2), Rate::one()),
			BadOrigin
		);
	});
}

#[test]
fn collator_reward_change() {
	ExtBuilder::default().build().execute_with(|| {
		// EPOCH 0
		assert_ok!(BlockRewards::set_collator_reward_per_session(
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
fn total_reward_change_isolated() {
	ExtBuilder::default().build().execute_with(|| {
		// EPOCH 0
		assert_ok!(BlockRewards::set_annual_treasury_inflation_rate(
			RuntimeOrigin::root(),
			Rate::one()
		));
		assert_eq!(
			NextSessionChanges::<Test>::get().treasury_inflation_rate,
			Some(Rate::one())
		);
		assert_eq!(
			ActiveSessionData::<Test>::get().treasury_inflation_rate,
			Rate::zero()
		);

		advance_session();

		// EPOCH 1
		assert_eq!(
			NextSessionChanges::<Test>::get().treasury_inflation_rate,
			None
		);
		assert_eq!(
			ActiveSessionData::<Test>::get().treasury_inflation_rate,
			Rate::one()
		);

		advance_session();

		// EPOCH 2
		assert_eq!(
			ActiveSessionData::<Test>::get().treasury_inflation_rate,
			Rate::one()
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
			<Test as Config>::StakeAmount::get() as u128 + ExistentialDeposit::get()
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
		assert_not_staked(&2, false);
		assert_not_staked(&3, false);
		assert_eq!(
			<Tokens as fungibles::Inspect::<AccountId>>::total_issuance(CurrencyId::Staking(
				StakingCurrency::BlockRewards
			)),
			<Test as Config>::StakeAmount::get() as u128 + ExistentialDeposit::get()
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
		assert_not_staked(&1, true);
		assert_staked(&2);
		assert_staked(&3);
		assert_not_staked(&4, false);
		assert_not_staked(&5, false);
		assert_eq!(
			<Tokens as fungibles::Inspect::<AccountId>>::total_issuance(CurrencyId::Staking(
				StakingCurrency::BlockRewards
			)),
			2 * <Test as Config>::StakeAmount::get() as u128 + 3 * ExistentialDeposit::get()
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
		assert_not_staked(&2, true);
		assert_staked(&3);
		assert_staked(&4);
		assert_staked(&5);
		assert_not_staked(&6, false);
		assert_not_staked(&7, false);
		assert_eq!(
			<Tokens as fungibles::Inspect::<AccountId>>::total_issuance(CurrencyId::Staking(
				StakingCurrency::BlockRewards
			)),
			3 * <Test as Config>::StakeAmount::get() as u128 + 5 * ExistentialDeposit::get()
		);
	});
}

#[test]
fn single_claim_reward() {
	ExtBuilder::default()
		.set_collator_reward(REWARD)
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
			System::assert_last_event(RuntimeEvent::Rewards(
				pallet_rewards::Event::RewardClaimed {
					group_id: <Test as Config>::StakeGroupId::get(),
					currency_id: <Test as Config>::StakeCurrencyId::get(),
					account_id: 1,
					amount: REWARD,
				},
			));
			// NOTE: Was not set
			assert_eq!(
				Balances::total_balance(&TreasuryPalletId::get().into_account_truncating()),
				0
			);
			assert_eq!(
				Balances::total_issuance(),
				REWARD + ExistentialDeposit::get()
			);
			assert_eq!(Balances::free_balance(&1), REWARD);
		});
}

#[test]
fn collator_rewards_greater_than_remainder() {
	let rate = Rate::saturating_from_rational(1, 10);

	ExtBuilder::default()
		.set_collator_reward(REWARD)
		.set_treasury_inflation_rate(rate)
		.build()
		.execute_with(|| {
			let initial_treasury_balance =
				Balances::free_balance(&TreasuryPalletId::get().into_account_truncating());

			// EPOCH 0 -> EPOCH
			let total_issuance = ExistentialDeposit::get();
			assert_eq!(Balances::total_issuance(), total_issuance);
			MockTime::mock_now(|| SECONDS_PER_YEAR * 1000);
			advance_session();

			// EPOCH 0 had one collator [1].
			// Thus, equal they get all.
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&1
				),
				Ok(REWARD)
			);
			let total_issuance_without_treasury = total_issuance + REWARD;
			let treasury_inflation = total_issuance_without_treasury / 10;
			assert_eq!(
				Balances::total_issuance(),
				total_issuance_without_treasury + treasury_inflation
			);
			assert_eq!(
				Balances::free_balance(&TreasuryPalletId::get().into_account_truncating()),
				initial_treasury_balance + treasury_inflation
			);

			// EPOCH 1 -> EPOCH 2
			MockTime::mock_now(|| 2 * SECONDS_PER_YEAR * 1000);
			advance_session();

			// EPOCH 1 had one collator [1].
			// Thus, reward is minted only once.
			let total_issuance_without_treasury = total_issuance_without_treasury + REWARD;
			let treasury_inflation =
				treasury_inflation + (treasury_inflation + total_issuance_without_treasury) / 10;
			assert_eq!(
				<Test as Config>::Rewards::compute_reward(
					<Test as Config>::StakeCurrencyId::get(),
					&1
				),
				Ok(2 * REWARD)
			);
			assert_eq!(
				Balances::total_issuance(),
				total_issuance_without_treasury + treasury_inflation
			);
			assert_eq!(
				Balances::free_balance(&TreasuryPalletId::get().into_account_truncating()),
				initial_treasury_balance + treasury_inflation
			);

			// EPOCH 2 -> EPOCH 3
			MockTime::mock_now(|| 3 * SECONDS_PER_YEAR * 1000);
			advance_session();

			// EPOCH 2 had two collators [2, 3].
			// Thus, both receive the reward.
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
			let total_issuance_without_treasury = total_issuance_without_treasury + 2 * REWARD;
			let treasury_inflation =
				treasury_inflation + (treasury_inflation + total_issuance_without_treasury) / 10;
			assert_eq!(
				Balances::total_issuance(),
				total_issuance_without_treasury + treasury_inflation
			);
			assert_eq!(
				Balances::free_balance(&TreasuryPalletId::get().into_account_truncating()),
				initial_treasury_balance + treasury_inflation
			);

			// EPOCH 3 -> EPOCH 4
			MockTime::mock_now(|| 4 * SECONDS_PER_YEAR * 1000);
			advance_session();

			// EPOCH 3 had three collators [3, 4, 5].
			// Thus, all three get the reward whereas [1, 2] do not.
			for collator in [1, 3].iter() {
				assert_eq!(
					<Test as Config>::Rewards::compute_reward(
						<Test as Config>::StakeCurrencyId::get(),
						collator
					),
					Ok(2 * REWARD)
				);
			}
			for collator in [2, 4, 5].iter() {
				assert_eq!(
					<Test as Config>::Rewards::compute_reward(
						<Test as Config>::StakeCurrencyId::get(),
						collator
					),
					Ok(REWARD)
				);
			}
			let total_issuance_without_treasury = total_issuance_without_treasury + 3 * REWARD;
			let treasury_inflation =
				treasury_inflation + (treasury_inflation + total_issuance_without_treasury) / 10;
			assert_eq!(
				Balances::total_issuance(),
				total_issuance_without_treasury + treasury_inflation
			);
			assert_eq!(
				Balances::free_balance(&TreasuryPalletId::get().into_account_truncating()),
				initial_treasury_balance + treasury_inflation
			);
		});
}

#[test]
fn late_claiming_works() {
	ExtBuilder::default()
		.set_collator_reward(REWARD)
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

#[test]
fn calculate_epoch_treasury_inflation() {
	let rate = Rate::saturating_from_rational(1, 10);

	ExtBuilder::default().build().execute_with(|| {
		MockTime::mock_now(|| 0);
		let inflation = Balances::total_issuance();
		assert_eq!(BlockRewards::calculate_epoch_treasury_inflation(rate, 0), 0);

		MockTime::mock_now(|| SECONDS_PER_YEAR * 1000);
		assert_eq!(
			BlockRewards::calculate_epoch_treasury_inflation(rate, 0),
			inflation / 10
		);
	})
}
