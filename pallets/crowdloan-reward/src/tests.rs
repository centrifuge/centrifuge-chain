// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Crowdloan reward pallet's unit test cases

#![cfg(test)]

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use frame_support::{assert_noop, traits::VestingSchedule};
use sp_runtime::Perbill;

use crate::{self as pallet_crowdloan_reward, mock::*, Error as CrowdloanRewardError, *};

// ----------------------------------------------------------------------------
// Test cases
// ----------------------------------------------------------------------------

#[test]
fn initalize_module() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| System::set_block_number(4))
		.execute_with(|| {
			assert!(CrowdloanReward::initialize(
				RuntimeOrigin::signed(1),
				Perbill::from_percent(12),
				4,
				4,
			)
			.is_ok());

			assert_eq!(
				CrowdloanReward::direct_payout_ratio(),
				Perbill::from_percent(12)
			);
			assert_eq!(CrowdloanReward::vesting_period(), Some(4));
			assert_eq!(CrowdloanReward::vesting_start(), Some(4));
		});
}

#[test]
fn not_admin_for_setters() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			System::set_block_number(1);
			CrowdloanReward::initialize(RuntimeOrigin::signed(1), Perbill::from_percent(20), 4, 3)
				.unwrap();
		})
		.execute_with(|| {
			assert_noop!(
				CrowdloanReward::set_vesting_start(RuntimeOrigin::signed(2), 1),
				CrowdloanRewardError::<Runtime>::MustBeAdministrator
			);
			assert_noop!(
				CrowdloanReward::set_vesting_period(RuntimeOrigin::signed(2), 3),
				CrowdloanRewardError::<Runtime>::MustBeAdministrator
			);
			assert_noop!(
				CrowdloanReward::set_direct_payout_ratio(
					RuntimeOrigin::signed(2),
					Perbill::from_percent(10)
				),
				CrowdloanRewardError::<Runtime>::MustBeAdministrator
			);
		});
}

#[test]
fn setters_ok() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			System::set_block_number(1);
			CrowdloanReward::initialize(RuntimeOrigin::signed(1), Perbill::from_percent(20), 4, 3)
				.unwrap();
		})
		.execute_with(|| {
			assert!(CrowdloanReward::set_vesting_start(RuntimeOrigin::signed(1), 1).is_ok());
			assert!(CrowdloanReward::set_vesting_period(RuntimeOrigin::signed(1), 55555).is_ok());
			assert!(CrowdloanReward::set_direct_payout_ratio(
				RuntimeOrigin::signed(1),
				Perbill::from_percent(9)
			)
			.is_ok());
			assert!(CrowdloanReward::set_vesting_start(RuntimeOrigin::root(), 1).is_ok());
			assert!(CrowdloanReward::set_vesting_period(RuntimeOrigin::root(), 55555).is_ok());
			assert!(CrowdloanReward::set_direct_payout_ratio(
				RuntimeOrigin::root(),
				Perbill::from_percent(9)
			)
			.is_ok());
		});
}

#[test]
fn reward_participant() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			System::set_block_number(1);
			CrowdloanReward::initialize(RuntimeOrigin::signed(1), Perbill::from_percent(20), 4, 3)
				.unwrap()
		})
		.execute_with(|| {
			let mod_account = CrowdloanReward::account_id();
			let total_issuance = Balances::total_issuance();
			let mod_balance = Balances::free_balance(&mod_account);
			let rew_balance = Balances::free_balance(&4);

			assert!(CrowdloanReward::reward(4, 100).is_ok());
			assert_eq!(Balances::free_balance(&mod_account), 0);
			assert_eq!(mod_balance, 0);
			assert_eq!(Balances::total_issuance(), total_issuance + 100);

			assert_eq!(Vesting::vesting_balance(&4), Some(80));
			assert_eq!(Balances::usable_balance(&4), rew_balance + 20);

			System::set_block_number(7);
			assert_eq!(System::block_number(), 7);
			// Account has fully vested
			assert_eq!(Vesting::vesting_balance(&4), Some(0));

			let events = reward_events();
			assert!(events.iter().any(|event| {
				*event == pallet_crowdloan_reward::Event::<Runtime>::RewardClaimed(4, 20, 80)
			}));
		});
}

#[test]
fn zero_direct_payout_reward() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			System::set_block_number(1);
			CrowdloanReward::initialize(RuntimeOrigin::signed(1), Perbill::from_percent(0), 4, 3)
				.unwrap()
		})
		.execute_with(|| {
			let mod_account = CrowdloanReward::account_id();
			let total_issuance = Balances::total_issuance();
			let mod_balance = Balances::free_balance(&mod_account);
			let rew_balance = Balances::free_balance(&4);

			assert!(CrowdloanReward::reward(4, 100).is_ok());
			assert_eq!(Balances::free_balance(&mod_account), mod_balance);
			assert_eq!(Balances::total_issuance(), total_issuance + 100);

			assert_eq!(Vesting::vesting_balance(&4), Some(100));
			// Ensure that no direct payout happened
			assert_eq!(Balances::usable_balance(&4), rew_balance);

			System::set_block_number(7);
			assert_eq!(System::block_number(), 7);
			// Account has fully vested
			assert_eq!(Vesting::vesting_balance(&4), Some(0));

			let events = reward_events();
			assert!(events.iter().any(|event| {
				*event == pallet_crowdloan_reward::Event::<Runtime>::RewardClaimed(4, 0, 100)
			}));
		});
}

#[test]
fn account_already_vesting() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			System::set_block_number(1);
			CrowdloanReward::initialize(RuntimeOrigin::signed(1), Perbill::from_percent(20), 4, 3)
				.unwrap()
		})
		.execute_with(|| {
			assert_noop!(
				CrowdloanReward::reward(1, 30),
				pallet_vesting::Error::<Runtime>::AtMaxVestingSchedules
			);
		});
}

#[test]
fn reward_amount_to_low_for_vesting() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			System::set_block_number(1);
			CrowdloanReward::initialize(RuntimeOrigin::signed(1), Perbill::from_percent(20), 4, 3)
				.unwrap()
		})
		.execute_with(|| {
			assert_noop!(
				CrowdloanReward::reward(1, 15),
				pallet_vesting::Error::<Runtime>::AmountLow
			);
		});
}
