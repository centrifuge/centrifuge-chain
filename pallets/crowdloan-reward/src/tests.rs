// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.


//! Crowdloan reward pallet's unit test cases


#![cfg(test)]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use super::*;
use crate::{
  self as pallet_crowdloan_reward,
  Error as CrowdloanRewardError,
  mock::*
};

use frame_support::{
  assert_noop,
  dispatch::{DispatchError},
  sp_runtime::traits::AccountIdConversion,
  traits::VestingSchedule
};


// ----------------------------------------------------------------------------
// Test cases
// ----------------------------------------------------------------------------

#[test]
fn convert_to_native() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(||System::set_block_number(1))
    .execute_with(|| {
        assert_eq!(CrowdloanReward::convert_to_native(10), 8);
    });
}

#[test]
fn initalize_module() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(||System::set_block_number(4))
    .execute_with(|| {
        assert!(CrowdloanReward::initialize(3, 12, 4, 4).is_ok());
    });
}

#[test]
fn not_admin_for_setters() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(|| {
      System::set_block_number(1);
      CrowdloanReward::initialize(50, 20, 4, 3).unwrap();
    })
    .execute_with(|| {
      assert_noop!(
        CrowdloanReward::set_vesting_start(Origin::signed(2), 1),
        DispatchError::BadOrigin
      );
      assert_noop!(
        CrowdloanReward::set_vesting_period(Origin::signed(2), 3),
        DispatchError::BadOrigin
      );
      assert_noop!(
        CrowdloanReward::set_conversion_rate(Origin::signed(2), 100),
        DispatchError::BadOrigin
      );
      assert_noop!(
        CrowdloanReward::set_direct_payout_ratio(Origin::signed(2), 10),
        DispatchError::BadOrigin
      );
    });
}

#[test]
fn setters_ok() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(|| {
      System::set_block_number(1);
      CrowdloanReward::initialize(50, 20, 4, 3).unwrap();
    })
    .execute_with(|| {
      assert!( CrowdloanReward::set_vesting_start(Origin::signed(1), 1).is_ok() );
      assert!( CrowdloanReward::set_vesting_period(Origin::signed(1), 55555).is_ok() );
      assert!( CrowdloanReward::set_direct_payout_ratio(Origin::signed(1), 9).is_ok() );
      assert!( CrowdloanReward::set_conversion_rate(Origin::root(), 80).is_ok() );
    });
}


#[test]
fn elapsed_time() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(|| System::set_block_number(3) )
    .execute_with(|| {
      assert_noop!(
        CrowdloanReward::set_vesting_start(Origin::root(), 2),
        CrowdloanRewardError::<MockRuntime>::ElapsedTime
      );
    });
}

#[test]
fn reward_participant() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(|| {
        System::set_block_number(1);
        CrowdloanReward::initialize(80, 20, 4, 3).unwrap()
    })
    .execute_with(|| {
      let mod_account = pallet_crowdloan_reward::ModuleId.into_account();
      let mod_balance = Balances::free_balance(&mod_account);
      let rew_balance = Balances::free_balance(&4);

      assert!(CrowdloanReward::reward(4, 50).is_ok());
      // Reward in native is contribution * 0.8. Hence, here 50 * 0.8 = 40
      assert_eq!(Balances::free_balance(&mod_account), mod_balance - 40);

      assert_eq!(Vesting::vesting_balance(&4), Some(32));
      assert_eq!(Balances::usable_balance(&4), rew_balance + 8);

      System::set_block_number(7);
      assert_eq!(System::block_number(), 7);
      // Account has fully vested
      assert_eq!(Vesting::vesting_balance(&4), Some(0));

      let events = reward_events();
      assert!(events.iter().any(|event| {
        *event == pallet_crowdloan_reward::Event::<MockRuntime>::RewardClaimed(4, 8, 32)
      }));
    });
}

#[test]
fn not_enough_funds_to_reward() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(|| {
      System::set_block_number(1);
      CrowdloanReward::initialize(80, 20, 4, 3).unwrap()
    })
    .execute_with(|| {
      assert_noop!(
        CrowdloanReward::reward(4, 200),
        CrowdloanRewardError::<MockRuntime>::NotEnoughFunds
      );
    });
}

#[test]
fn account_already_vesting() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(|| {
      System::set_block_number(1);
      CrowdloanReward::initialize(80, 20, 4, 3).unwrap()
    })
    .execute_with(|| {
      assert_noop!(
        CrowdloanReward::reward(1, 30),
        CrowdloanRewardError::<MockRuntime>::ExistingVestingSchedule);
    });
}
#[test]
fn reward_amount_to_low_for_vesting() {
  TestExternalitiesBuilder::default()
    .existential_deposit(1)
    .build(|| {
      System::set_block_number(1);
      CrowdloanReward::initialize(80, 20, 4, 3).unwrap()
    })
    .execute_with(|| {
      assert_noop!(
        CrowdloanReward::reward(1, 15),
        CrowdloanRewardError::<MockRuntime>::AmountLow);
    });
}
