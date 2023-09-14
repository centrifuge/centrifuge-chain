// Copyright 2021 Development GmbH (centrifuge.io).
// This file is part of Development chain project.
//
// Development is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Development is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// Copyright 2021 Development GmbH (centrifuge.io).
// This file is part of Development chain project.
//
// Development is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Development is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::{constants::currency_decimals, parachains, Balance, PoolId, TrancheId};
use cfg_traits::{liquidity_pools::Codec, TryConvert};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Rate,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use cfg_utils::vec_to_fixed_array;
use codec::Encode;
use development_runtime::{
	Balances, LiquidityPoolsGateway, LocationToAccountId, OrmlAssetRegistry, OrmlTokens,
	Runtime as DevelopmentRuntime, RuntimeCall as DevelopmentRuntimeCall, RuntimeOrigin, System,
	XTokens, XcmTransactor,
};
use frame_support::{assert_ok, traits::fungible::Mutate};
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use pallet_liquidity_pools::Message;
use pallet_xcm_transactor::{Currency, CurrencyPayment, TransactWeights};
use runtime_common::{
	account_conversion::AccountConverter,
	xcm::general_key,
	xcm_fees::{default_per_second, ksm_per_second},
};
use sp_core::{bounded::BoundedVec, ConstU32, H160};
use sp_runtime::traits::BadOrigin;
use xcm::{
	latest::{Junction, Junction::*, Junctions::*, MultiLocation, NetworkId, WeightLimit},
	v3::{OriginKind, Weight},
	VersionedMultiLocation, VersionedXcm,
};
use xcm_emulator::TestExt;

use crate::liquidity_pools::pallet::{
	development::{
		setup::{
			centrifuge_account, cfg, dollar, moonbeam_account, ALICE, BOB, CHARLIE,
			PARA_ID_MOONBEAM,
		},
		test_net::{Development, Moonbeam, RelayChain, TestNet},
		tests::register_ausd,
	},
	xcm_metadata,
};

/*

NOTE: We hardcode the expected balances after an XCM operation given that the weights involved in
XCM execution often change slightly with each Polkadot update. We could simply test that the final
balance after some XCM operation is `initialBalance - amount - fee`, which would mean we would
never have to touch the tests again. However, by hard-coding these values we are forced to catch
an unexpectedly big change that would have a big impact on the weights and fees and thus balances,
which would go unnoticed and untreated otherwise.

 */

#[test]
fn incoming_xcm() {
	TestNet::reset();

	let test_address = H160::from_low_u64_be(345654);
	let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddCurrency {
		currency: 0,
		evm_address: test_address.0,
	};

	let lp_gateway_msg = BoundedVec::<
		u8,
		<DevelopmentRuntime as pallet_liquidity_pools_gateway::Config>::MaxIncomingMessageSize,
	>::try_from(msg.serialize())
	.expect("msg should convert to BoundedVec");

	let call = DevelopmentRuntimeCall::LiquidityPoolsGateway(
		pallet_liquidity_pools_gateway::Call::process_msg {
			msg: lp_gateway_msg,
		},
	);

	let cfg_in_sibling = CurrencyId::ForeignAsset(12);

	transfer_cfg_to_sibling();

	let dest = MultiLocation::new(1, X1(Parachain(parachains::polkadot::centrifuge::ID)));

	Development::execute_with(|| {
		assert_ok!(LiquidityPoolsGateway::add_relayer(
			RuntimeOrigin::root(),
			DomainAddress::EVM(
				1282,
				H160::from_slice(&BOB.as_ref()[0..20]).to_fixed_bytes()
			)
		));
	});

	Moonbeam::execute_with(|| {
		XcmTransactor::set_transact_info(
			RuntimeOrigin::root(),
			Box::new(dest.into()),
			Default::default(),
			Default::default(),
			Default::default(),
		)?;

		XcmTransactor::set_fee_per_second(
			RuntimeOrigin::root(),
			Box::new(VersionedMultiLocation::V3(MultiLocation::new(
				1,
				X2(
					Parachain(parachains::polkadot::centrifuge::ID),
					general_key(parachains::polkadot::centrifuge::CFG_KEY),
				),
			))),
			100,
		)
	});

	Moonbeam::execute_with(|| {
		assert_ok!(XcmTransactor::transact_through_signed(
			RuntimeOrigin::signed(BOB.into()),
			Box::new(dest.into()),
			CurrencyPayment {
				currency: Currency::AsCurrencyId(cfg_in_sibling),
				fee_amount: Some(155548480000000000),
			},
			call.encode().into(),
			TransactWeights {
				transact_required_weight_at_most: Weight::from_all(12530000000),
				overall_weight: Some(Weight::from_all(15530000000)),
			},
		));
	});

	Development::execute_with(|| {
		let events = System::events();

		assert!(events.len() > 0)
	});
}

#[test]
fn transfer_cfg_to_sibling() {
	TestNet::reset();

	let alice_initial_balance = cfg(10_000);
	let bob_initial_balance = cfg(10_000);
	let transfer_amount = cfg(5);
	let cfg_in_sibling = CurrencyId::ForeignAsset(12);

	// CFG Metadata
	let meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
		decimals: 18,
		name: "Development".into(),
		symbol: "CFG".into(),
		existential_deposit: 1_000_000_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::new(
			1,
			X2(
				Parachain(parachains::polkadot::centrifuge::ID),
				general_key(parachains::polkadot::centrifuge::CFG_KEY),
			),
		))),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::Xcm(Default::default()),
			..CustomMetadata::default()
		},
	};

	let location = MultiLocation {
		parents: 1,
		interior: X2(
			Parachain(PARA_ID_MOONBEAM),
			AccountId32 {
				network: None,
				id: BOB.into(),
			},
		),
	};

	let bob_acc =
		AccountConverter::<DevelopmentRuntime, LocationToAccountId>::try_convert(location).unwrap();

	Development::execute_with(|| {
		assert_ok!(Balances::mint_into(&bob_acc.into(), 1000 * dollar(18)));
		assert_eq!(Balances::free_balance(&ALICE.into()), alice_initial_balance);
		assert_eq!(Balances::free_balance(&moonbeam_account()), 0);

		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta.clone(),
			Some(CurrencyId::Native),
		));
	});

	Moonbeam::execute_with(|| {
		assert_eq!(OrmlTokens::free_balance(cfg_in_sibling, &BOB.into()), 0);

		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			meta,
			Some(cfg_in_sibling)
		));
	});

	Development::execute_with(|| {
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			CurrencyId::Native,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(PARA_ID_MOONBEAM),
						Junction::AccountId32 {
							network: None,
							id: BOB,
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000_000.into()),
		));

		// Confirm that Alice's balance is initial balance - amount transferred
		assert_eq!(
			Balances::free_balance(&ALICE.into()),
			alice_initial_balance - transfer_amount
		);

		// Verify that the amount transferred is now part of the sibling account here
		assert_eq!(Balances::free_balance(&moonbeam_account()), transfer_amount);
	});

	Moonbeam::execute_with(|| {
		let current_balance = OrmlTokens::free_balance(cfg_in_sibling, &BOB.into());

		// Verify that BOB now has (amount transferred - fee)
		assert_eq!(current_balance, transfer_amount - fee(18));

		// Sanity check for the actual amount BOB ends up with
		assert_eq!(current_balance, 4991987200000000000);
	});
}

#[test]
fn transfer_cfg_sibling_to_centrifuge() {
	TestNet::reset();

	// In order to be able to transfer CFG from Moonbeam to Development, we need to
	// first send CFG from Development to Moonbeam, or else it fails since it'd be
	// like Moonbeam had minted CFG on their side.
	transfer_cfg_to_sibling();

	let alice_initial_balance = 9995000000000000000000;
	let bob_initial_balance = cfg(5) - cfg_fee();
	let transfer_amount = cfg(4);
	// Note: This asset was registered in `transfer_cfg_to_sibling`
	let cfg_in_sibling = CurrencyId::ForeignAsset(12);

	Development::execute_with(|| {
		assert_eq!(Balances::free_balance(&ALICE.into()), alice_initial_balance);
	});

	Moonbeam::execute_with(|| {
		assert_eq!(Balances::free_balance(&centrifuge_account()), 0);
		assert_eq!(
			OrmlTokens::free_balance(cfg_in_sibling, &BOB.into()),
			bob_initial_balance
		);

		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(BOB.into()),
			cfg_in_sibling,
			transfer_amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(parachains::polkadot::centrifuge::ID),
						Junction::AccountId32 {
							network: None,
							id: CHARLIE,
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(8_000_000_000_000.into()),
		));

		// Confirm that Charlie's balance is initial balance - amount transferred
		assert_eq!(
			OrmlTokens::free_balance(cfg_in_sibling, &BOB.into()),
			bob_initial_balance - transfer_amount
		);
	});

	Development::execute_with(|| {
		// Verify that Charlie's balance equals the amount transferred - fee
		assert_eq!(
			Balances::free_balance(&CHARLIE.into()),
			transfer_amount - cfg_fee(),
		);
	});
}

#[test]
fn test_total_fee() {
	assert_eq!(cfg_fee(), 8012800000000000);
}

fn cfg_fee() -> Balance {
	fee(currency_decimals::NATIVE)
}

fn ausd_fee() -> Balance {
	fee(currency_decimals::AUSD)
}

fn fee(decimals: u32) -> Balance {
	calc_fee(default_per_second(decimals))
}

// The fee associated with transferring DOT tokens
fn dot_fee() -> Balance {
	fee(10)
}

fn calc_fee(fee_per_second: Balance) -> Balance {
	// We divide the fee to align its unit and multiply by 4 as that seems to be the
	// unit of time the tests take.
	// NOTE: it is possible that in different machines this value may differ. We
	// shall see.
	fee_per_second.div_euclid(10_000) * 8
}
