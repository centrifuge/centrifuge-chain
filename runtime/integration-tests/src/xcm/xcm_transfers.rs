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

use frame_support::assert_ok;
use xcm::latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId};
use xcm::VersionedMultiLocation;
use xcm_emulator::TestExt;

use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};

use crate::xcm::setup::{
	air, altair_account, ausd, foreign, karura_account, ksm, sibling_account, CurrencyId, ALICE,
	BOB, PARA_ID_SIBLING,
};
use crate::xcm::test_net::{Altair, Karura, KusamaNet, Sibling, TestNet};

use altair_runtime::{Balances, CustomMetadata, Origin, OrmlAssetRegistry, OrmlTokens, XTokens};
use runtime_common::xcm_fees::{default_per_second, ksm_per_second};
use runtime_common::{decimals, parachains, Balance, XcmMetadata};

#[test]
fn transfer_air_to_sibling() {
	TestNet::reset();

	let alice_initial_balance = air(10);
	let bob_initial_balance = air(10);
	let transfer_amount = air(1);
	let transfer_amount = air(5);
	let air_in_sibling = CurrencyId::ForeignAsset(12);

	Altair::execute_with(|| {
		assert_eq!(Balances::free_balance(&ALICE.into()), alice_initial_balance);
		assert_eq!(Balances::free_balance(&sibling_account()), 0);
	});

	Sibling::execute_with(|| {
		assert_eq!(
			OrmlTokens::free_balance(air_in_sibling.clone(), &BOB.into()),
			0
		);

		// Register AIR as foreign asset in the sibling parachain
		let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 18,
			name: "Altair".into(),
			symbol: "AIR".into(),
			existential_deposit: 1_000_000_000_000,
			location: Some(VersionedMultiLocation::V1(MultiLocation::new(
				1,
				X2(
					Parachain(parachains::altair::ID),
					GeneralKey(parachains::altair::AIR_KEY.to_vec()),
				),
			))),
			additional: CustomMetadata::default(),
		};
		assert_ok!(OrmlAssetRegistry::register_asset(
			Origin::root(),
			meta,
			Some(air_in_sibling.clone())
		));
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
		let current_balance = OrmlTokens::free_balance(air_in_sibling, &BOB.into());

		// Verify that BOB now has (amount transferred - fee)
		assert_eq!(current_balance, transfer_amount - fee(18));

		// Sanity check for the actual amount BOB ends up with
		assert_eq!(current_balance, 4990676000000000000);
	});
}

#[test]
fn transfer_air_sibling_to_altair() {
	TestNet::reset();

	// In order to be able to transfer AIR from Sibling to Altair, we need to first send
	// AIR from Altair to Sibling, or else it fails since it'd be like Sibling had minted
	// AIR on their side.
	transfer_air_to_sibling();

	let alice_initial_balance = air(5);
	let bob_initial_balance = air(5) - air_fee();
	let transfer_amount = air(1);
	// Note: This asset was registered in `transfer_air_to_sibling`
	let air_in_sibling = CurrencyId::ForeignAsset(12);

	Altair::execute_with(|| {
		assert_eq!(Balances::free_balance(&ALICE.into()), alice_initial_balance);
	});

	Sibling::execute_with(|| {
		assert_eq!(Balances::free_balance(&altair_account()), 0);
		assert_eq!(
			OrmlTokens::free_balance(air_in_sibling.clone(), &BOB.into()),
			bob_initial_balance
		);
	});

	Sibling::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(BOB.into()),
			air_in_sibling.clone(),
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(parachains::altair::ID),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: ALICE.into(),
						}
					)
				)
				.into()
			),
			8_000_000_000_000,
		));

		// Confirm that Bobs's balance is initial balance - amount transferred
		assert_eq!(
			OrmlTokens::free_balance(air_in_sibling, &BOB.into()),
			bob_initial_balance - transfer_amount
		);
	});

	Altair::execute_with(|| {
		// Verify that ALICE now has initial balance + amount transferred - fee
		assert_eq!(
			Balances::free_balance(&ALICE.into()),
			alice_initial_balance + transfer_amount - air_fee(),
		);
	});
}

#[test]
fn transfer_kusd_to_altair() {
	TestNet::reset();

	let alice_initial_balance = ausd(10);
	let bob_initial_balance = ausd(10);
	let transfer_amount = ausd(7);

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
						Parachain(parachains::altair::ID),
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

		// Sanity check the actual balance
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::KUSD, &BOB.into()),
			16990676000000
		);
	});
}

#[test]
fn transfer_ksm_from_relay_chain() {
	let transfer_amount: Balance = ksm(1);

	KusamaNet::execute_with(|| {
		assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
			kusama_runtime::Origin::signed(ALICE.into()),
			Box::new(Parachain(parachains::altair::ID).into().into()),
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
			ksm(1),
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
			999988476752
		);
	});
}

#[test]
fn transfer_foreign_sibling_to_altair() {
	TestNet::reset();

	let alice_initial_balance = air(10);
	let sibling_asset_id = CurrencyId::ForeignAsset(1);
	let asset_location =
		MultiLocation::new(1, X2(Parachain(PARA_ID_SIBLING), GeneralKey(vec![0, 1])));
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "Sibling Native Token".into(),
		symbol: "SBLNG".into(),
		existential_deposit: 1_000_000_000_000,
		location: Some(VersionedMultiLocation::V1(asset_location.clone())),
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
		assert_eq!(OrmlTokens::free_balance(sibling_asset_id, &BOB.into()), 0)
	});

	Altair::execute_with(|| {
		// First, register the asset in altair
		assert_ok!(OrmlAssetRegistry::register_asset(
			Origin::root(),
			meta.clone(),
			Some(sibling_asset_id)
		));
	});

	Sibling::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::Native,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(parachains::altair::ID),
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
	});

	Altair::execute_with(|| {
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

pub mod asset_registry {
	use super::{assert_ok, parachains, GeneralKey, MultiLocation, X1};
	use crate::xcm::test_net::{Altair, TestNet};
	use altair_runtime::{Balance, CurrencyId, CustomMetadata, Origin, OrmlAssetRegistry};
	use frame_support::assert_noop;
	use orml_traits::asset_registry::AssetMetadata;
	use orml_traits::currency::MultiCurrency;
	use sp_runtime::traits::BadOrigin;
	use xcm::prelude::{Parachain, X2};
	use xcm::VersionedMultiLocation;
	use xcm_emulator::TestExt;

	#[test]
	fn register_air_works() {
		Altair::execute_with(|| {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 18,
				name: "Altair".into(),
				symbol: "AIR".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V1(MultiLocation::new(
					0,
					X1(GeneralKey(parachains::altair::AIR_KEY.to_vec())),
				))),
				additional: CustomMetadata::default(),
			};

			assert_ok!(OrmlAssetRegistry::register_asset(
				Origin::root(),
				meta,
				Some(CurrencyId::Native)
			));
		});
	}

	#[test]
	fn register_foreign_asset_works() {
		Altair::execute_with(|| {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 12,
				name: "Acala Dollar".into(),
				symbol: "AUSD".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V1(MultiLocation::new(
					1,
					X2(
						Parachain(2000),
						GeneralKey(parachains::altair::AIR_KEY.to_vec()),
					),
				))),
				additional: CustomMetadata::default(),
			};

			assert_ok!(OrmlAssetRegistry::register_asset(
				Origin::root(),
				meta,
				Some(CurrencyId::ForeignAsset(42))
			));
		});
	}

	#[test]
	// Verify that registering tranche tokens is not allowed through extrinsics
	fn register_tranche_asset_blocked() {
		Altair::execute_with(|| {
			let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
				decimals: 12,
				name: "Tranche Token 1".into(),
				symbol: "TRNCH".into(),
				existential_deposit: 1_000_000_000_000,
				location: Some(VersionedMultiLocation::V1(MultiLocation::new(
					1,
					X2(Parachain(2000), GeneralKey(vec![42])),
				))),
				additional: CustomMetadata::default(),
			};

			// It fails with `BadOrigin` even when submitted with `Origin::root` since we only
			// allow for tranche tokens to be registered through the pools pallet.
			let asset_id = CurrencyId::Tranche(42, [42u8; 16]);
			assert_noop!(
				OrmlAssetRegistry::register_asset(
					Origin::root(),
					meta.clone(),
					Some(asset_id.clone())
				),
				BadOrigin
			);
		});
	}
}

#[test]
fn test_total_fee() {
	assert_eq!(air_fee(), 9324000000000000);
	assert_eq!(fee(decimals::AUSD), 9324000000);
	assert_eq!(fee(decimals::KSM), 9324000000);
}

pub mod currency_id_convert {
	use super::*;
	use altair_runtime::CurrencyIdConvert;
	use sp_runtime::traits::Convert as C2;
	use xcm::VersionedMultiLocation;
	use xcm_executor::traits::Convert as C1;

	#[test]
	fn convert_air() {
		assert_eq!(parachains::altair::AIR_KEY.to_vec(), vec![0, 1]);

		// The way AIR is represented relative within the Altair runtime
		let air_location_inner: MultiLocation =
			MultiLocation::new(0, X1(GeneralKey(parachains::altair::AIR_KEY.to_vec())));

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(air_location_inner),
			Ok(CurrencyId::Native),
		);

		// The canonical way AIR is represented out in the wild
		let air_location_canonical: MultiLocation = MultiLocation::new(
			1,
			X2(
				Parachain(parachains::altair::ID),
				GeneralKey(parachains::altair::AIR_KEY.to_vec()),
			),
		);

		Altair::execute_with(|| {
			assert_eq!(
				<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
				Some(air_location_canonical)
			)
		});
	}

	#[test]
	fn convert_ausd() {
		assert_eq!(parachains::karura::KUSD_KEY.to_vec(), vec![0, 129]);

		let kusd_location: MultiLocation = MultiLocation::new(
			1,
			X2(
				Parachain(parachains::karura::ID),
				GeneralKey(parachains::karura::KUSD_KEY.to_vec()),
			),
		);

		Altair::execute_with(|| {
			assert_eq!(
				<CurrencyIdConvert as C1<_, _>>::convert(kusd_location.clone()),
				Ok(CurrencyId::KUSD),
			);

			assert_eq!(
				<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::KUSD),
				Some(kusd_location)
			)
		});
	}

	#[test]
	fn convert_ksm() {
		let ksm_location: MultiLocation = MultiLocation::parent().into();

		Altair::execute_with(|| {
			assert_eq!(
				<CurrencyIdConvert as C1<_, _>>::convert(ksm_location.clone()),
				Ok(CurrencyId::KSM),
			);

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

		Altair::execute_with(|| {
			assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location.clone()).is_err());
		});
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

fn air_fee() -> Balance {
	fee(decimals::NATIVE)
}

fn kusd_fee() -> Balance {
	fee(decimals::AUSD)
}

fn fee(decimals: u32) -> Balance {
	calc_fee(default_per_second(decimals))
}

// The fee associated with transferring KSM tokens
fn ksm_fee() -> Balance {
	calc_fee(ksm_per_second())
}

fn calc_fee(fee_per_second: Balance) -> Balance {
	// We divide the fee to align its unit and multiply by 4 as that seems to be the unit of
	// time the transfers take.
	// NOTE: it is possible that in different machines this value may differ. We shall see.
	fee_per_second.div_euclid(10_000) * 8
}
