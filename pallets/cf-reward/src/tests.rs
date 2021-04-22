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

#[test]
fn initalize_module() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(||{})
        .execute_with(|| {
            assert!(Rewards::initialize(
                Origin::root(),
                111,
                100,
            ).is_ok())
        });
}

#[test]
fn ongoing_lease_at_init() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(||{
            System::set_block_number(0);
        })
        .execute_with(|| {
            assert_noop!(
                Rewards::initialize( Origin::root() , 111, 100, ),
                pallet_rewards::Error::<Test>::OngoingLease
            );
        });
}

#[test]
fn not_root_at_init() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(||{})
        .execute_with(|| {
            assert_noop!(
                Rewards::initialize( Origin::signed(1), 111, 100, ),
                DispatchError::BadOrigin
            );
        });
}

#[test]
fn reward_participant() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            Rewards::initialize(
                Origin::root(),
                111,
                100,
            ).unwrap();
        })
        .execute_with(|| {
            let mod_balance = Balances::free_balance(&Rewards::mod_account());
            assert!(Rewards::reward(4, 20).is_ok());
            assert_eq!(Balances::free_balance(&Rewards::mod_account()), mod_balance - 20);
            assert_eq!(Vesting::vesting_balance(&4).unwrap(), 20);

            let events = reward_events();
            assert!(events.iter().any(|event| {
                *event == pallet_rewards::Event::<Test>::RewardClaimed(4)
            }));
        });
}

#[test]
fn not_enough_funds_to_reward() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            Rewards::initialize(
                Origin::root(),
                111,
                100,
            ).unwrap();
        })
        .execute_with(|| {
            assert_noop!(Rewards::reward(4, 200), pallet_rewards::Error::<Test>::NotEnoughFunds);
        });
}

#[test]
fn account_already_vesting() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            Rewards::initialize(
                Origin::root(),
                111,
                100,
            ).unwrap();
        })
        .execute_with(|| {
            assert_noop!(Rewards::reward(1, 20), pallet_vesting::Error::<Test>::ExistingVestingSchedule);
        });
}
#[test]
fn reward_amount_to_low_for_vesting() {
    ExtBuilder::default()
        .existential_deposit(1)
        .build(|| {
            Rewards::initialize(
                Origin::root(),
                111,
                100,
            ).unwrap();
        })
        .execute_with(|| {
            assert_noop!(Rewards::reward(1, 15), pallet_vesting::Error::<Test>::AmountLow);
        });
}