// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.
//
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use frame_support::assert_ok;
use xcm_emulator::TestExt;
use xcm::latest::{Junction, Junctions::*, MultiLocation, NetworkId};
use orml_traits::MultiCurrency;

use crate::parachain::{
	Balances, NativePerSecond, Origin, OrmlTokens, UsdPerSecond2000, XTokens, PARA_ID,
};
use super::{
	test_net::{Parachain, Sibling, TestNet},
	setup::{
		parachain_account, native_amount, sibling_account, usd_amount, CurrencyId, ALICE, BOB,
		PARA_ID_SIBLING,
	}
};

use runtime_common::Balance;

#[test]
fn transfer_native_to_sibling() {
	TestNet::reset();

	let alice_initial_balance = native_amount(10);
	let bob_initial_balance = native_amount(10);
	let transfer_amount = native_amount(1);

	Parachain::execute_with(|| {
		assert_eq!(Balances::free_balance(&ALICE.into()), alice_initial_balance);
		assert_eq!(Balances::free_balance(&sibling_account()), 0);
	});

	Sibling::execute_with(|| {
		assert_eq!(Balances::free_balance(&BOB.into()), bob_initial_balance);
	});

	Parachain::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::Native,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(PARA_ID_SIBLING),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			8_000_000_000_000,
		));

		// Confirm that Alice's balance is initial balance - amount transferred
		assert_eq!(
			Balances::free_balance(&ALICE.into()),
			alice_initial_balance - transfer_amount
		);

		// Verify that the amount transferred is now part of the sibling account here
		assert_eq!(Balances::free_balance(&sibling_account()), transfer_amount);
	});

	Sibling::execute_with(|| {
		// Verify that BOB now has initial balance + amount transferred - fee
		assert_eq!(
			Balances::free_balance(&BOB.into()),
			bob_initial_balance + transfer_amount - native_fee(),
		);
	});
}

#[test]
fn transfer_usd_to_sibling() {
	TestNet::reset();

	let alice_initial_balance = usd_amount(10);
	let bob_initial_balance = usd_amount(10);
	let transfer_amount = usd_amount(7);

	Parachain::execute_with(|| {
		assert_ok!(OrmlTokens::deposit(
			CurrencyId::Usd,
			&ALICE.into(),
			alice_initial_balance
		));

		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &sibling_account()),
			0
		);
	});

	Sibling::execute_with(|| {
		assert_ok!(OrmlTokens::deposit(
			CurrencyId::Usd,
			&BOB.into(),
			bob_initial_balance
		));
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &BOB.into()),
			bob_initial_balance,
		);
	});

	Parachain::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::Usd,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(PARA_ID_SIBLING),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			8_000_000_000,
		));

		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &ALICE.into()),
			alice_initial_balance - transfer_amount
		);

		// Verify that the amount transferred is now part of the sibling account here
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &sibling_account()),
			transfer_amount
		);
	});

	Sibling::execute_with(|| {
		// Verify that BOB now has initial balance + amount transferred - fee
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &BOB.into()),
			bob_initial_balance + transfer_amount - usd_fee()
		);
	});
}

#[test]
fn transfer_usd_to_development() {
	TestNet::reset();

	let alice_initial_balance = usd_amount(10);
	let bob_initial_balance = usd_amount(10);
	let transfer_amount = usd_amount(7);

	Sibling::execute_with(|| {
		assert_ok!(OrmlTokens::deposit(
			CurrencyId::Usd,
			&ALICE.into(),
			alice_initial_balance
		));

		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &parachain_account()),
			0
		);
	});

	Parachain::execute_with(|| {
		assert_ok!(OrmlTokens::deposit(
			CurrencyId::Usd,
			&BOB.into(),
			bob_initial_balance
		));
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &BOB.into()),
			bob_initial_balance,
		);

		assert_ok!(OrmlTokens::deposit(
			CurrencyId::Usd,
			&sibling_account().into(),
			bob_initial_balance
		));
	});

	Sibling::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::Usd,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(PARA_ID),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			8_000_000_000,
		));

		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &ALICE.into()),
			alice_initial_balance - transfer_amount
		);

		// Verify that the amount transferred is now part of the development account here
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &parachain_account()),
			transfer_amount
		);
	});

	Parachain::execute_with(|| {
		// Verify that BOB now has initial balance + amount transferred - fee
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::Usd, &BOB.into()),
			bob_initial_balance + transfer_amount - usd_fee()
		);
	});
}

// The fee associated with transferring Native tokens
fn native_fee() -> Balance {
	let (_asset, fee) = NativePerSecond::get();
	// We divide the fee to align its unit and multiply by 4 as that seems to be the unit of
	// time the transfers take.
	// NOTE: it is possible that in different machines this value may differ. We shall see.
	fee.div_euclid(10_000) * 4
}

// The fee associated with transferring Native tokens
fn usd_fee() -> Balance {
	let (_asset, fee) = UsdPerSecond2000::get();
	// We divide the fee to align its unit and multiply by 4 as that seems to be the unit of
	// time the transfers take.
	// NOTE: it is possible that in different machines this value may differ. We shall see.
	fee.div_euclid(10_000) * 4
}
