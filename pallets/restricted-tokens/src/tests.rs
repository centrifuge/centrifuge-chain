// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use crate::mock::DISTR_PER_ACCOUNT;
use crate::mock::*;
use crate::Error;
use frame_support::traits::tokens::{fungible, fungibles, DepositConsequence, WithdrawConsequence};
use frame_support::{assert_noop, assert_ok};
use orml_traits::GetByKey;

#[test]
fn transfer_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(pallet_restricted_tokens::Pallet::<MockRuntime>::transfer(
				Origin::signed(1),
				2,
				CurrencyId::KUSD,
				DISTR_PER_ACCOUNT
			));
			assert_ok!(pallet_restricted_tokens::Pallet::<MockRuntime>::transfer(
				Origin::signed(1),
				2,
				CurrencyId::USDT,
				DISTR_PER_ACCOUNT
			));
			assert_ok!(pallet_restricted_tokens::Pallet::<MockRuntime>::transfer(
				Origin::signed(100),
				101,
				CurrencyId::RestrictedCoin,
				DISTR_PER_ACCOUNT
			));
		})
}

#[test]
fn transfer_fails() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_noop!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer(
					Origin::signed(10),
					2,
					CurrencyId::KUSD,
					100
				),
				orml_tokens::Error::<MockRuntime>::BalanceTooLow
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer(
					Origin::signed(10),
					2,
					CurrencyId::USDT,
					100
				),
				orml_tokens::Error::<MockRuntime>::BalanceTooLow
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer(
					Origin::signed(10),
					100,
					CurrencyId::RestrictedCoin,
					100
				),
				pallet_restricted_tokens::Error::<MockRuntime>::PreConditionsNotMet
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer(
					Origin::signed(100),
					10,
					CurrencyId::RestrictedCoin,
					100
				),
				pallet_restricted_tokens::Error::<MockRuntime>::PreConditionsNotMet
			);
		})
}

#[test]
fn transfer_keep_alive_fails() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_noop!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_keep_alive(
					Origin::signed(1),
					2,
					CurrencyId::KUSD,
					DISTR_PER_ACCOUNT
				),
				orml_tokens::Error::<MockRuntime>::KeepAlive
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_keep_alive(
					Origin::signed(1),
					2,
					CurrencyId::USDT,
					DISTR_PER_ACCOUNT
				),
				orml_tokens::Error::<MockRuntime>::KeepAlive
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_keep_alive(
					Origin::signed(100),
					101,
					CurrencyId::RestrictedCoin,
					DISTR_PER_ACCOUNT
				),
				orml_tokens::Error::<MockRuntime>::KeepAlive
			);
		})
}

#[test]
fn transfer_keep_alive_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_keep_alive(
					Origin::signed(1),
					2,
					CurrencyId::KUSD,
					DISTR_PER_ACCOUNT - 1
				)
			);
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_keep_alive(
					Origin::signed(1),
					2,
					CurrencyId::USDT,
					DISTR_PER_ACCOUNT - 1
				)
			);
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_keep_alive(
					Origin::signed(100),
					101,
					CurrencyId::RestrictedCoin,
					DISTR_PER_ACCOUNT - 1
				)
			);
		})
}

#[test]
fn transfer_all_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_all(
					Origin::signed(1),
					2,
					CurrencyId::KUSD,
					false
				)
			);
			assert!(orml_tokens::Pallet::<MockRuntime>::accounts(2, CurrencyId::KUSD).free == 2000);
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_all(
					Origin::signed(1),
					2,
					CurrencyId::USDT,
					false
				)
			);
			assert!(orml_tokens::Pallet::<MockRuntime>::accounts(2, CurrencyId::USDT).free == 2000);
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::transfer_all(
					Origin::signed(100),
					101,
					CurrencyId::RestrictedCoin,
					false
				)
			);
			assert!(
				orml_tokens::Pallet::<MockRuntime>::accounts(101, CurrencyId::RestrictedCoin).free
					== 2000
			);
		})
}

#[test]
fn force_transfer_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::force_transfer(
					Origin::root(),
					1,
					2,
					CurrencyId::KUSD,
					DISTR_PER_ACCOUNT
				)
			);
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::force_transfer(
					Origin::root(),
					1,
					2,
					CurrencyId::USDT,
					DISTR_PER_ACCOUNT
				)
			);
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::force_transfer(
					Origin::root(),
					1,
					2,
					CurrencyId::RestrictedCoin,
					DISTR_PER_ACCOUNT
				)
			);
		})
}

#[test]
fn force_transfer_fails() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::force_transfer(
					Origin::signed(1),
					1,
					2,
					CurrencyId::KUSD,
					DISTR_PER_ACCOUNT
				)
				.is_err()
			);
			assert!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::force_transfer(
					Origin::signed(1),
					1,
					2,
					CurrencyId::USDT,
					DISTR_PER_ACCOUNT
				)
				.is_err()
			);
			assert!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::force_transfer(
					Origin::signed(100),
					100,
					101,
					CurrencyId::RestrictedCoin,
					DISTR_PER_ACCOUNT
				)
				.is_err()
			);
		})
}

#[test]
fn set_balance_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::set_balance(
					Origin::root(),
					1,
					CurrencyId::KUSD,
					200,
					100
				)
			);
			assert!(orml_tokens::Pallet::<MockRuntime>::accounts(1, CurrencyId::KUSD).free == 200);
			assert!(
				orml_tokens::Pallet::<MockRuntime>::accounts(1, CurrencyId::KUSD).reserved == 100
			);

			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::set_balance(
					Origin::root(),
					1,
					CurrencyId::USDT,
					400,
					200
				)
			);
			assert!(orml_tokens::Pallet::<MockRuntime>::accounts(1, CurrencyId::USDT).free == 400);
			assert!(
				orml_tokens::Pallet::<MockRuntime>::accounts(1, CurrencyId::USDT).reserved == 200
			);

			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::set_balance(
					Origin::root(),
					1111,
					CurrencyId::RestrictedCoin,
					999,
					80
				)
			);
			assert!(
				orml_tokens::Pallet::<MockRuntime>::accounts(1111, CurrencyId::RestrictedCoin).free
					== 999
			);
			assert!(
				orml_tokens::Pallet::<MockRuntime>::accounts(1111, CurrencyId::RestrictedCoin)
					.reserved == 80
			);

			assert_ok!(
				pallet_restricted_tokens::Pallet::<MockRuntime>::set_balance(
					Origin::root(),
					101,
					CurrencyId::RestrictedCoin,
					0,
					100
				)
			);
			assert!(
				orml_tokens::Pallet::<MockRuntime>::accounts(101, CurrencyId::RestrictedCoin).free
					== 0
			);
			assert!(
				orml_tokens::Pallet::<MockRuntime>::accounts(101, CurrencyId::RestrictedCoin)
					.reserved == 100
			);
		})
}

// Tests for fungible::* trait calls that restricted tokens wraps

#[test]
fn fungible_total_issuance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::total_issuance(), 10 * DISTR_PER_ACCOUNT)
		})
}

#[test]
fn fungible_minimum_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::minimum_balance(), 1)
		})
}

#[test]
fn fungible_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::balance(&1), DISTR_PER_ACCOUNT)
		})
}

#[test]
fn fungible_reducible_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::reducible_balance(&1, true), DISTR_PER_ACCOUNT - ExistentialDeposit::get());
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::reducible_balance(&1, false), DISTR_PER_ACCOUNT - ExistentialDeposit::get());
		})
}

#[test]
fn fungible_can_deposit() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::can_deposit(&1, 10) == DepositConsequence::Success);
		})
}

#[test]
fn fungible_can_withdraw() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let res = <pallet_restricted_tokens::Pallet<MockRuntime> as fungible::Inspect<
				AccountId,
			>>::can_withdraw(&1, DISTR_PER_ACCOUNT)
				== WithdrawConsequence::ReducedToZero(0);
			assert!(res);
			let res = <pallet_restricted_tokens::Pallet<MockRuntime> as fungible::Inspect<
				AccountId,
			>>::can_withdraw(&1, DISTR_PER_ACCOUNT - ExistentialDeposit::get())
				== WithdrawConsequence::Success;
			assert!(res);
		})
}

#[test]
fn fungible_balance_on_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::InspectHold<
					AccountId,
				>>::balance_on_hold(&1,),
				0
			);
		})
}

#[test]
fn fungible_can_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::InspectHold<
					AccountId,
				>>::can_hold(&1, DISTR_PER_ACCOUNT)
			);
		})
}

#[test]
fn fungible_mint_into() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Mutate<AccountId>>::mint_into(&1, 10).is_ok());
		})
}

#[test]
fn fungible_burn_from() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Mutate<AccountId>>::burn_from(&1, DISTR_PER_ACCOUNT).is_ok());
		})
}

#[test]
fn fungible_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::MutateHold<
					AccountId,
				>>::hold(&1, DISTR_PER_ACCOUNT)
				.is_ok()
			);
		})
}

#[test]
fn fungible_release() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::MutateHold<
					AccountId,
				>>::hold(&1, DISTR_PER_ACCOUNT)
				.is_ok()
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::MutateHold<
					AccountId,
				>>::release(&1, DISTR_PER_ACCOUNT, false)
				.is_ok()
			);
		})
}

#[test]
fn fungible_transfer_held() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::MutateHold<AccountId>>::hold(&1, DISTR_PER_ACCOUNT).is_ok());
			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::MutateHold<AccountId>>::transfer_held(&1, &9, DISTR_PER_ACCOUNT, false, true).is_ok());
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::reducible_balance(&1, false), 0);
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::reducible_balance(&9, false), DISTR_PER_ACCOUNT - ExistentialDeposit::get());


			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::MutateHold<AccountId>>::hold(&2, DISTR_PER_ACCOUNT).is_ok());
			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::MutateHold<AccountId>>::transfer_held(&2, &9, DISTR_PER_ACCOUNT, false, false).is_ok());
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::reducible_balance(&9, false), 2 * DISTR_PER_ACCOUNT - ExistentialDeposit::get());
			assert_eq!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Inspect<AccountId>>::reducible_balance(&2, false), 0);
		})
}

#[test]
fn fungible_transfer() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			// Min holding period is not over
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Transfer<
					AccountId,
				>>::transfer(&1, &100, DISTR_PER_ACCOUNT, false)
				.is_err()
			);
			Timer::pass(MIN_HOLD_PERIOD);
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungible::Transfer<
					AccountId,
				>>::transfer(&1, &100, DISTR_PER_ACCOUNT, false)
				.is_ok()
			);
		})
}

// Tests for fungibles::* trait calls that restricted tokens wraps

#[test]
fn fungibles_total_issuance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::total_issuance(CurrencyId::Cfg),
				10 * DISTR_PER_ACCOUNT
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::total_issuance(CurrencyId::KUSD),
				10 * DISTR_PER_ACCOUNT
			);
		})
}

#[test]
fn fungibles_minimum_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::minimum_balance(CurrencyId::Cfg),
				ExistentialDeposit::get()
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::minimum_balance(CurrencyId::KUSD),
				ExistentialDeposits::get(&CurrencyId::KUSD)
			)
		})
}

#[test]
fn fungibles_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::balance(CurrencyId::Cfg, &1),
				DISTR_PER_ACCOUNT
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::balance(CurrencyId::KUSD, &1),
				DISTR_PER_ACCOUNT
			)
		})
}

#[test]
fn fungibles_reducible_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::Cfg, &1, false),
				DISTR_PER_ACCOUNT - ExistentialDeposit::get()
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::KUSD, &1, false),
				DISTR_PER_ACCOUNT / 2
			);
		})
}

#[test]
fn fungibles_can_deposit() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::can_deposit(CurrencyId::Cfg, &1, 10)
					== DepositConsequence::Success
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::can_deposit(CurrencyId::KUSD, &1, 10)
					== DepositConsequence::Success
			);
		})
}

#[test]
fn fungibles_can_withdraw() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let res = <pallet_restricted_tokens::Pallet<MockRuntime> as fungibles::Inspect<
				AccountId,
			>>::can_withdraw(CurrencyId::KUSD, &1, DISTR_PER_ACCOUNT)
				== WithdrawConsequence::ReducedToZero(0);
			assert!(res);
			let res = <pallet_restricted_tokens::Pallet<MockRuntime> as fungibles::Inspect<
				AccountId,
			>>::can_withdraw(
				CurrencyId::KUSD,
				&1,
				DISTR_PER_ACCOUNT - ExistentialDeposit::get(),
			) == WithdrawConsequence::Success;
			assert!(res);
		})
}

#[test]
fn fungibles_balance_on_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::InspectHold<
					AccountId,
				>>::balance_on_hold(CurrencyId::USDT, &1,),
				0
			);
		})
}

#[test]
fn fungibles_can_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::InspectHold<
					AccountId,
				>>::can_hold(CurrencyId::Cfg, &1, DISTR_PER_ACCOUNT)
			);
			assert!(
				!<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::InspectHold<
					AccountId,
				>>::can_hold(CurrencyId::KUSD, &1, 0)
			);
			assert!(
				!<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::InspectHold<
					AccountId,
				>>::can_hold(CurrencyId::USDT, &1, 0)
			);
		})
}

#[test]
fn fungibles_mint_into() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Mutate<AccountId>>::mint_into(CurrencyId::RestrictedCoin, &1, 10),
				Error::<MockRuntime>::PreConditionsNotMet
			);

			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Mutate<AccountId>>::mint_into(CurrencyId::RestrictedCoin, &POOL_PALLET_ID, 10).is_ok())
		})
}

#[test]
fn fungibles_burn_from() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Mutate<AccountId>>::burn_from(CurrencyId::RestrictedCoin, &1, DISTR_PER_ACCOUNT),
				Error::<MockRuntime>::PreConditionsNotMet,
			);

			assert!(<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Mutate<AccountId>>::mint_into(CurrencyId::RestrictedCoin, &POOL_PALLET_ID, 10).is_ok())
		})
}

#[test]
fn fungibles_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::RestrictedCoin, &1, DISTR_PER_ACCOUNT)
				.is_ok()
			);

			assert_noop!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::KUSD, &1, 1),
				Error::<MockRuntime>::PreConditionsNotMet,
			);

			assert_noop!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::USDT, &1, 1),
				Error::<MockRuntime>::PreConditionsNotMet,
			);
		})
}

#[test]
fn fungibles_release() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::RestrictedCoin, &1, DISTR_PER_ACCOUNT)
				.is_ok()
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::release(CurrencyId::RestrictedCoin, &1, DISTR_PER_ACCOUNT, false)
				.is_ok()
			);
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::KUSD, &1, DISTR_PER_ACCOUNT),
				Error::<MockRuntime>::PreConditionsNotMet
			);
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::USDT, &1, DISTR_PER_ACCOUNT),
				Error::<MockRuntime>::PreConditionsNotMet
			);
		})
}

#[test]
fn fungibles_transfer_held() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::RestrictedCoin, &1, DISTR_PER_ACCOUNT)
				.is_ok()
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::transfer_held(
					CurrencyId::RestrictedCoin,
					&1,
					&9,
					DISTR_PER_ACCOUNT,
					false,
					true
				)
				.is_ok()
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::RestrictedCoin, &1, false),
				0
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::RestrictedCoin, &9, false),
				DISTR_PER_ACCOUNT / 2
			);

			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::RestrictedCoin, &2, DISTR_PER_ACCOUNT)
				.is_ok()
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::MutateHold<
					AccountId,
				>>::transfer_held(
					CurrencyId::RestrictedCoin,
					&2,
					&9,
					DISTR_PER_ACCOUNT,
					false,
					false
				)
				.is_ok()
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::RestrictedCoin, &9, false),
				DISTR_PER_ACCOUNT
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::RestrictedCoin, &2, false),
				0
			);
		})
}

#[test]
fn fungibles_transfer() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			// Min holding period is not over
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Transfer<
					AccountId,
				>>::transfer(CurrencyId::Cfg, &1, &100, DISTR_PER_ACCOUNT, false)
				.is_err()
			);
			Timer::pass(MIN_HOLD_PERIOD);
			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Transfer<
					AccountId,
				>>::transfer(CurrencyId::Cfg, &1, &100, DISTR_PER_ACCOUNT, false)
				.is_ok()
			);
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Transfer<
					AccountId,
				>>::transfer(
					CurrencyId::RestrictedCoin,
					&1,
					&100,
					DISTR_PER_ACCOUNT,
					false
				),
				Error::<MockRuntime>::PreConditionsNotMet
			);

			assert!(
				<pallet_restricted_tokens::Pallet::<MockRuntime> as fungibles::Transfer<
					AccountId,
				>>::transfer(
					CurrencyId::RestrictedCoin,
					&100,
					&101,
					DISTR_PER_ACCOUNT,
					false
				)
				.is_ok()
			);
		})
}
