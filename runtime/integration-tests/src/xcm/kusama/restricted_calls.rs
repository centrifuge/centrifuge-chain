// This file is part of Altair chain project.
//
// Altair is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Altair is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// Copyright 2021 Altair GmbH (altair.io).
// This file is part of Altair chain project.
//
// Altair is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Altair is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use crate::xcm::kusama::setup::{
	air, foreign, sibling_account, CurrencyId, ALICE, BOB, PARA_ID_SIBLING,
};
use crate::xcm::kusama::test_net::{Altair, KusamaNet, Sibling, TestNet};
use altair_runtime::{Balances, Call, CustomMetadata, Origin, PolkadotXcm, XTokens};
use frame_support::dispatch::Dispatchable;
use frame_support::{assert_err, assert_noop, assert_ok};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::xcm_fees::{default_per_second, ksm_per_second};
use runtime_common::{decimals, parachains, Balance, XcmMetadata};
use sp_runtime::DispatchError;
use sp_runtime::DispatchError::BadOrigin;
use xcm::latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId};
use xcm::v1::MultiAsset;
use xcm::v2::{AssetId, Fungibility};
use xcm::v2::{Instruction::WithdrawAsset, Xcm};
use xcm::VersionedMultiLocation;
use xcm_emulator::TestExt;

/// Verify that calls that must be blocked by the BaseCallFilter are indeed blocked.
pub mod blocked {
	use super::*;

	#[test]
	fn xtokens_transfer() {
		// For now, Tranche tokens are not supported in the XCM config so
		// we just safe-guard that trying to transfer a tranche token fails.
		// Once Tranche tokens are supported, we need to tighten this test.
		Altair::execute_with(|| {
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
		Altair::execute_with(|| {
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
				frame_system::Error::<altair_runtime::Runtime>::CallFiltered
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
		Altair::execute_with(|| {
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
