// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

use ::xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId},
	prelude::{Parachain, X1, X2},
	VersionedMultiLocation,
};
use cfg_primitives::{PoolId, TrancheId};
use cfg_traits::connectors::OutboundQueue;
use cfg_types::{
	domain_address::Domain,
	fixed_point::Rate,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use connectors_gateway_routers::{
	ethereum_xcm::EthereumXCMRouter, DomainRouter, EVMChain, EVMDomain, FeeValues, XcmDomain,
	XcmTransactInfo,
};
use frame_support::{assert_noop, assert_ok};
use hex::FromHex;
use orml_traits::{asset_registry::AssetMetadata, MultiCurrency};
use pallet_connectors::Message;
use runtime_common::{xcm::general_key, xcm_fees::default_per_second};
use sp_core::{bounded::BoundedVec, H160};
use xcm_emulator::TestExt;

use crate::{
	chain::centrifuge::{
		Balance, ConnectorsGateway, OrmlAssetRegistry, OrmlTokens, Runtime, RuntimeOrigin,
	},
	connectors_gateway::routers::ethereum_xcm::{
		setup::{dollar, ALICE, BOB, CHARLIE, PARA_ID_MOONBEAM, TEST_DOMAIN},
		test_net::{Development, Moonbeam, RelayChain, TestNet},
	},
	utils::accounts::Keyring,
};

fn setup() {
	let moonbeam_location = MultiLocation {
		parents: 1,
		interior: X1(Parachain(PARA_ID_MOONBEAM)),
	};
	let moonbeam_native_token = MultiLocation {
		parents: 1,
		interior: X2(Parachain(PARA_ID_MOONBEAM), general_key(&[0, 1])),
	};

	/// Register Moonbeam's native token
	let glmr_currency_id = CurrencyId::ForeignAsset(1);
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "Glimmer".into(),
		symbol: "GLMR".into(),
		existential_deposit: 1_000_000,
		location: Some(VersionedMultiLocation::V3(moonbeam_native_token)),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Xcm(Default::default()),
			..CustomMetadata::default()
		},
	};

	let ethereum_xcm_router = EthereumXCMRouter::<Runtime> {
		xcm_domain: XcmDomain {
			location: Box::new(
				moonbeam_location
					.try_into()
					.expect("Bad xcm domain location"),
			),
			ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
			contract_address: H160::from(
				<[u8; 20]>::from_hex("cE0Cb9BB900dfD0D378393A041f3abAb6B182882")
					.expect("Invalid address"),
			),
			max_gas_limit: 700_000,
			transact_info: XcmTransactInfo {
				transact_extra_weight: 1.into(),
				max_weight: 8_000_000_000_000_000.into(),
				transact_extra_weight_signed: Some(3.into()),
			},
			fee_currency: glmr_currency_id,
			fee_per_second: default_per_second(18),
			fee_asset_location: Box::new(
				moonbeam_native_token
					.try_into()
					.expect("Bad xcm fee asset location"),
			),
		},
		_marker: Default::default(),
	};

	let domain_router = DomainRouter::EthereumXCM(ethereum_xcm_router);

	assert_ok!(ConnectorsGateway::set_domain_router(
		RuntimeOrigin::root(),
		TEST_DOMAIN,
		domain_router,
	));

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(glmr_currency_id)
	));

	// Give Alice and BOB enough glimmer to pay for fees
	OrmlTokens::deposit(glmr_currency_id, &ALICE.into(), 10 * dollar(18));
	OrmlTokens::deposit(glmr_currency_id, &BOB.into(), 10 * dollar(18));

	// We first need to register AUSD in the asset registry
	let ausd_meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 12,
		name: "Acala Dollar".into(),
		symbol: "AUSD".into(),
		existential_deposit: 1_000,
		location: None,
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Xcm(Default::default()),
			..CustomMetadata::default()
		},
	};
	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		ausd_meta,
		Some(CurrencyId::AUSD)
	));
}

#[test]
fn call() {
	TestNet::reset();

	Development::execute_with(|| {
		setup();

		let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::Transfer {
			currency: 0,
			sender: ALICE.into(),
			receiver: BOB.into(),
			amount: 1_000u128,
		};

		assert_ok!(<ConnectorsGateway as OutboundQueue>::submit(
			TEST_DOMAIN,
			ALICE.into(),
			msg.clone(),
		));

		assert_noop!(
			<ConnectorsGateway as OutboundQueue>::submit(
				Domain::EVM(1285),
				ALICE.into(),
				msg.clone(),
			),
			pallet_connectors_gateway::Error::<Runtime>::RouterNotFound,
		);

		assert_noop!(
			<ConnectorsGateway as OutboundQueue>::submit(TEST_DOMAIN, CHARLIE.into(), msg),
			pallet_xcm_transactor::Error::<Runtime>::UnableToWithdrawAsset,
		);
	});
}
