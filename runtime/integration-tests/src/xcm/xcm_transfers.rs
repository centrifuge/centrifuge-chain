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

use xcm::latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId};

use orml_traits::MultiCurrency;

use crate::xcm::setup::{
	air_amount, altair_account, karura_account, ksm_amount, kusd_amount, sibling_account,
	CurrencyId, ALICE, BOB, PARA_ID_ALTAIR, PARA_ID_SIBLING,
};
use crate::xcm::test_net::{Altair, Karura, KusamaNet, Sibling, TestNet};

use altair_runtime::{
	AirPerSecond, Balances, KUsdPerSecond, KsmPerSecond, Origin, OrmlTokens, XTokens,
};
use runtime_common::{parachains, Balance};

#[test]
fn transfer_air_to_sibling() {
	TestNet::reset();

	let alice_initial_balance = air_amount(10);
	let bob_initial_balance = air_amount(10);
	let transfer_amount = air_amount(1);

	Altair::execute_with(|| {
		assert_eq!(Balances::free_balance(&ALICE.into()), alice_initial_balance);
		assert_eq!(Balances::free_balance(&sibling_account()), 0);
	});

	Sibling::execute_with(|| {
		assert_eq!(Balances::free_balance(&BOB.into()), bob_initial_balance);
	});

	Altair::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::Native,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(PARA_ID_SIBLING),
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
			bob_initial_balance + transfer_amount - air_fee(),
		);
	});
}

#[test]
fn transfer_kusd_to_development() {
	TestNet::reset();

	let alice_initial_balance = kusd_amount(10);
	let bob_initial_balance = kusd_amount(10);
	let transfer_amount = kusd_amount(7);

	Karura::execute_with(|| {
		assert_ok!(OrmlTokens::deposit(
			CurrencyId::KUSD,
			&ALICE.into(),
			alice_initial_balance
		));

		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::KUSD, &altair_account()),
			0
		);
	});

	Altair::execute_with(|| {
		assert_ok!(OrmlTokens::deposit(
			CurrencyId::KUSD,
			&BOB.into(),
			bob_initial_balance
		));
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::KUSD, &BOB.into()),
			bob_initial_balance,
		);

		assert_ok!(OrmlTokens::deposit(
			CurrencyId::KUSD,
			&karura_account().into(),
			bob_initial_balance
		));
	});

	Karura::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::KUSD,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(PARA_ID_ALTAIR),
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
			OrmlTokens::free_balance(CurrencyId::KUSD, &ALICE.into()),
			alice_initial_balance - transfer_amount
		);

		// Verify that the amount transferred is now part of the altair parachain account here
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::KUSD, &altair_account()),
			transfer_amount
		);
	});

	Altair::execute_with(|| {
		// Verify that BOB now has initial balance + amount transferred - fee
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::KUSD, &BOB.into()),
			bob_initial_balance + transfer_amount - kusd_fee()
		);
	});
}

#[test]
fn transfer_from_relay_chain() {
	let transfer_amount: Balance = ksm_amount(1);

	KusamaNet::execute_with(|| {
		assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
			kusama_runtime::Origin::signed(ALICE.into()),
			Box::new(Parachain(PARA_ID_ALTAIR).into().into()),
			Box::new(
				Junction::AccountId32 {
					network: NetworkId::Any,
					id: BOB,
				}
				.into()
				.into()
			),
			Box::new((Here, transfer_amount).into()),
			0
		));
	});

	Altair::execute_with(|| {
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::KSM, &BOB.into()),
			transfer_amount - ksm_fee()
		);
	});
}

#[test]
fn transfer_ksm_to_relay_chain() {
	Altair::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::KSM,
			ksm_amount(1),
			Box::new(
				MultiLocation::new(
					1,
					X1(Junction::AccountId32 {
						id: BOB,
						network: NetworkId::Any,
					})
				)
				.into()
			),
			4_000_000_000
		));
	});

	KusamaNet::execute_with(|| {
		assert_eq!(
			kusama_runtime::Balances::free_balance(&BOB.into()),
			999893333340
		);
	});
}

pub mod currency_id_convert {
	use super::*;
	use altair_runtime::CurrencyIdConvert;
	use sp_runtime::traits::Convert as C2;
	use xcm_executor::traits::Convert as C1;

	#[test]
	fn convert_air() {
		assert_eq!(parachains::altair::AIR_KEY.to_vec(), vec![0]);

		let air_location: MultiLocation = MultiLocation::new(
			1,
			X2(
				Parachain(parachains::altair::ID),
				GeneralKey(parachains::altair::AIR_KEY.to_vec()),
			),
		);

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(air_location.clone()),
			Ok(CurrencyId::Native),
		);

		Altair::execute_with(|| {
			assert_eq!(
				<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
				Some(air_location)
			)
		});
	}

	#[test]
	fn convert_kusd() {
		assert_eq!(parachains::karura::KUSD_KEY.to_vec(), vec![0, 129]);

		let kusd_location: MultiLocation = MultiLocation::new(
			1,
			X2(
				Parachain(parachains::karura::ID),
				GeneralKey(parachains::karura::KUSD_KEY.to_vec()),
			),
		);

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(kusd_location.clone()),
			Ok(CurrencyId::KUSD),
		);

		Altair::execute_with(|| {
			assert_eq!(
				<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::KUSD),
				Some(kusd_location)
			)
		});
	}

	#[test]
	fn convert_ksm() {
		let ksm_location: MultiLocation = MultiLocation::parent().into();

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(ksm_location.clone()),
			Ok(CurrencyId::KSM),
		);

		Altair::execute_with(|| {
			assert_eq!(
				<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::KSM),
				Some(ksm_location)
			)
		});
	}

	#[test]
	fn convert_unkown_multilocation() {
		let unknown_location: MultiLocation = MultiLocation::new(
			1,
			X2(Parachain(parachains::altair::ID), GeneralKey([42].to_vec())),
		);

		assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location.clone()).is_err());
	}

	#[test]
	fn convert_unsupported_currency() {
		Altair::execute_with(|| {
			assert_eq!(
				<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Tranche(
					0,
					[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
				)),
				None
			)
		});
	}
}

// The fee associated with transferring AIR tokens
fn air_fee() -> Balance {
	let (_asset, fee) = AirPerSecond::get();
	// We divide the fee to align its unit and multiply by 4 as that seems to be the unit of
	// time the transfers take.
	// NOTE: it is possible that in different machines this value may differ. We shall see.
	fee * 8.div_euclid(10_000)
}

// The fee associated with transferring KUSD tokens
fn kusd_fee() -> Balance {
	let (_asset, fee) = KUsdPerSecond::get();
	// We divide the fee to align its unit and multiply by 4 as that seems to be the unit of
	// time the transfers take.
	fee.div_euclid(10_000) * 8
}

// The fee associated with transferring KSM tokens
fn ksm_fee() -> Balance {
	let (_asset, fee) = KsmPerSecond::get();
	// We divide the fee to align its unit and multiply by 8 as that seems to be the unit of
	// time the transfers take.
	fee.div_euclid(10_000) * 8
}
