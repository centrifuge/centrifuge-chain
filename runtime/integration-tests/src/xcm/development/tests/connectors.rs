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

use ::xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
	prelude::{Parachain, X1, X2},
	VersionedMultiLocation,
};
use cfg_primitives::{currency_decimals, parachains, Balance, PoolId, TrancheId};
use cfg_types::{
	CurrencyId, CurrencyId::ForeignAsset, CustomMetadata, ForeignAssetId, Rate, XcmMetadata,
};
use codec::Encode;
use development_runtime::{
	Balances, Connectors, Origin, OrmlAssetRegistry, OrmlTokens, PoolSystem, XTokens, XcmTransactor,
};
use frame_support::{assert_noop, assert_ok, dispatch::Weight};
use hex::FromHex;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use pallet_connectors::{encoded_contract_call, Domain, Message, ParachainId, Router, XcmDomain};
use pallet_pool_system::{TrancheInput, TrancheMetadata, TrancheType};
use runtime_common::{xcm::general_key, xcm_fees::default_per_second};
use sp_core::H160;
use sp_runtime::{
	traits::{BadOrigin, One},
	BoundedVec, Perquintill,
};
use xcm_emulator::TestExt;

use crate::{
	xcm::development::{
		setup::{cfg, dollar, ALICE, BOB, PARA_ID_MOONBEAM},
		test_net::{Development, Moonbeam, RelayChain, TestNet},
	},
	*,
};

/// Verify that `Connectors::add_pool` succeeds when called with all the necessary requirements.
/// We can't actually verify that the call hits the ConnectorsXcmRouter contract on Moonbeam
/// since that would require a very heavy e2e setup to emulate. Instead, here we test that we
/// can send the extrinsic and we have other unit tests verifying the encoding of the remote
/// EVM call to be executed on Moonbeam.
#[test]
fn add_pool_works() {
	TestNet::reset();

	let moonbeam_location = MultiLocation {
		parents: 1,
		interior: X1(Parachain(PARA_ID_MOONBEAM)),
	};
	let moonbeam_native_token = MultiLocation {
		parents: 1,
		interior: X2(Parachain(PARA_ID_MOONBEAM), general_key(&[0, 1])),
	};

	Development::execute_with(|| {
		// We need to set the Transact info for Moonbeam in the XcmTransact pallet
		assert_ok!(XcmTransactor::set_transact_info(
			Origin::root(),
			Box::new(VersionedMultiLocation::V1(moonbeam_location.clone())),
			1,
			8_000_000_000_000_000,
			Some(3)
		));

		assert_ok!(XcmTransactor::set_fee_per_second(
			Origin::root(),
			Box::new(VersionedMultiLocation::V1(moonbeam_native_token.clone())),
			default_per_second(18), // default fee_per_second for this token which has 18 decimals
		));

		/// Register Moonbeam's native token
		let glmr_currency_id = CurrencyId::ForeignAsset(1);
		let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 18,
			name: "Glimmer".into(),
			symbol: "GLMR".into(),
			existential_deposit: 1_000_000,
			location: Some(VersionedMultiLocation::V1(moonbeam_native_token)),
			additional: CustomMetadata::default(),
		};

		assert_ok!(OrmlAssetRegistry::register_asset(
			Origin::root(),
			meta,
			Some(glmr_currency_id.clone())
		));

		// Give Alice enough glimmer to pay for fees
		OrmlTokens::deposit(glmr_currency_id, &ALICE.into(), 10 * dollar(18));

		assert_ok!(Connectors::set_domain_router(
			Origin::root(),
			Domain::Parachain(ParachainId::Moonbeam),
			Router::Xcm(XcmDomain {
				location: moonbeam_location
					.clone()
					.try_into()
					.expect("Bad xcm version"),
				ethereum_xcm_transact_call_index: vec![38, 0],
				contract_address: H160::from(
					<[u8; 20]>::from_hex("cE0Cb9BB900dfD0D378393A041f3abAb6B182882")
						.expect("Invalid address"),
				),
				fee_currency: glmr_currency_id,
			}),
		));

		// Register the pool
		let pool_id = 42;

		// we first need to register AUSD in the asset registry
		let ausd_meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 12,
			name: "Acala Dollar".into(),
			symbol: "AUSD".into(),
			existential_deposit: 1_000,
			location: None,
			additional: CustomMetadata::default(),
		};
		assert_ok!(OrmlAssetRegistry::register_asset(
			Origin::root(),
			ausd_meta,
			Some(CurrencyId::AUSD)
		));

		// then we can create the pool
		assert_ok!(PoolSystem::create(
			Origin::signed(BOB.into()),
			BOB.into(),
			pool_id,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: One::one(),
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				}
			],
			CurrencyId::AUSD,
			10_000 * dollar(currency_decimals::AUSD),
			None
		));

		// Finally, verify that with all the requirements set in place,
		// we can call Connectors::add_pool.
		assert_ok!(Connectors::add_pool(
			Origin::signed(ALICE.into()),
			pool_id,
			Domain::Parachain(ParachainId::Moonbeam),
		));
	});
}

#[test]
fn encoded_ethereum_xcm_add_pool() {
	// Ethereum_xcm with Connectors::hande(Message::AddPool) as `input` - this was our first
	// successfully ethereum_xcm encoded call tested in Moonbase.
	let expected_encoded_hex = "26000080380100000000000000000000000000000000000000000000000000000000000100ce0cb9bb900dfd0d378393a041f3abab6b18288200000000000000000000000000000000000000000000000000000000000000009101bf48bcb600000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000009010000000000bce1a4000000000000000000000000000000000000000000000000";
	let _expected_encoded = hex::decode(expected_encoded_hex).expect("Decode failed");

	let moonbase_location = MultiLocation {
		parents: 1,
		interior: X1(Parachain(1000)),
	};
	// 38 is the pallet index, 0 is the `transact` extrinsic index.
	let ethereum_xcm_transact_call_index = vec![38, 0];
	let contract_address = H160::from(
		<[u8; 20]>::from_hex("cE0Cb9BB900dfD0D378393A041f3abAb6B182882").expect("Decoding failed"),
	);
	let domain_info = XcmDomain {
		location: VersionedMultiLocation::V1(moonbase_location.clone()),
		ethereum_xcm_transact_call_index,
		contract_address,
		fee_currency: ForeignAsset(1),
	};

	let connectors_message =
		Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddPool { pool_id: 12378532 };

	let contract_call = encoded_contract_call(connectors_message.encode());
	let encoded_call = Connectors::encoded_ethereum_xcm_call(domain_info, contract_call);
	let encoded_call_hex = hex::encode(encoded_call);

	assert_eq!(encoded_call_hex, expected_encoded_hex);
}
