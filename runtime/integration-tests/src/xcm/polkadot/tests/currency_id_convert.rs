// // Copyright 2021 Centrifuge GmbH (centrifuge.io).
// // This file is part of Centrifuge chain project.
// //
// // Centrifuge is free software: you can redistribute it and/or modify
// // it under the terms of the GNU General Public License as published by
// // the Free Software Foundation, either version 3 of the License, or
// // (at your option) any later version (see http://www.gnu.org/licenses).
// // Centrifuge is distributed in the hope that it will be useful,
// // but WITHOUT ANY WARRANTY; without even the implied warranty of
// // MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// // GNU General Public License for more details.
//
// // Copyright 2021 Centrifuge GmbH (centrifuge.io).
// // This file is part of Centrifuge chain project.
// //
// // Centrifuge is free software: you can redistribute it and/or modify
// // it under the terms of the GNU General Public License as published by
// // the Free Software Foundation, either version 3 of the License, or
// // (at your option) any later version (see http://www.gnu.org/licenses).
// // Centrifuge is distributed in the hope that it will be useful,
// // but WITHOUT ANY WARRANTY; without even the implied warranty of
// // MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// // GNU General Public License for more details.
//
// use centrifuge_runtime::{
// 	Balances, CurrencyIdConvert, OrmlAssetRegistry, OrmlTokens, RuntimeOrigin, XTokens,
// };
// use cfg_primitives::{constants::currency_decimals, parachains, Balance};
// use cfg_types::{
// 	tokens::{CurrencyId, CustomMetadata},
// 	xcm::XcmMetadata,
// };
// use codec::Encode;
// use frame_support::{assert_noop, assert_ok};
// use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
// use runtime_common::{
// 	xcm::general_key,
// 	xcm_fees::{default_per_second, ksm_per_second},
// };
// use sp_runtime::{
// 	traits::{ConstU32, Convert as C2},
// 	WeakBoundedVec,
// };
// use xcm::{
// 	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
// 	VersionedMultiLocation,
// };
// use xcm_emulator::TestExt;
// use xcm_executor::traits::Convert as C1;
// use cfg_utils::vec_to_fixed_array;
//
// use super::register_dot;
// use crate::xcm::polkadot::{
// 	setup::{
// 		acala_account, ausd, centrifuge_account, cfg, dot, foreign, sibling_account, ALICE, BOB,
// 		DOT_ASSET_ID, PARA_ID_SIBLING,
// 	},
// 	test_net::{Acala, Centrifuge, PolkadotNet, Sibling, TestNet},
// };
//
// #[test]
// fn convert_cfg() {
// 	assert_eq!(parachains::polkadot::centrifuge::CFG_KEY, &[0, 1]);
//
// 	// The way CFG is represented relative within the Centrifuge runtime
// 	let cfg_location_inner: MultiLocation = MultiLocation::new(
// 		0,
// 		X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
// 	);
//
// 	assert_eq!(
// 		<CurrencyIdConvert as C1<_, _>>::convert(cfg_location_inner),
// 		Ok(CurrencyId::Native),
// 	);
//
// 	// The canonical way CFG is represented out in the wild
// 	let cfg_location_canonical: MultiLocation = MultiLocation::new(
// 		1,
// 		X2(
// 			Parachain(parachains::polkadot::centrifuge::ID),
// 			general_key(parachains::polkadot::centrifuge::CFG_KEY),
// 		),
// 	);
//
// 	Centrifuge::execute_with(|| {
// 		assert_eq!(
// 			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
// 			Some(cfg_location_canonical)
// 		)
// 	});
// }
//
// /// Verify that Tranche tokens are not handled by the CurrencyIdConvert
// /// since we don't allow Tranche tokens to be transferable through XCM.
// #[test]
// fn convert_tranche() {
// 	// We don't yet know the pools pallet index on the Centrifuge runtime
// 	const MOCK_POOLS_PALLET_INDEX: u8 = 42;
// 	let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
// 	let tranche_id =
// 		WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
// 	let tranche_multilocation = MultiLocation {
// 		parents: 1,
// 		interior: X3(
// 			Parachain(parachains::polkadot::centrifuge::ID),
// 			PalletInstance(MOCK_POOLS_PALLET_INDEX),
// 			GeneralKey { length: tranche_id.len() as u8, data: vec_to_fixed_array(tranche_id.to_vec()) },
// 		),
// 	};
//
// 	Centrifuge::execute_with(|| {
// 		assert_eq!(
// 			<CurrencyIdConvert as C1<_, _>>::convert(tranche_multilocation.clone()),
// 			Err(tranche_multilocation),
// 		);
// 	});
//
// 	Centrifuge::execute_with(|| {
// 		assert_eq!(
// 			<CurrencyIdConvert as C2<_, _>>::convert(tranche_currency),
// 			None
// 		)
// 	});
// }
//
// #[test]
// fn convert_ausd() {
// 	assert_eq!(parachains::polkadot::acala::AUSD_KEY, &[0, 1]);
//
// 	let ausd_location: MultiLocation = MultiLocation::new(
// 		1,
// 		X2(
// 			Parachain(parachains::polkadot::acala::ID),
// 			general_key(parachains::polkadot::acala::AUSD_KEY),
// 		),
// 	);
//
// 	Centrifuge::execute_with(|| {
// 		assert_eq!(
// 			<CurrencyIdConvert as C1<_, _>>::convert(ausd_location.clone()),
// 			Ok(CurrencyId::AUSD),
// 		);
//
// 		assert_eq!(
// 			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::AUSD),
// 			Some(ausd_location)
// 		)
// 	});
// }
//
// #[test]
// fn convert_dot() {
// 	let dot_location: MultiLocation = MultiLocation::parent().into();
//
// 	Centrifuge::execute_with(|| {
// 		register_dot();
//
// 		assert_eq!(
// 			<CurrencyIdConvert as C1<_, _>>::convert(dot_location.clone()),
// 			Ok(DOT_ASSET_ID),
// 		);
//
// 		assert_eq!(
// 			<CurrencyIdConvert as C2<_, _>>::convert(DOT_ASSET_ID),
// 			Some(dot_location)
// 		)
// 	});
// }
//
// #[test]
// fn convert_unkown_multilocation() {
// 	let unknown_location: MultiLocation = MultiLocation::new(
// 		1,
// 		X2(
// 			Parachain(parachains::polkadot::centrifuge::ID),
// 			general_key(&[42].to_vec()),
// 		),
// 	);
//
// 	Centrifuge::execute_with(|| {
// 		assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location.clone()).is_err());
// 	});
// }
//
// #[test]
// fn convert_unsupported_currency() {
// 	Centrifuge::execute_with(|| {
// 		assert_eq!(
// 			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Tranche(
// 				0,
// 				[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
// 			)),
// 			None
// 		)
// 	});
// }
