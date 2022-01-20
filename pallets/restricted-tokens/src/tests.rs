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
use frame_support::{assert_noop, assert_ok};

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
