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

use frame_support::{
	assert_noop, assert_ok,
	traits::{
		tokens::{
			fungible, fungibles, DepositConsequence, ExistenceRequirement, Fortitude, Precision,
			Preservation, Provenance, Restriction, WithdrawConsequence,
		},
		BalanceStatus, Currency, LockableCurrency, ReservableCurrency, WithdrawReasons,
	},
};
use orml_traits::GetByKey;

use crate::{
	mock::{DISTR_PER_ACCOUNT, *},
	Error,
};

#[test]
fn transfer_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::transfer(
				RuntimeOrigin::signed(1),
				2,
				CurrencyId::AUSD,
				DISTR_PER_ACCOUNT
			));
			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::transfer(
				RuntimeOrigin::signed(100),
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
				pallet_restricted_tokens::Pallet::<Runtime>::transfer(
					RuntimeOrigin::signed(10),
					2,
					CurrencyId::AUSD,
					100
				),
				orml_tokens::Error::<Runtime>::BalanceTooLow
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<Runtime>::transfer(
					RuntimeOrigin::signed(10),
					2,
					CurrencyId::AUSD,
					100
				),
				orml_tokens::Error::<Runtime>::BalanceTooLow
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<Runtime>::transfer(
					RuntimeOrigin::signed(10),
					100,
					CurrencyId::RestrictedCoin,
					100
				),
				pallet_restricted_tokens::Error::<Runtime>::PreConditionsNotMet
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<Runtime>::transfer(
					RuntimeOrigin::signed(100),
					10,
					CurrencyId::RestrictedCoin,
					100
				),
				pallet_restricted_tokens::Error::<Runtime>::PreConditionsNotMet
			);
		})
}

#[test]
fn transfer_keep_alive_fails() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_noop!(
				pallet_restricted_tokens::Pallet::<Runtime>::transfer_keep_alive(
					RuntimeOrigin::signed(1),
					2,
					CurrencyId::AUSD,
					DISTR_PER_ACCOUNT
				),
				orml_tokens::Error::<Runtime>::KeepAlive
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<Runtime>::transfer_keep_alive(
					RuntimeOrigin::signed(1),
					2,
					CurrencyId::AUSD,
					DISTR_PER_ACCOUNT
				),
				orml_tokens::Error::<Runtime>::KeepAlive
			);
			assert_noop!(
				pallet_restricted_tokens::Pallet::<Runtime>::transfer_keep_alive(
					RuntimeOrigin::signed(100),
					101,
					CurrencyId::RestrictedCoin,
					DISTR_PER_ACCOUNT
				),
				orml_tokens::Error::<Runtime>::KeepAlive
			);
		})
}

#[test]
fn transfer_keep_alive_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(
				pallet_restricted_tokens::Pallet::<Runtime>::transfer_keep_alive(
					RuntimeOrigin::signed(1),
					2,
					CurrencyId::AUSD,
					DISTR_PER_ACCOUNT - 1
				)
			);
			assert_ok!(
				pallet_restricted_tokens::Pallet::<Runtime>::transfer_keep_alive(
					RuntimeOrigin::signed(100),
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
			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::transfer_all(
				RuntimeOrigin::signed(1),
				2,
				CurrencyId::AUSD,
			));
			assert!(orml_tokens::Pallet::<Runtime>::accounts(2, CurrencyId::AUSD).free == 2000);
			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::transfer_all(
				RuntimeOrigin::signed(1),
				2,
				CurrencyId::AUSD,
			));
			assert!(orml_tokens::Pallet::<Runtime>::accounts(2, CurrencyId::AUSD).free == 2000);
			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::transfer_all(
				RuntimeOrigin::signed(100),
				101,
				CurrencyId::RestrictedCoin,
			));
			assert!(
				orml_tokens::Pallet::<Runtime>::accounts(101, CurrencyId::RestrictedCoin).free
					== 2000
			);
		})
}

#[test]
fn force_transfer_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::force_transfer(
				RuntimeOrigin::root(),
				1,
				2,
				CurrencyId::AUSD,
				DISTR_PER_ACCOUNT
			));
			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::force_transfer(
				RuntimeOrigin::root(),
				1,
				2,
				CurrencyId::RestrictedCoin,
				DISTR_PER_ACCOUNT
			));
		})
}

#[test]
fn force_transfer_fails() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(pallet_restricted_tokens::Pallet::<Runtime>::force_transfer(
				RuntimeOrigin::signed(1),
				1,
				2,
				CurrencyId::AUSD,
				DISTR_PER_ACCOUNT
			)
			.is_err());
			assert!(pallet_restricted_tokens::Pallet::<Runtime>::force_transfer(
				RuntimeOrigin::signed(1),
				1,
				2,
				CurrencyId::AUSD,
				DISTR_PER_ACCOUNT
			)
			.is_err());
			assert!(pallet_restricted_tokens::Pallet::<Runtime>::force_transfer(
				RuntimeOrigin::signed(100),
				100,
				101,
				CurrencyId::RestrictedCoin,
				DISTR_PER_ACCOUNT
			)
			.is_err());
		})
}

#[test]
fn set_balance_works() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::set_balance(
				RuntimeOrigin::root(),
				1,
				CurrencyId::AUSD,
				200,
				100
			));
			assert!(orml_tokens::Pallet::<Runtime>::accounts(1, CurrencyId::AUSD).free == 200);
			assert!(orml_tokens::Pallet::<Runtime>::accounts(1, CurrencyId::AUSD).reserved == 100);

			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::set_balance(
				RuntimeOrigin::root(),
				1,
				CurrencyId::AUSD,
				400,
				200
			));
			assert!(orml_tokens::Pallet::<Runtime>::accounts(1, CurrencyId::AUSD).free == 400);
			assert!(orml_tokens::Pallet::<Runtime>::accounts(1, CurrencyId::AUSD).reserved == 200);

			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::set_balance(
				RuntimeOrigin::root(),
				1111,
				CurrencyId::RestrictedCoin,
				999,
				80
			));
			assert!(
				orml_tokens::Pallet::<Runtime>::accounts(1111, CurrencyId::RestrictedCoin).free
					== 999
			);
			assert!(
				orml_tokens::Pallet::<Runtime>::accounts(1111, CurrencyId::RestrictedCoin).reserved
					== 80
			);

			assert_ok!(pallet_restricted_tokens::Pallet::<Runtime>::set_balance(
				RuntimeOrigin::root(),
				101,
				CurrencyId::RestrictedCoin,
				0,
				100
			));
			assert!(
				orml_tokens::Pallet::<Runtime>::accounts(101, CurrencyId::RestrictedCoin).free == 0
			);
			assert!(
				orml_tokens::Pallet::<Runtime>::accounts(101, CurrencyId::RestrictedCoin).reserved
					== 100
			);
		})
}

// Tests for fungible::* trait calls that restricted tokens wraps

#[test]
fn fungible_total_issuance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::total_issuance(), 10 * DISTR_PER_ACCOUNT)
		})
}

#[test]
fn fungible_minimum_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::minimum_balance(), 1)
		})
}

#[test]
fn fungible_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::balance(&1), DISTR_PER_ACCOUNT)
		})
}

#[test]
fn fungible_reducible_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::reducible_balance(&1, Preservation::Expendable, Fortitude::Polite), DISTR_PER_ACCOUNT - ExistentialDeposit::get());
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::reducible_balance(&1, Preservation::Expendable, Fortitude::Polite), DISTR_PER_ACCOUNT - ExistentialDeposit::get());
		})
}

#[test]
fn fungible_can_deposit() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::can_deposit(&1, 10, Provenance::Extant) == DepositConsequence::Success);
		})
}

#[test]
fn fungible_can_withdraw() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let res = <pallet_restricted_tokens::Pallet<Runtime> as fungible::Inspect<
				AccountId,
			>>::can_withdraw(&1, DISTR_PER_ACCOUNT)
				== WithdrawConsequence::ReducedToZero(0);
			assert!(res);
			let res = <pallet_restricted_tokens::Pallet<Runtime> as fungible::Inspect<
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungible::InspectHold<
					AccountId,
				>>::balance_on_hold(&(), &1),
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungible::InspectHold<
					AccountId,
				>>::can_hold(&(), &1, DISTR_PER_ACCOUNT)
			);
		})
}

#[test]
fn fungible_mint_into() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Mutate<AccountId>>::mint_into(&1, 10).is_ok());
		})
}

#[test]
fn fungible_burn_from() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Mutate<AccountId>>::burn_from(&1, DISTR_PER_ACCOUNT, Precision::Exact, Fortitude::Force).is_ok());
		})
}

#[test]
fn fungible_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungible::MutateHold<
					AccountId,
				>>::hold(&(), &1, DISTR_PER_ACCOUNT)
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungible::MutateHold<
					AccountId,
				>>::hold(&(), &1, DISTR_PER_ACCOUNT)
				.is_ok()
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungible::MutateHold<
					AccountId,
				>>::release(&(), &1, DISTR_PER_ACCOUNT, Precision::Exact)
				.is_ok()
			);
		})
}

#[test]
fn fungible_transfer_on_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::MutateHold<AccountId>>::hold(&(), &1, DISTR_PER_ACCOUNT).is_ok());
			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::MutateHold<AccountId>>::transfer_on_hold(&(), &1, &9, DISTR_PER_ACCOUNT, Precision::BestEffort, Restriction::OnHold, Fortitude::Polite).is_ok());
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::reducible_balance(&1, Preservation::Preserve, Fortitude::Polite), 0);
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::reducible_balance(&9, Preservation::Preserve, Fortitude::Polite), DISTR_PER_ACCOUNT - ExistentialDeposit::get());
			// nuno ^ this might be failing because of BestEffort or because ExistentialDeposit changed

			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::MutateHold<AccountId>>::hold(&(), &2, DISTR_PER_ACCOUNT).is_ok());
			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::MutateHold<AccountId>>::transfer_on_hold(&(), &2, &9, DISTR_PER_ACCOUNT, Precision::Exact, Restriction::Free,Fortitude::Polite).is_ok());
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::reducible_balance(&9, Preservation::Preserve, Fortitude::Polite), 2 * DISTR_PER_ACCOUNT - ExistentialDeposit::get());
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Inspect<AccountId>>::reducible_balance(&2, Preservation::Preserve, Fortitude::Polite), 0);
		})
}

#[test]
fn fungible_transfer() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			// Min holding period is not over
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Mutate<
					AccountId,
				>>::transfer(&1, &100, DISTR_PER_ACCOUNT, Preservation::Expendable)
				.is_err()
			);
			Timer::pass(MIN_HOLD_PERIOD);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungible::Mutate<
					AccountId,
				>>::transfer(&1, &100, DISTR_PER_ACCOUNT, Preservation::Expendable)
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::total_issuance(CurrencyId::Cfg),
				10 * DISTR_PER_ACCOUNT
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::total_issuance(CurrencyId::AUSD),
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::minimum_balance(CurrencyId::Cfg),
				ExistentialDeposit::get()
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::minimum_balance(CurrencyId::AUSD),
				ExistentialDeposits::get(&CurrencyId::AUSD)
			)
		})
}

#[test]
fn fungibles_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::balance(CurrencyId::Cfg, &1),
				DISTR_PER_ACCOUNT
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::balance(CurrencyId::AUSD, &1),
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::Cfg, &1, Preservation::Expendable, Fortitude::Polite),
				DISTR_PER_ACCOUNT - ExistentialDeposit::get()
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::AUSD, &1, Preservation::Expendable, Fortitude::Polite),
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::can_deposit(CurrencyId::Cfg, &1, 10, Provenance::Extant)
					== DepositConsequence::Success
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::can_deposit(CurrencyId::AUSD, &1, 10, Provenance::Extant)
					== DepositConsequence::Success
			);
		})
}

#[test]
fn fungibles_can_withdraw() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let res = <pallet_restricted_tokens::Pallet<Runtime> as fungibles::Inspect<
				AccountId,
			>>::can_withdraw(CurrencyId::AUSD, &1, DISTR_PER_ACCOUNT)
				== WithdrawConsequence::ReducedToZero(0);
			assert!(res);
			let res = <pallet_restricted_tokens::Pallet<Runtime> as fungibles::Inspect<
				AccountId,
			>>::can_withdraw(
				CurrencyId::AUSD,
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::InspectHold<
					AccountId,
				>>::balance_on_hold(CurrencyId::AUSD, &(), &1),
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::InspectHold<
					AccountId,
				>>::can_hold(CurrencyId::Cfg, &(), &1, DISTR_PER_ACCOUNT)
			);
			assert!(
				!<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::InspectHold<
					AccountId,
				>>::can_hold(CurrencyId::AUSD, &(), &1, 0)
			);
			assert!(
				!<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::InspectHold<
					AccountId,
				>>::can_hold(CurrencyId::AUSD, &(), &1, 0)
			);
		})
}

#[test]
fn fungibles_mint_into() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let amount = 10;

			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Mutate<AccountId>>::mint_into(CurrencyId::RestrictedCoin, &1, amount.clone()),
				Error::<Runtime>::PreConditionsNotMet
			);

			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Mutate<AccountId>>::mint_into(CurrencyId::RestrictedCoin, &POOL_PALLET_ID, amount).is_ok());
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<AccountId>>::balance(CurrencyId::RestrictedCoin, &POOL_PALLET_ID), amount);
		})
}

#[test]
fn fungibles_burn_from() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Mutate<AccountId>>::burn_from(CurrencyId::RestrictedCoin, &1, DISTR_PER_ACCOUNT, Precision::Exact, Fortitude::Force),
				Error::<Runtime>::PreConditionsNotMet,
			);

			assert!(<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Mutate<AccountId>>::mint_into(CurrencyId::RestrictedCoin, &POOL_PALLET_ID, 10).is_ok())
		})
}

#[test]
fn fungibles_hold() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::RestrictedCoin, &(), &1, DISTR_PER_ACCOUNT)
				.is_ok()
			);

			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::AUSD, &(), &1, 1),
				Error::<Runtime>::PreConditionsNotMet,
			);

			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::AUSD, &(), &1, 1),
				Error::<Runtime>::PreConditionsNotMet,
			);
		})
}

#[test]
fn fungibles_release() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::RestrictedCoin, &(), &1, DISTR_PER_ACCOUNT)
				.is_ok()
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::release(CurrencyId::RestrictedCoin, &(), &1, DISTR_PER_ACCOUNT, Precision::Exact)
				.is_ok()
			);
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::AUSD, &(), &1, DISTR_PER_ACCOUNT),
				Error::<Runtime>::PreConditionsNotMet
			);
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::AUSD, &(), &1, DISTR_PER_ACCOUNT),
				Error::<Runtime>::PreConditionsNotMet
			);
		})
}

#[test]
fn fungibles_transfer_held() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::RestrictedCoin, &(), &1, DISTR_PER_ACCOUNT)
				.is_ok()
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::transfer_on_hold(
					CurrencyId::RestrictedCoin,
					&(),
					&1,
					&9,
					DISTR_PER_ACCOUNT,
					Precision::Exact,
					Restriction::OnHold,
					Fortitude::Polite,
				)
				.is_ok()
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::RestrictedCoin, &1, Preservation::Expendable, Fortitude::Polite),
				0
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::RestrictedCoin, &9, Preservation::Expendable, Fortitude::Polite),
				DISTR_PER_ACCOUNT / 2
			);

			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::hold(CurrencyId::RestrictedCoin, &(), &2, DISTR_PER_ACCOUNT)
				.is_ok()
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::MutateHold<
					AccountId,
				>>::transfer_on_hold(
					CurrencyId::RestrictedCoin,
					&(),
					&2,
					&9,
					DISTR_PER_ACCOUNT,
					Precision::Exact,
					Restriction::Free,
					Fortitude::Polite,
				)
				.is_ok()
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::RestrictedCoin, &9, Preservation::Expendable, Fortitude::Polite),
				DISTR_PER_ACCOUNT
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Inspect<
					AccountId,
				>>::reducible_balance(CurrencyId::RestrictedCoin, &2, Preservation::Expendable, Fortitude::Polite),
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
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Mutate<
					AccountId,
				>>::transfer(CurrencyId::Cfg, &1, &100, DISTR_PER_ACCOUNT, Preservation::Expendable)
				.is_err()
			);
			Timer::pass(MIN_HOLD_PERIOD);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Mutate<
					AccountId,
				>>::transfer(CurrencyId::Cfg, &1, &100, DISTR_PER_ACCOUNT, Preservation::Expendable)
				.is_ok()
			);
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Mutate<
					AccountId,
				>>::transfer(
					CurrencyId::RestrictedCoin,
					&1,
					&100,
					DISTR_PER_ACCOUNT,
					Preservation::Expendable,
				),
				Error::<Runtime>::PreConditionsNotMet
			);

			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as fungibles::Mutate<
					AccountId,
				>>::transfer(
					CurrencyId::RestrictedCoin,
					&100,
					&101,
					DISTR_PER_ACCOUNT,
					Preservation::Expendable,
				)
				.is_ok()
			);
		})
}

// Tests for currency::* traits calls that restricted tokens wraps

#[test]
fn currency_make_free_balance_be() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			{
				<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::make_free_balance_be(&80, 100)
			};
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::free_balance(
					&80
				),
				100
			);
		})
}

#[test]
fn currency_deposit_into_existing() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			{let _imb = <pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::deposit_into_existing(&8, 100).unwrap();}
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::free_balance(&8), DISTR_PER_ACCOUNT + 100);
		})
}

#[test]
fn currency_deposit_creating() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let issuance = <pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::total_issuance();
			{let _imb = <pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::deposit_creating(&80, 100);}
			assert_eq!(<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::free_balance(&80), 100);
			assert_eq!(<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::total_issuance(), issuance + 100);
		})
}

#[test]
fn currency_withdraw() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::withdraw(
					&1,
					10,
					WithdrawReasons::TRANSFER,
					ExistenceRequirement::KeepAlive
				),
				Error::<Runtime>::PreConditionsNotMet
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::withdraw(
					&1,
					10,
					WithdrawReasons::TRANSACTION_PAYMENT,
					ExistenceRequirement::KeepAlive
				)
				.is_ok(),
			);
		})
}

#[test]
fn currency_slash() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let issuance =
				<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::total_issuance(
				);
			{
				let (_, _) =
					<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::slash(
						&1, 10,
					);
			}
			assert_eq!(
				<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::total_balance(
					&1
				),
				DISTR_PER_ACCOUNT - 10
			);
			assert_eq!(
				<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::total_issuance(
				),
				issuance - 10
			);
		})
}

#[test]
fn currency_transfer() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::transfer(
					&1,
					&20,
					DISTR_PER_ACCOUNT,
					ExistenceRequirement::AllowDeath
				)
				.is_ok()
			);
		})
}

#[test]
fn currency_ensure_can_withdraw() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_noop!(
				<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::ensure_can_withdraw(
					&1,
					10,
					WithdrawReasons::TRANSFER,
					DISTR_PER_ACCOUNT - 10
				),
				Error::<Runtime>::PreConditionsNotMet
			);
			assert!(
				<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::ensure_can_withdraw(
					&1,
					10,
					WithdrawReasons::TRANSACTION_PAYMENT,
					DISTR_PER_ACCOUNT - 10
				)
				.is_ok(),
			);
		})
}

#[test]
fn currency_free_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet::<Runtime> as Currency<AccountId>>::free_balance(
					&1,
				),
				DISTR_PER_ACCOUNT
			);
		})
}

#[test]
fn currency_issue() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let _ = <pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::issue(100);
		})
}

#[test]
fn currency_burn() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			let _ = <pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::burn(100);
		})
}

#[test]
fn currency_total_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::total_balance(
					&1
				),
				DISTR_PER_ACCOUNT
			);
		})
}

#[test]
fn currency_can_slash() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(<pallet_restricted_tokens::Pallet<Runtime> as Currency<
				AccountId,
			>>::can_slash(&1, DISTR_PER_ACCOUNT));
		})
}

#[test]
fn currency_minimum_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_eq!(
				<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::minimum_balance(
				),
				ExistentialDeposit::get()
			);
		})
}

#[test]
fn currency_can_reserve() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert!(
				<pallet_restricted_tokens::Pallet<Runtime> as ReservableCurrency<AccountId>>::can_reserve(
					&1,
					DISTR_PER_ACCOUNT
				)
			);
		})
}

#[test]
fn currency_slash_reserved() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(<pallet_restricted_tokens::Pallet<Runtime> as ReservableCurrency<AccountId>>::reserve(&1, DISTR_PER_ACCOUNT));
			let _ = <pallet_restricted_tokens::Pallet<Runtime> as ReservableCurrency<AccountId>>::slash_reserved(&1, DISTR_PER_ACCOUNT);
		})
}

#[test]
fn currency_reserved_balance() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(<pallet_restricted_tokens::Pallet<Runtime> as ReservableCurrency<AccountId>>::reserve(&1, 100));
			assert_eq!(<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::free_balance(&1), DISTR_PER_ACCOUNT - 100);
		})
}

#[test]
fn currency_reserve() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(<pallet_restricted_tokens::Pallet<Runtime> as ReservableCurrency<AccountId>>::reserve(&1, 100));
		})
}

#[test]
fn currency_unreserve() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(<pallet_restricted_tokens::Pallet<Runtime> as ReservableCurrency<AccountId>>::reserve(&1, 100));
			assert_eq!(<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::free_balance(&1), DISTR_PER_ACCOUNT - 100);
			<pallet_restricted_tokens::Pallet<Runtime> as ReservableCurrency<AccountId>>::unreserve(&1, 100);
			assert_eq!(<pallet_restricted_tokens::Pallet<Runtime> as Currency<AccountId>>::free_balance(&1), DISTR_PER_ACCOUNT);
		})
}

#[test]
fn currency_repatriate_reserved() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			assert_ok!(<pallet_restricted_tokens::Pallet<Runtime> as ReservableCurrency<AccountId>>::repatriate_reserved(&1, &2, 100, BalanceStatus::Free));
		})
}

#[test]
fn currency_remove_lock() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			<pallet_restricted_tokens::Pallet<Runtime> as LockableCurrency<AccountId>>::set_lock(
				LOCK_ID,
				&1,
				DISTR_PER_ACCOUNT,
				WithdrawReasons::TRANSFER,
			);
			<pallet_restricted_tokens::Pallet<Runtime> as LockableCurrency<AccountId>>::remove_lock(
				LOCK_ID, &1,
			);
		})
}

#[test]
fn currency_set_lock() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			<pallet_restricted_tokens::Pallet<Runtime> as LockableCurrency<AccountId>>::set_lock(
				LOCK_ID,
				&1,
				DISTR_PER_ACCOUNT,
				WithdrawReasons::TRANSFER,
			);
		})
}

#[test]
fn currency_extend_lock() {
	TestExternalitiesBuilder::default()
		.build(Some(|| {}))
		.execute_with(|| {
			<pallet_restricted_tokens::Pallet<Runtime> as LockableCurrency<AccountId>>::set_lock(
				LOCK_ID,
				&1,
				DISTR_PER_ACCOUNT,
				WithdrawReasons::TRANSFER,
			);
			<pallet_restricted_tokens::Pallet<Runtime> as LockableCurrency<AccountId>>::extend_lock(
				LOCK_ID,
				&1,
				DISTR_PER_ACCOUNT,
				WithdrawReasons::TRANSACTION_PAYMENT,
			);
		})
}
