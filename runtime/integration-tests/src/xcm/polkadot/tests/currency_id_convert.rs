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

use centrifuge_runtime::{
	Balances, CurrencyIdConvert, OrmlAssetRegistry, OrmlTokens, RuntimeOrigin, XTokens,
};
use cfg_primitives::{constants::currency_decimals, parachains, Balance};
use cfg_types::{
	tokens::{CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use cfg_utils::vec_to_fixed_array;
use codec::Encode;
use frame_support::{assert_noop, assert_ok};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::{
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_runtime::{
	traits::{ConstU32, Convert as C2},
	WeakBoundedVec,
};
use xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
	VersionedMultiLocation,
};
use xcm_emulator::TestExt;
use xcm_executor::traits::Convert as C1;

use super::register_dot;
use crate::xcm::polkadot::{
	setup::{
		acala_account, ausd, centrifuge_account, cfg, dot, foreign, sibling_account, ALICE,
		AUSD_ASSET_ID, BOB, DOT_ASSET_ID, NO_XCM_ASSET_ID, PARA_ID_SIBLING,
	},
	test_net::{Acala, Centrifuge, PolkadotNet, Sibling, TestNet},
	tests::{register_ausd, register_cfg, register_cfg_v2, register_no_xcm_token},
};

#[test]
fn convert_cfg() {
	assert_eq!(parachains::polkadot::centrifuge::CFG_KEY, &[0, 1]);

	Centrifuge::execute_with(|| {
		// The way CFG is represented relative within the Centrifuge runtime
		let cfg_location_inner: MultiLocation = MultiLocation::new(
			0,
			X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
		);

		register_cfg();

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(cfg_location_inner),
			Ok(CurrencyId::Native),
		);

		// The canonical way CFG is represented out in the wild
		let cfg_location_canonical: MultiLocation = MultiLocation::new(
			1,
			X2(
				Parachain(parachains::polkadot::centrifuge::ID),
				general_key(parachains::polkadot::centrifuge::CFG_KEY),
			),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
			Some(cfg_location_canonical)
		)
	});
}

/// Verify that even with CFG registered in the AssetRegistry with a XCM v2
/// MultiLocation, that `CurrencyIdConvert` can look it up given an identical
/// location in XCM v3.
#[test]
fn convert_cfg_xcm_v2() {
	assert_eq!(parachains::polkadot::centrifuge::CFG_KEY, &[0, 1]);

	Centrifuge::execute_with(|| {
		// Registered as xcm v2
		register_cfg_v2();

		// The way CFG is represented relative within the Centrifuge runtime in xcm v3
		let cfg_location_inner: MultiLocation = MultiLocation::new(
			0,
			X1(general_key(parachains::polkadot::centrifuge::CFG_KEY)),
		);

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(cfg_location_inner),
			Ok(CurrencyId::Native),
		);

		// The canonical way CFG is represented out in the wild
		let cfg_location_canonical: MultiLocation = MultiLocation::new(
			1,
			X2(
				Parachain(parachains::polkadot::centrifuge::ID),
				general_key(parachains::polkadot::centrifuge::CFG_KEY),
			),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Native),
			Some(cfg_location_canonical)
		)
	});
}

/// Verify that a registered token that is NOT XCM transferable is filtered out
/// by CurrencyIdConvert as expected.
#[test]
fn convert_no_xcm_token() {
	Centrifuge::execute_with(|| {
		register_no_xcm_token();

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(NO_XCM_ASSET_ID),
			None
		)
	});
}

#[test]
fn convert_ausd() {
	assert_eq!(parachains::polkadot::acala::AUSD_KEY, &[0, 1]);

	let ausd_location: MultiLocation = MultiLocation::new(
		1,
		X2(
			Parachain(parachains::polkadot::acala::ID),
			general_key(parachains::polkadot::acala::AUSD_KEY),
		),
	);

	Centrifuge::execute_with(|| {
		register_ausd();

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(ausd_location),
			Ok(AUSD_ASSET_ID),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(AUSD_ASSET_ID),
			Some(ausd_location)
		)
	});
}

#[test]
fn convert_dot() {
	let dot_location: MultiLocation = MultiLocation::parent();

	Centrifuge::execute_with(|| {
		register_dot();

		assert_eq!(
			<CurrencyIdConvert as C1<_, _>>::convert(dot_location),
			Ok(DOT_ASSET_ID),
		);

		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(DOT_ASSET_ID),
			Some(dot_location)
		)
	});
}

#[test]
fn convert_unknown_multilocation() {
	let unknown_location: MultiLocation = MultiLocation::new(
		1,
		X2(
			Parachain(parachains::polkadot::centrifuge::ID),
			general_key([42].as_ref()),
		),
	);

	Centrifuge::execute_with(|| {
		assert!(<CurrencyIdConvert as C1<_, _>>::convert(unknown_location).is_err());
	});
}

#[test]
fn convert_unsupported_currency() {
	Centrifuge::execute_with(|| {
		assert_eq!(
			<CurrencyIdConvert as C2<_, _>>::convert(CurrencyId::Tranche(
				0,
				[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
			)),
			None
		)
	});
}
