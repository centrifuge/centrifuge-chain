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

use altair_runtime::{Balances, OrmlAssetRegistry, OrmlTokens, RuntimeOrigin, XTokens};
use cfg_primitives::{constants::currency_decimals, parachains, Balance};
use cfg_types::{
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use frame_support::{assert_noop, assert_ok};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::{
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_runtime::traits::BadOrigin;
use xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
	prelude::{Parachain, X2},
	VersionedMultiLocation,
};
use xcm_emulator::TestExt;

use crate::xcm::kusama::{
	setup::{
		air, altair_account, ausd, foreign, karura_account, ksm, sibling_account, ALICE, BOB,
		PARA_ID_SIBLING,
	},
	test_net::{Altair, Karura, KusamaNet, Sibling, TestNet},
};

#[test]
fn register_air_works() {
	Altair::execute_with(|| {
		let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 18,
			name: "Altair".into(),
			symbol: "AIR".into(),
			existential_deposit: 1_000_000_000_000,
			location: Some(VersionedMultiLocation::V3(MultiLocation::new(
				0,
				X1(general_key(parachains::kusama::altair::AIR_KEY)),
			))),
			additional: CustomMetadata {
				transferability: CrossChainTransferability::Xcm(Default::default()),
				..CustomMetadata::default()
			},
		};

		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
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
			location: Some(VersionedMultiLocation::V3(MultiLocation::new(
				1,
				X2(
					Parachain(parachains::kusama::karura::ID),
					general_key(parachains::kusama::karura::AUSD_KEY),
				),
			))),
			additional: CustomMetadata {
				transferability: CrossChainTransferability::Xcm(Default::default()),
				..CustomMetadata::default()
			},
		};

		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
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
			decimals: 18,
			name: "Tranche Token 1".into(),
			symbol: "TRNCH".into(),
			existential_deposit: 1_000_000_000_000,
			location: Some(VersionedMultiLocation::V3(MultiLocation::new(
				1,
				X2(Parachain(2000), general_key(&[42])),
			))),
			additional: CustomMetadata {
				transferability: CrossChainTransferability::Xcm(Default::default()),
				..CustomMetadata::default()
			},
		};

		// It fails with `BadOrigin` even when submitted with `Origin::root` since we
		// only allow for tranche tokens to be registered through the pools pallet.
		let asset_id = CurrencyId::Tranche(42, [42u8; 16]);
		assert_noop!(
			OrmlAssetRegistry::register_asset(RuntimeOrigin::root(), meta, Some(asset_id)),
			BadOrigin
		);
	});
}
