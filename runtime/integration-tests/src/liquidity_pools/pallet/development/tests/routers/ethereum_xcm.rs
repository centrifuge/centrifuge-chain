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
use cfg_traits::liquidity_pools::OutboundQueue;
use cfg_types::{
	domain_address::Domain,
	fixed_point::Quantity,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use frame_support::{assert_noop, assert_ok};
use hex::FromHex;
use liquidity_pools_gateway_routers::{
	ethereum_xcm::EthereumXCMRouter, AxelarXCMRouter, DomainRouter, EVMDomain, FeeValues,
	XCMRouter, XcmDomain, XcmTransactInfo,
};
use orml_traits::{asset_registry::AssetMetadata, MultiCurrency};
use pallet_liquidity_pools::Message;
use runtime_common::{xcm::general_key, xcm_fees::default_per_second};
use sp_core::{bounded::BoundedVec, H160};
use xcm_emulator::TestExt;

use crate::{
	chain::centrifuge::{
		Balance, LiquidityPoolsGateway, OrmlAssetRegistry, OrmlTokens, Runtime, RuntimeOrigin,
	},
	liquidity_pools::pallet::development::{
		setup::{dollar, ALICE, BOB, CHARLIE, PARA_ID_MOONBEAM, TEST_DOMAIN},
		test_net::{Development, Moonbeam, RelayChain, TestNet},
		tests::routers::axelar_evm::TEST_EVM_CHAIN,
	},
	utils::{accounts::Keyring, AUSD_CURRENCY_ID},
};

#[test]
fn submit_ethereum_xcm() {
	submit_test_fn(get_ethereum_xcm_router_fn());
}

#[test]
fn submit_axelar_xcm() {
	submit_test_fn(get_axelar_xcm_router_fn());
}

fn submit_test_fn(router_creation_fn: RouterCreationFn) {
	TestNet::reset();

	Development::execute_with(|| {
		setup(router_creation_fn);

		let msg = Message::<Domain, PoolId, TrancheId, Balance, Quantity>::Transfer {
			currency: 0,
			sender: ALICE.into(),
			receiver: BOB.into(),
			amount: 1_000u128,
		};

		assert_ok!(<LiquidityPoolsGateway as OutboundQueue>::submit(
			ALICE.into(),
			TEST_DOMAIN,
			msg.clone(),
		));

		assert_noop!(
			<LiquidityPoolsGateway as OutboundQueue>::submit(
				ALICE.into(),
				Domain::EVM(1285),
				msg.clone(),
			),
			pallet_liquidity_pools_gateway::Error::<Runtime>::RouterNotFound,
		);
	});
}

type RouterCreationFn = Box<dyn Fn(VersionedMultiLocation, CurrencyId) -> DomainRouter<Runtime>>;

fn get_axelar_xcm_router_fn() -> RouterCreationFn {
	Box::new(
		|location: VersionedMultiLocation, currency_id: CurrencyId| -> DomainRouter<Runtime> {
			let router = AxelarXCMRouter::<Runtime> {
				router: XCMRouter {
					xcm_domain: XcmDomain {
						location: Box::new(location.try_into().expect("Bad xcm domain location")),
						ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
						contract_address: H160::from_low_u64_be(11),
						max_gas_limit: 700_000,
						transact_required_weight_at_most: Default::default(),
						overall_weight: Default::default(),
						fee_currency: currency_id,
						fee_amount: dollar(18).saturating_div(5),
					},
					_marker: Default::default(),
				},
				axelar_target_chain: TEST_EVM_CHAIN.clone(),
				axelar_target_contract: H160::from_low_u64_be(111),
				_marker: Default::default(),
			};

			DomainRouter::AxelarXCM(router)
		},
	)
}

fn get_ethereum_xcm_router_fn() -> RouterCreationFn {
	Box::new(
		|location: VersionedMultiLocation, currency_id: CurrencyId| -> DomainRouter<Runtime> {
			let router = EthereumXCMRouter::<Runtime> {
				router: XCMRouter {
					xcm_domain: XcmDomain {
						location: Box::new(location.try_into().expect("Bad xcm domain location")),
						ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
						contract_address: H160::from_low_u64_be(11),
						max_gas_limit: 700_000,
						transact_required_weight_at_most: Default::default(),
						overall_weight: Default::default(),
						fee_currency: currency_id,
						fee_amount: dollar(18).saturating_div(5),
					},
					_marker: Default::default(),
				},
				_marker: Default::default(),
			};

			DomainRouter::EthereumXCM(router)
		},
	)
}

fn setup(router_creation_fn: RouterCreationFn) {
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

	let domain_router = router_creation_fn(moonbeam_location.into(), glmr_currency_id);

	assert_ok!(LiquidityPoolsGateway::set_domain_router(
		RuntimeOrigin::root(),
		TEST_DOMAIN,
		domain_router,
	));

	assert_ok!(OrmlAssetRegistry::register_asset(
		RuntimeOrigin::root(),
		meta,
		Some(glmr_currency_id)
	));

	// Fund the gateway sender account with enough glimmer to pay for fees
	OrmlTokens::deposit(
		glmr_currency_id,
		&<Runtime as pallet_liquidity_pools_gateway::Config>::Sender::get(),
		1_000_000_000_000 * dollar(18),
	);

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
		Some(AUSD_CURRENCY_ID)
	));
}
