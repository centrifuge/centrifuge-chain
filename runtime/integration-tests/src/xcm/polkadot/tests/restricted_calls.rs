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

use centrifuge_runtime::{Balances, Call, Origin, PolkadotXcm, XTokens};
use cfg_primitives::{constants::currency_decimals, parachains, Balance};
use cfg_types::{CurrencyId, CustomMetadata, XcmMetadata};
use frame_support::{assert_err, assert_noop, assert_ok, dispatch::Dispatchable};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::{
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_runtime::{DispatchError, DispatchError::BadOrigin};
use xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
	v1::MultiAsset,
	v2::{AssetId, Fungibility, Instruction::WithdrawAsset, Xcm},
	VersionedMultiLocation,
};
use xcm_emulator::TestExt;

use crate::xcm::polkadot::{
	setup::{
		acala_account, ausd, centrifuge_account, cfg, dot, foreign, sibling_account, ALICE, BOB,
		DOT_ASSET_ID, PARA_ID_SIBLING,
	},
	test_net::{Acala, Centrifuge, PolkadotNet, Sibling, TestNet},
};

/// Verify that calls that must be blocked by the BaseCallFilter are indeed blocked.
pub mod blocked {
	use super::*;

	#[test]
	fn xtokens_transfer() {
		// For now, Tranche tokens are not supported in the XCM config so
		// we just safe-guard that trying to transfer a tranche token fails.
		// Once Tranche tokens are supported, we need to tighten this test.
		Centrifuge::execute_with(|| {
			assert!(XTokens::transfer(
				Origin::signed(ALICE.into()),
				CurrencyId::Tranche(401, [0; 16]),
				42,
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
			)
			.is_err());
		});
	}

	#[test]
	fn polkadot_xcm_send() {
		Centrifuge::execute_with(|| {
			assert_noop!(
				Call::dispatch(
					Call::PolkadotXcm(pallet_xcm::Call::send {
						dest: Box::new(
							MultiLocation::new(1, X1(Parachain(PARA_ID_SIBLING))).into()
						),
						message: Box::new(xcm::VersionedXcm::V2(Xcm::<Call>(vec![]).into())),
					}),
					Origin::signed(ALICE.into())
				),
				frame_system::Error::<centrifuge_runtime::Runtime>::CallFiltered
			);
		});
	}
}

/// Verify calls that must remain allowed. Sanity check to avoid us
/// from silently block calls we didn't mean to block.
pub mod allowed {
	use super::*;

	#[test]
	fn polkadot_xcm_force_xcm_version() {
		Centrifuge::execute_with(|| {
			assert_ok!(Call::dispatch(
				Call::PolkadotXcm(pallet_xcm::Call::force_xcm_version {
					location: Box::new(
						MultiLocation::new(1, X1(Parachain(PARA_ID_SIBLING))).into()
					),
					xcm_version: 2,
				}),
				Origin::root(),
			));
		});
	}
}
