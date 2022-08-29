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

use crate::xcm::polkadot::setup::{
	acala_account, ausd, centrifuge_account, cfg, dot, foreign, sibling_account, CurrencyId, ALICE,
	BOB, DOT_ASSET_ID, PARA_ID_SIBLING,
};
use crate::xcm::polkadot::test_net::{Acala, Centrifuge, PolkadotNet, Sibling, TestNet};
use centrifuge_runtime::{
	Balances, CustomMetadata, Origin, OrmlAssetRegistry, OrmlTokens, XTokens,
};
use frame_support::{assert_noop, assert_ok, error::BadOrigin};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::{xcm::general_key, xcm_fees::{default_per_second, ksm_per_second}};
use runtime_common::{decimals, parachains, Balance, XcmMetadata};
use xcm::latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId};
use xcm::VersionedMultiLocation;
use xcm_emulator::TestExt;

#[test]
fn register_cfg_works() {
	Centrifuge::execute_with(|| {
		let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 18,
			name: "Centrifuge".into(),
			symbol: "CFG".into(),
			existential_deposit: 1_000_000_000_000,
			location: Some(VersionedMultiLocation::V1(MultiLocation::new(
				0,
				X1(general_key(
					parachains::polkadot::centrifuge::CFG_KEY,
				)),
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
	Centrifuge::execute_with(|| {
		let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 12,
			name: "Acala Dollar".into(),
			symbol: "AUSD".into(),
			existential_deposit: 1_000_000_000_000,
			location: Some(VersionedMultiLocation::V1(MultiLocation::new(
				1,
				X2(
					Parachain(2000),
					general_key(parachains::polkadot::centrifuge::CFG_KEY),
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
	Centrifuge::execute_with(|| {
		let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 12,
			name: "Tranche Token 1".into(),
			symbol: "TRNCH".into(),
			existential_deposit: 1_000_000_000_000,
			location: Some(VersionedMultiLocation::V1(MultiLocation::new(
				1,
				X2(Parachain(2000), general_key(&[42])),
			))),
			additional: CustomMetadata::default(),
		};

		// It fails with `BadOrigin` even when submitted with `Origin::root` since we only
		// allow for tranche tokens to be registered through the pools pallet.
		let asset_id = CurrencyId::Tranche(42, [42u8; 16]);
		assert_noop!(
			OrmlAssetRegistry::register_asset(Origin::root(), meta.clone(), Some(asset_id.clone())),
			BadOrigin
		);
	});
}
