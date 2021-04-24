// Copyright 2019-2021 Centrifuge Inc.
// This file is part of Cent-Chain.

// Cent-Chain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cent-Chain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cent-Chain.  If not, see <http://www.gnu.org/licenses/>.
use crate::mock::*;
use crowdloan_claim::reward::Reward;
use frame_support::traits::VestingSchedule;
use frame_support::assert_noop;
use crate as pallet_rewards;
use sp_runtime::DispatchError;
use frame_support::sp_runtime::traits::AccountIdConversion;

#[test]
fn convert_to_native() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(||System::set_block_number(1))
        .execute_with(|| {
            assert_eq!(Rewards::convert_to_native(10), 8);
        });
}

#[test]
fn initalize_module() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(||System::set_block_number(4))
        .execute_with(|| {
            assert!(Rewards::initialize(3, 12, 4, 4).is_ok());
        });
}

#[test]
fn not_admin_for_setters() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            System::set_block_number(1);
            Rewards::initialize(50, 20, 4, 3).unwrap();
        })
        .execute_with(|| {
            assert_noop!(
                Rewards::set_vesting_start(Origin::signed(2), 1),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Rewards::set_vesting_period(Origin::signed(2), 3),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Rewards::set_conversion_rate(Origin::signed(2), 100),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Rewards::set_direct_payout_ratio(Origin::signed(2), 10),
                DispatchError::BadOrigin
            );
        });
}

#[test]
fn setters_ok() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            System::set_block_number(1);
            Rewards::initialize(50, 20, 4, 3).unwrap();
        })
        .execute_with(|| {
            assert!( Rewards::set_vesting_start(Origin::signed(1), 1).is_ok() );
            assert!( Rewards::set_vesting_period(Origin::signed(1), 55555).is_ok() );
            assert!( Rewards::set_direct_payout_ratio(Origin::signed(1), 9).is_ok() );
            assert!( Rewards::set_conversion_rate(Origin::root(), 80).is_ok() );
        });
}


#[test]
fn elapsed_time() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| System::set_block_number(3) )
        .execute_with(|| {
            assert_noop!(
                Rewards::set_vesting_start(Origin::root(), 2),
                pallet_rewards::Error::<Test>::ElapsedTime
            );
        });
}

#[test]
fn reward_participant() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            System::set_block_number(1);
            Rewards::initialize(80, 20, 4, 3).unwrap()
        })
        .execute_with(|| {
            let mod_account = super::MODULE_ID.into_account();
            let mod_balance = Balances::free_balance(&mod_account);
            let rew_balance = Balances::free_balance(&4);

            assert!(Rewards::reward(4, 50).is_ok());
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
                *event == pallet_rewards::Event::<Test>::RewardClaimed(4, 8, 32)
            }));
        });
}

#[test]
fn not_enough_funds_to_reward() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            System::set_block_number(1);
            Rewards::initialize(80, 20, 4, 3).unwrap()
        })
        .execute_with(|| {
            assert_noop!(
                Rewards::reward(4, 200),
                pallet_rewards::Error::<Test>::NotEnoughFunds
            );
        });
}

#[test]
fn account_already_vesting() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            System::set_block_number(1);
            Rewards::initialize(80, 20, 4, 3).unwrap()
        })
        .execute_with(|| {
            assert_noop!(
                Rewards::reward(1, 30),
                pallet_vesting::Error::<Test>::ExistingVestingSchedule);
        });
}
#[test]
fn reward_amount_to_low_for_vesting() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            System::set_block_number(1);
            Rewards::initialize(80, 20, 4, 3).unwrap()
        })
        .execute_with(|| {
            assert_noop!(
                Rewards::reward(1, 15),
                pallet_vesting::Error::<Test>::AmountLow);
        });
}