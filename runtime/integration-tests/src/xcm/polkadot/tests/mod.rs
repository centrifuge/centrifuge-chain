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
	tokens::{CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use frame_support::assert_ok;
use orml_traits::asset_registry::AssetMetadata;
use runtime_common::{xcm::general_key, xcm_fees::ksm_per_second};
use xcm::{
	latest::MultiLocation,
	prelude::{Parachain, X2},
	VersionedMultiLocation,
};

use super::setup::DOT_ASSET_ID;
use crate::xcm::polkadot::setup::AUSD_ASSET_ID;

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

/// Register DOT in the asset registry.
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
