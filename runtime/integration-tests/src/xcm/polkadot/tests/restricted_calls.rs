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

use centrifuge_runtime::{Balances, Multisig, PolkadotXcm, RuntimeCall, RuntimeOrigin, XTokens};
use cfg_primitives::{constants::currency_decimals, parachains, Balance};
use cfg_types::{
	tokens::{CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use codec::Encode;
use frame_support::{
	assert_err, assert_noop, assert_ok, dispatch::Dispatchable, traits::WrapperKeepOpaque,
};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use runtime_common::xcm_fees::{default_per_second, ksm_per_second};
use sp_runtime::{DispatchError, DispatchError::BadOrigin};
use xcm::{
	latest::{
		AssetId, Fungibility, Junction, Junction::*, Junctions::*, MultiAsset, MultiLocation,
		NetworkId, WeightLimit,
	},
	v2::{Instruction::WithdrawAsset, Xcm},
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

/// Verify that calls that would allow for Tranche token to be transferred
/// through XCM fail because the underlying CurrencyIdConvert doesn't handle
/// Tranche tokens.
pub mod blocked {
	use cfg_utils::vec_to_fixed_array;
	use frame_support::weights::Weight;
	use sp_runtime::{traits::ConstU32, WeakBoundedVec};
	use xcm::{latest::MultiAssets, VersionedMultiAsset, VersionedMultiAssets};

	use super::*;

	#[test]
	fn xtokens_transfer() {
		// For now, Tranche tokens are not supported in the XCM config so
		// we just safe-guard that trying to transfer a tranche token fails.
		Centrifuge::execute_with(|| {
			assert_noop!(
				XTokens::transfer(
					RuntimeOrigin::signed(ALICE.into()),
					CurrencyId::Tranche(401, [0; 16]),
					42,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(PARA_ID_SIBLING),
								Junction::AccountId32 {
									network: None,
									id: BOB.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				),
				orml_xtokens::Error::<altair_runtime::Runtime>::NotCrossChainTransferableCurrency
			);
		});
	}

	// Verify that trying to transfer Tranche tokens using their MultiLocation
	// representation also fails.
	#[test]
	fn xtokens_transfer_multiasset() {
		use codec::Encode;

		let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
		let tranche_id =
			WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
		let tranche_location = MultiLocation {
			parents: 1,
			interior: X3(
				Parachain(123),
				PalletInstance(42),
				GeneralKey {
					length: tranche_id.len() as u8,
					data: vec_to_fixed_array(tranche_id.to_vec()),
				},
			),
		};
		let tranche_multi_asset = VersionedMultiAsset::from(MultiAsset::from((
			AssetId::Concrete(tranche_location),
			Fungibility::Fungible(42),
		)));

		Centrifuge::execute_with(|| {
			assert_noop!(
				XTokens::transfer_multiasset(
					RuntimeOrigin::signed(ALICE.into()),
					Box::new(tranche_multi_asset),
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(PARA_ID_SIBLING),
								Junction::AccountId32 {
									network: None,
									id: BOB.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				),
				orml_xtokens::Error::<altair_runtime::Runtime>::XcmExecutionFailed
			);
		});
	}

	#[test]
	fn xtokens_transfer_multiassets() {
		use codec::Encode;

		let tranche_currency = CurrencyId::Tranche(401, [0; 16]);
		let tranche_id =
			WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);
		let tranche_location = MultiLocation {
			parents: 1,
			interior: X3(
				Parachain(123),
				PalletInstance(42),
				GeneralKey {
					length: tranche_id.len() as u8,
					data: vec_to_fixed_array(tranche_id.to_vec()),
				},
			),
		};
		let tranche_multi_asset = MultiAsset::from((
			AssetId::Concrete(tranche_location),
			Fungibility::Fungible(42),
		));

		Centrifuge::execute_with(|| {
			assert_noop!(
				XTokens::transfer_multiassets(
					RuntimeOrigin::signed(ALICE.into()),
					Box::new(VersionedMultiAssets::from(MultiAssets::from(vec![
						tranche_multi_asset
					]))),
					0,
					Box::new(
						MultiLocation::new(
							1,
							X2(
								Parachain(PARA_ID_SIBLING),
								Junction::AccountId32 {
									network: None,
									id: BOB.into(),
								}
							)
						)
						.into()
					),
					WeightLimit::Limited(8_000_000_000_000.into()),
				),
				orml_xtokens::Error::<altair_runtime::Runtime>::XcmExecutionFailed
			);
		});
	}
}
