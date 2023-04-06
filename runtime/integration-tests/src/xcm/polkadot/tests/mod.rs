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
use cfg_primitives::Balance;
use cfg_types::{
	tokens::{CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use frame_support::assert_ok;
use orml_traits::asset_registry::AssetMetadata;
use runtime_common::xcm_fees::ksm_per_second;
use xcm::{latest::MultiLocation, VersionedMultiLocation};

use super::setup::DOT_ASSET_ID;

mod asset_registry;
mod currency_id_convert;
mod restricted_calls;
mod transfers;

/// Register DOT in the asset registry.
/// It should be executed within an externalities environment.
fn register_dot() {
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 12,
		name: "Polkadot".into(),
		symbol: "DOT".into(),
		existential_deposit: 100_000_000,
		location: Some(VersionedMultiLocation::V1(MultiLocation::parent())),
		additional: CustomMetadata {
			xcm: XcmMetadata {
				// We specify a custom fee_per_second and verify below that this value is
				// used when XCM transfer fees are charged for this token.
				fee_per_second: Some(ksm_per_second()),
			},
			..CustomMetadata::default()
		},
	};
	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(DOT_ASSET_ID)
	));
}
