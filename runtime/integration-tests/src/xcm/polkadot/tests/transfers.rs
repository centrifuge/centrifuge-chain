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

use centrifuge_runtime::{Balances, OrmlAssetRegistry, OrmlTokens, RuntimeOrigin, XTokens};
use cfg_primitives::{constants::currency_decimals, parachains, Balance};
use cfg_types::{
	tokens::{CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use frame_support::assert_ok;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::{
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_runtime::traits::BadOrigin;
use xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId, WeightLimit},
	VersionedMultiLocation,
};
use xcm_emulator::TestExt;

use super::register_dot;
use crate::xcm::polkadot::{
	setup::{
		acala_account, ausd, centrifuge_account, cfg, dot, foreign, sibling_account, ALICE,
		AUSD_ASSET_ID, BOB, DOT_ASSET_ID, PARA_ID_SIBLING,
	},
	test_net::{Acala, Centrifuge, PolkadotNet, Sibling, TestNet},
	tests::register_ausd,
};

/*

NOTE: We hardcode the expected balances after an XCM operation given that the weights involved in
XCM execution often change slightly with each Polkadot update. We could simply test that the final
balance after some XCM operation is `initialBalance - amount - fee`, which would mean we would
never have to touch the tests again. However, by hard-coding these values we are forced to catch
an unexpectedly big change that would have a big impact on the weights and fees and thus balances,
which would go unnoticed and untreated otherwise.

 */

#[test]
fn transfer_cfg_to_sibling() {
	TestNet::reset();

	let alice_initial_balance = cfg(10);
	let bob_initial_balance = cfg(10);
	let transfer_amount = cfg(1);
	let transfer_amount = cfg(5);
	let cfg_in_sibling = CurrencyId::ForeignAsset(12);

	// CFG Metadata
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "Centrifuge".into(),
		symbol: "CFG".into(),
		existential_deposit: 1_000_000_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(
			1,
			X2(
				Parachain(parachains::polkadot::centrifuge::ID),
				general_key(parachains::polkadot::centrifuge::CFG_KEY),
			),
		))),
		additional: CustomMetadata::default(),
	};

	Centrifuge::execute_with(|| {
		assert_eq!(Balances::free_balance(&ALICE.into()), alice_initial_balance);
		assert_eq!(Balances::free_balance(&sibling_account()), 0);

		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta.clone(),
			Some(CurrencyId::Native),
		));
	});

	Sibling::execute_with(|| {
		assert_eq!(
			OrmlTokens::free_balance(cfg_in_sibling.clone(), &BOB.into()),
			0
		);

		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta,
			Some(cfg_in_sibling.clone())
		));
	});

	Centrifuge::execute_with(|| {
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			CurrencyId::Native,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(PARA_ID_SIBLING),
						Junction::AccountId32 {
							network: None,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000_000.into()),
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
		let current_balance = OrmlTokens::free_balance(cfg_in_sibling, &BOB.into());

		// Verify that BOB now has (amount transferred - fee)
		assert_eq!(current_balance, transfer_amount - fee(18));

		// Sanity check for the actual amount BOB ends up with
		assert_eq!(current_balance, 4991987200000000000);
	});
}

#[test]
fn transfer_cfg_sibling_to_centrifuge() {
	TestNet::reset();

	// In order to be able to transfer CFG from Sibling to Centrifuge, we need to first send
	// CFG from Centrifuge to Sibling, or else it fails since it'd be like Sibling had minted
	// CFG on their side.
	transfer_cfg_to_sibling();

	let alice_initial_balance = cfg(5);
	let bob_initial_balance = cfg(5) - cfg_fee();
	let transfer_amount = cfg(1);
	// Note: This asset was registered in `transfer_cfg_to_sibling`
	let cfg_in_sibling = CurrencyId::ForeignAsset(12);

	Centrifuge::execute_with(|| {
		assert_eq!(Balances::free_balance(&ALICE.into()), alice_initial_balance);
	});

	Sibling::execute_with(|| {
		assert_eq!(Balances::free_balance(&centrifuge_account()), 0);
		assert_eq!(
			OrmlTokens::free_balance(cfg_in_sibling.clone(), &BOB.into()),
			bob_initial_balance
		);
	});

	Sibling::execute_with(|| {
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(BOB.into()),
			cfg_in_sibling.clone(),
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						Junction::AccountId32 {
							network: None,
							id: ALICE.into(),
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000_000.into()),
		));

		// Confirm that Bobs's balance is initial balance - amount transferred
		assert_eq!(
			OrmlTokens::free_balance(cfg_in_sibling, &BOB.into()),
			bob_initial_balance - transfer_amount
		);
	});

	Centrifuge::execute_with(|| {
		// Verify that ALICE now has initial balance + amount transferred - fee
		assert_eq!(
			Balances::free_balance(&ALICE.into()),
			alice_initial_balance + transfer_amount - cfg_fee(),
		);
	});
}

#[test]
fn transfer_ausd_to_centrifuge() {
	TestNet::reset();

	let alice_initial_balance = ausd(10);
	let transfer_amount = ausd(7);

	Acala::execute_with(|| {
		register_ausd();

		assert_ok!(OrmlTokens::deposit(
			AUSD_ASSET_ID,
			&ALICE.into(),
			alice_initial_balance
		));

		assert_eq!(
			OrmlTokens::free_balance(AUSD_ASSET_ID, &centrifuge_account()),
			0
		);
	});

	Centrifuge::execute_with(|| {
		register_ausd();

		assert_eq!(OrmlTokens::free_balance(AUSD_ASSET_ID, &BOB.into()), 0,);
	});

	Acala::execute_with(|| {
		assert_eq!(
			OrmlTokens::free_balance(AUSD_ASSET_ID, &ALICE.into()),
			ausd(10),
		);
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			AUSD_ASSET_ID,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						Junction::AccountId32 {
							network: None,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000_000.into()),
		));

		assert_eq!(
			OrmlTokens::free_balance(AUSD_ASSET_ID, &ALICE.into()),
			alice_initial_balance - transfer_amount
		);

		// Verify that the amount transferred is now part of the centrifuge parachain account here
		assert_eq!(
			OrmlTokens::free_balance(AUSD_ASSET_ID, &centrifuge_account()),
			transfer_amount
		);
	});

	Centrifuge::execute_with(|| {
		// Verify that BOB now has initial balance + amount transferred - fee
		assert_eq!(
			OrmlTokens::free_balance(AUSD_ASSET_ID, &BOB.into()),
			transfer_amount - ausd_fee()
		);
	});
}

#[test]
fn transfer_dot_from_relay_chain() {
	let alice_initial_dot = dot(10);
	let transfer_amount: Balance = dot(3);

	Centrifuge::execute_with(|| {
		register_dot();
		assert_eq!(OrmlTokens::free_balance(DOT_ASSET_ID, &ALICE.into()), 0);
	});

	PolkadotNet::execute_with(|| {
		assert_eq!(
			polkadot_runtime::Balances::free_balance(&ALICE.into()),
			alice_initial_dot
		);

		assert_ok!(polkadot_runtime::XcmPallet::reserve_transfer_assets(
			polkadot_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(Parachain(parachains::polkadot::centrifuge::ID).into()),
			Box::new(
				Junction::AccountId32 {
					network: None,
					id: ALICE,
				}
				.into()
			),
			Box::new((Here, transfer_amount).into()),
			0
		));

		assert_eq!(
			polkadot_runtime::Balances::free_balance(&ALICE.into()),
			alice_initial_dot - transfer_amount
		);
	});

	Centrifuge::execute_with(|| {
		assert_eq!(
			OrmlTokens::free_balance(DOT_ASSET_ID, &ALICE.into()),
			transfer_amount - dot_fee()
		);
	});
}

#[test]
fn transfer_dot_to_relay_chain() {
	transfer_dot_from_relay_chain();

	Centrifuge::execute_with(|| {
		let alice_initial_dot = OrmlTokens::free_balance(DOT_ASSET_ID, &ALICE.into());

		assert_eq!(alice_initial_dot, dot(3) - dot_fee(),);

		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			DOT_ASSET_ID,
			dot(1),
			Box::new(
				MultiLocation::new(
					// nuno
					1,
					X1(Junction::AccountId32 {
						id: ALICE,
						network: None,
					})
				)
				.into()
			),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			OrmlTokens::free_balance(DOT_ASSET_ID, &ALICE.into()),
			alice_initial_dot - dot(1),
		);
	});

	PolkadotNet::execute_with(|| {
		assert_eq!(
			polkadot_runtime::Balances::free_balance(&ALICE.into()),
			79637471000
		);
	});
}

#[test]
fn transfer_foreign_sibling_to_centrifuge() {
	TestNet::reset();

	let alice_initial_balance = cfg(10);
	let sibling_asset_id = CurrencyId::ForeignAsset(1);
	let asset_location =
		MultiLocation::new(1, X2(Parachain(PARA_ID_SIBLING), general_key(&vec![0, 1])));
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "Sibling Native Token".into(),
		symbol: "SBLNG".into(),
		existential_deposit: 1_000_000_000_000,
		location: Some(VersionedMultiLocation::V3(asset_location.clone())),
		additional: CustomMetadata {
			xcm: XcmMetadata {
				// We specify a custom fee_per_second and verify below that this value is
				// used when XCM transfer fees are charged for this token.
				fee_per_second: Some(8420000000000000000),
			},
			..CustomMetadata::default()
		},
	};
	let transfer_amount = foreign(1, meta.decimals);

	Sibling::execute_with(|| {
		assert_eq!(OrmlTokens::free_balance(sibling_asset_id, &BOB.into()), 0);
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta.clone(),
			Some(CurrencyId::Native),
		));
	});

	Centrifuge::execute_with(|| {
		// First, register the asset in centrifuge
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta.clone(),
			Some(sibling_asset_id)
		));
	});

	Sibling::execute_with(|| {
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			CurrencyId::Native,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						Junction::AccountId32 {
							network: None,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000_000.into()),
		));

		// Confirm that Alice's balance is initial balance - amount transferred
		assert_eq!(
			Balances::free_balance(&ALICE.into()),
			alice_initial_balance - transfer_amount
		);
	});

	Centrifuge::execute_with(|| {
		let bob_balance = OrmlTokens::free_balance(sibling_asset_id, &BOB.into());

		// Verify that BOB now has initial balance + amount transferred - fee
		assert_eq!(
			bob_balance,
			transfer_amount - calc_fee(meta.additional.xcm.fee_per_second.unwrap())
		);
		// Sanity check to ensure the calculated is what is expected
		assert_eq!(bob_balance, 993264000000000000);
	});
}

#[test]
fn transfer_wormhole_usdc_acala_to_centrifuge() {
	TestNet::reset();

	let usdc_asset_id = CurrencyId::ForeignAsset(39);
	let asset_location = MultiLocation::new(
		1,
		X2(
			Parachain(parachains::polkadot::acala::ID),
			general_key("0x02f3a00dd12f644daec907013b16eb6d14bf1c4cb4".as_bytes()),
		),
	);
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 6,
		name: "Wormhole USDC".into(),
		symbol: "WUSDC".into(),
		existential_deposit: 1,
		location: Some(VersionedMultiLocation::V3(asset_location.clone())),
		additional: CustomMetadata::default(),
	};
	let transfer_amount = foreign(12, meta.decimals);
	let alice_initial_balance = transfer_amount * 100;

	Acala::execute_with(|| {
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta.clone(),
			Some(usdc_asset_id)
		));
		assert_ok!(OrmlTokens::deposit(
			usdc_asset_id,
			&ALICE.into(),
			alice_initial_balance
		));
		assert_eq!(
			OrmlTokens::free_balance(usdc_asset_id, &ALICE.into()),
			alice_initial_balance
		);
		assert_eq!(Balances::free_balance(&ALICE.into()), cfg(10));
	});

	Centrifuge::execute_with(|| {
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta.clone(),
			Some(usdc_asset_id)
		));
	});

	Acala::execute_with(|| {
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			usdc_asset_id,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						Junction::AccountId32 {
							network: None,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000.into()),
		));
		// Confirm that Alice's balance is initial balance - amount transferred
		assert_eq!(
			OrmlTokens::free_balance(usdc_asset_id, &ALICE.into()),
			alice_initial_balance - transfer_amount
		);
	});

	Centrifuge::execute_with(|| {
		let bob_balance = OrmlTokens::free_balance(usdc_asset_id, &BOB.into());

		// Sanity check to ensure the calculated is what is expected
		assert_eq!(bob_balance, 11991988);
	});
}

#[test]
fn test_total_fee() {
	assert_eq!(cfg_fee(), 8012800000000000);
}

fn cfg_fee() -> Balance {
	fee(currency_decimals::NATIVE)
}

fn ausd_fee() -> Balance {
	fee(currency_decimals::AUSD)
}

fn fee(decimals: u32) -> Balance {
	calc_fee(default_per_second(decimals))
}

// The fee associated with transferring DOT tokens
fn dot_fee() -> Balance {
	fee(10)
}

fn calc_fee(fee_per_second: Balance) -> Balance {
	// We divide the fee to align its unit and multiply by 4 as that seems to be the unit of
	// time the tests take.
	// NOTE: it is possible that in different machines this value may differ. We shall see.
	fee_per_second.div_euclid(10_000) * 8
}
