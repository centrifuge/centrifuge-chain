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

use centrifuge_runtime::{OrmlAssetRegistry, RuntimeOrigin};
use cfg_primitives::{parachains, Balance};
use cfg_types::{
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use frame_support::assert_ok;
use orml_traits::asset_registry::AssetMetadata;
use runtime_common::{xcm::general_key, xcm_fees::ksm_per_second};
use sp_core::{bounded::WeakBoundedVec, ConstU32};
use xcm::{
	latest::MultiLocation,
	prelude::{Parachain, X2},
	VersionedMultiLocation,
};

use crate::xcm::polkadot::setup::{AUSD_ASSET_ID, DOT_ASSET_ID, NO_XCM_ASSET_ID};

mod asset_registry;
mod currency_id_convert;
mod restricted_calls;
mod transfers;

/// Register DOT in the asset registry.
/// It should be executed within an externalities environment.
fn register_dot() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 10,
		name: "Polkadot".into(),
		symbol: "DOT".into(),
		existential_deposit: 100_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::parent())),
		additional: CustomMetadata::default(),
	};
	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(DOT_ASSET_ID)
	));
}

/// Register AUSD in the asset registry.
/// It should be executed within an externalities environment.
fn register_ausd() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 12,
		name: "Acala Dollar".into(),
		symbol: "AUSD".into(),
		existential_deposit: 1_000_000_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(
			1,
			X2(
				Parachain(parachains::polkadot::acala::ID),
				general_key(parachains::polkadot::acala::AUSD_KEY),
			),
		))),
		additional: CustomMetadata::default(),
	};

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(AUSD_ASSET_ID)
	));
}

/// Register CFG in the asset registry.
/// It should be executed within an externalities environment.
fn register_cfg() {
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

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(CurrencyId::Native)
	));
}

/// Register CFG in the asset registry as XCM v2, just like it is in production.
/// It should be executed within an externalities environment.
fn register_cfg_v2() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "Centrifuge".into(),
		symbol: "CFG".into(),
		existential_deposit: 1_000_000_000_000,
		location: Some(VersionedMultiLocation::V2(xcm::v2::MultiLocation::new(
			1,
			xcm::v2::Junctions::X2(
				xcm::v2::Junction::Parachain(parachains::polkadot::centrifuge::ID),
				xcm::v2::Junction::GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
					parachains::polkadot::centrifuge::CFG_KEY.into(),
					None,
				)),
			),
		))),
		additional: CustomMetadata::default(),
	};

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(CurrencyId::Native)
	));
}

/// Register a token whose `CrossChainTransferability` does NOT include XCM.
fn register_no_xcm_token() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "NO XCM".into(),
		symbol: "NXCM".into(),
		existential_deposit: 1_000_000_000_000,
		location: None,
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Connectors,
			..CustomMetadata::default()
		},
	};

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(NO_XCM_ASSET_ID)
	));
}
