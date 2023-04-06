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
use cfg_primitives::{currency_decimals, parachains, AccountId, Balance, PoolId, TrancheId};
use cfg_traits::{Permissions as _, PoolMutate};
use cfg_types::{
	fixed_point::Rate,
	permissions::{PermissionScope, PoolRole, Role, UNION},
	tokens::{CurrencyId, CurrencyId::ForeignAsset, CustomMetadata, ForeignAssetId},
	xcm::XcmMetadata,
};
use codec::Encode;
use development_runtime::{
	Balances, Connectors, Loans, OrmlAssetRegistry, OrmlTokens, Permissions, PoolSystem,
	RuntimeOrigin, XTokens, XcmTransactor,
};
use frame_support::{assert_noop, assert_ok, dispatch::Weight, traits::Get};
use hex::FromHex;
use orml_traits::{asset_registry::AssetMetadata, FixedConversionRateProvider, MultiCurrency};
use pallet_connectors::{
	encoded_contract_call, Codec, Domain, DomainAddress, DomainLocator,
	Error::UnauthorizedTransfer, Message, ParachainId, Router, XcmDomain,
};
use pallet_pool_system::{
	pool_types::PoolDetails,
	tranches::{TrancheInput, TrancheLoc, TrancheMetadata, TrancheType},
};
use runtime_common::{xcm::general_key, xcm_fees::default_per_second};
use sp_core::H160;
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, ConstU32, One},
	BoundedVec, DispatchError, Perquintill, WeakBoundedVec,
};
use xcm_emulator::TestExt;

use crate::{
	xcm::development::{
		setup::{cfg, dollar, ALICE, BOB, PARA_ID_MOONBEAM},
		test_net::{Development, Moonbeam, RelayChain, TestNet},
	},
	*,
};

/// NOTE: We can't actually verify that the Connectors messages hits the ConnectorsXcmRouter
/// contract on Moonbeam since that would require a rather heavy e2e setup to emulate, involving
/// depending on Moonbeam's runtime, having said contract deployed to their evm environment, and
/// be able to query the evm side. Instead, these tests verify that - given all pre-requirements
/// are set up correctly - we succeed to send the Connectors message from the Centrifuge chain pov.
/// We have other unit tests verifying the Connectors' messages encoding and the encoding of the
/// remote EVM call to be executed on Moonbeam.

/// Verify that `Connectors::add_pool` succeeds when called with all the necessary requirements.
#[test]
fn add_pool() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();
		let pool_id: u64 = 42;

		// Verify that the pool must exist before we can call Connectors::add_pool
		assert_noop!(
			Connectors::add_pool(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				Domain::EVM(1284),
			),
			pallet_connectors::Error::<development_runtime::Runtime>::PoolNotFound
		);

		// Now create the pool
		utils::create_pool(pool_id);

		// Verify that we can now call Connectors::add_pool successfully
		assert_ok!(Connectors::add_pool(
			RuntimeOrigin::signed(ALICE.into()),
			pool_id,
			Domain::EVM(1284),
		));
	});
}

/// Verify that `Connectors::add_tranche` succeeds when called with all the necessary requirements.
/// We can't actually verify that the call hits the ConnectorsXcmRouter contract on Moonbeam
/// since that would require a very heavy e2e setup to emulate. Instead, here we test that we
/// can send the extrinsic and we have other unit tests verifying the encoding of the remote
/// EVM call to be executed on Moonbeam.
#[test]
fn add_tranche() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		// Now create the pool
		let pool_id: u64 = 42;
		utils::create_pool(pool_id);

		// Verify we can't call Connectors::add_tranche with a non-existing tranche_id
		let nonexistent_tranche = [71u8; 16];
		assert_noop!(
			Connectors::add_tranche(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id.clone(),
				nonexistent_tranche,
				Domain::EVM(1284),
			),
			pallet_connectors::Error::<development_runtime::Runtime>::TrancheNotFound
		);

		// Find the right tranche id
		let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
		let tranche_id = pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists");

		// Finally, verify we can call Connectors::add_tranche successfully
		// when given a valid pool + tranche id pair.
		assert_ok!(Connectors::add_tranche(
			RuntimeOrigin::signed(ALICE.into()),
			pool_id.clone(),
			tranche_id,
			Domain::EVM(1284),
		));
	});
}

#[test]
fn update_member() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		// Now create the pool
		let pool_id: u64 = 42;
		utils::create_pool(pool_id);

		// Find the right tranche id
		let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
		let tranche_id = pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists");

		// Finally, verify we can call Connectors::add_tranche successfully
		// when given a valid pool + tranche id pair.
		let new_member = DomainAddress::EVM(1284, [3; 20]);
		let valid_until = 2555583502;

		// Verify it fails if the origin is not a MemberListAdmin
		assert_noop!(
			Connectors::update_member(
				RuntimeOrigin::signed(ALICE.into()),
				new_member.clone(),
				pool_id.clone(),
				tranche_id.clone(),
				valid_until.clone(),
			),
			BadOrigin
		);

		// Make ALICE the MembersListAdmin of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			ALICE.into(),
			PermissionScope::Pool(pool_id.clone()),
			Role::PoolRole(PoolRole::MemberListAdmin),
		));

		// Verify it now works
		assert_ok!(Connectors::update_member(
			RuntimeOrigin::signed(ALICE.into()),
			new_member.clone(),
			pool_id.clone(),
			tranche_id.clone(),
			valid_until.clone(),
		));

		// Verify the Investor role was set as expected in Permissions
		assert!(Permissions::has(
			PermissionScope::Pool(pool_id.clone()),
			new_member.into_account_truncating(),
			Role::PoolRole(PoolRole::TrancheInvestor(
				tranche_id.clone(),
				valid_until.clone()
			)),
		));

		// Verify it can be called for another member
		assert_ok!(Connectors::update_member(
			RuntimeOrigin::signed(ALICE.into()),
			DomainAddress::EVM(1284, [9; 20]),
			pool_id.clone(),
			tranche_id.clone(),
			valid_until.clone(),
		));
	});
}

#[test]
fn update_token_price() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		// Now create the pool
		let pool_id: u64 = 42;
		utils::create_pool(pool_id);

		// Find the right tranche id
		let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
		let tranche_id = pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists");

		// Verify it now works
		assert_ok!(Connectors::update_token_price(
			RuntimeOrigin::signed(ALICE.into()),
			pool_id.clone(),
			tranche_id.clone(),
			Domain::EVM(1284),
		));
	});
}

#[test]
fn transfer() {
	TestNet::reset();

	Development::execute_with(|| {
		utils::setup_pre_requirements();

		// Now create the pool
		let pool_id: u64 = 42;
		utils::create_pool(pool_id);

		// Find the tranche id
		let pool_details = PoolSystem::pool(pool_id).expect("Pool should exist");
		let tranche_id = pool_details
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.expect("Tranche at index 0 exists");

		let dest_address = DomainAddress::EVM(1284, [99; 20]);

		// Verify that we first need the destination address to be whitelisted
		assert_noop!(
			Connectors::transfer(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id.clone(),
				tranche_id.clone(),
				dest_address.clone(),
				42,
			),
			pallet_connectors::Error::<development_runtime::Runtime>::UnauthorizedTransfer
		);

		// Make BOB the MembersListAdmin of this Pool
		assert_ok!(Permissions::add(
			RuntimeOrigin::root(),
			Role::PoolRole(PoolRole::PoolAdmin),
			BOB.into(),
			PermissionScope::Pool(pool_id.clone()),
			Role::PoolRole(PoolRole::MemberListAdmin),
		));

		// Call the Connectors::update_member which ensures the destination address is whitelisted.
		assert_ok!(Connectors::update_member(
			RuntimeOrigin::signed(BOB.into()),
			dest_address.clone(),
			pool_id.clone(),
			tranche_id.clone(),
			u64::MAX,
		));

		// Give BOB enough Tranche balance to be able to transfer it
		OrmlTokens::deposit(
			CurrencyId::Tranche(pool_id.clone(), tranche_id.clone()),
			&BOB.into(),
			100_000,
		);

		// Finally, verify that we can now transfer the tranche to the destination address
		let amount = 123;
		assert_ok!(Connectors::transfer(
			RuntimeOrigin::signed(BOB.into()),
			pool_id.clone(),
			tranche_id.clone(),
			dest_address.clone(),
			amount,
		));

		// The account to which the tranche should have been transferred
		// to on Centrifuge for bookkeeping purposes.
		let domain_account: AccountId = DomainLocator::<Domain> {
			domain: dest_address.clone().into(),
		}
		.into_account_truncating();

		// Verify that the correct amount of the Tranche token was transferred
		// to the dest domain account on Centrifuge.
		assert_eq!(
			OrmlTokens::free_balance(
				CurrencyId::Tranche(pool_id.clone(), tranche_id.clone()),
				&domain_account
			),
			amount
		)
	});
}

#[test]
fn test_vec_to_fixed_array() {
	let src = "TrNcH".as_bytes().to_vec();
	let symbol: [u8; 32] = cfg_utils::vec_to_fixed_array(src.clone());

	assert!(symbol.starts_with("TrNcH".as_bytes()));
	assert_eq!(
		symbol,
		[
			84, 114, 78, 99, 72, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0
		]
	);
}

#[test]
fn encoded_ethereum_xcm_add_pool() {
	// Ethereum_xcm with Connectors::hande(Message::AddPool) as `input` - this was our first
	// successfully ethereum_xcm encoded call tested in Moonbase.
	let expected_encoded_hex = "26000060ae0a00000000000000000000000000000000000000000000000000000000000100ce0cb9bb900dfd0d378393a041f3abab6b18288200000000000000000000000000000000000000000000000000000000000000009101bf48bcb600000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000009010000000000bce1a4000000000000000000000000000000000000000000000000";

	let moonbase_location = MultiLocation {
		parents: 1,
		interior: X1(Parachain(1000)),
	};
	// 38 is the pallet index, 0 is the `transact` extrinsic index.
	let ethereum_xcm_transact_call_index = BoundedVec::truncate_from(vec![38, 0]);
	let contract_address = H160::from(
		<[u8; 20]>::from_hex("cE0Cb9BB900dfD0D378393A041f3abAb6B182882").expect("Decoding failed"),
	);
	let domain_info = XcmDomain {
		location: VersionedMultiLocation::V1(moonbase_location.clone()),
		ethereum_xcm_transact_call_index,
		contract_address,
		fee_currency: ForeignAsset(1),
		max_gas_limit: 700_000,
	};

	let connectors_message =
		Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddPool { pool_id: 12378532 };

	let contract_call = encoded_contract_call(connectors_message.serialize());
	let encoded_call = Connectors::encoded_ethereum_xcm_call(domain_info, contract_call);
	let encoded_call_hex = hex::encode(encoded_call);

	assert_eq!(encoded_call_hex, expected_encoded_hex);
}

// Verify that the max tranche token symbol and name lengths are what the Connectors pallet expects.
#[test]
fn verify_tranche_fields_sizes() {
	assert_eq!(
		cfg_types::consts::pools::MaxTrancheNameLengthBytes::get(),
		pallet_connectors::TOKEN_NAME_SIZE as u32
	);
	assert_eq!(
		cfg_types::consts::pools::MaxTrancheSymbolLengthBytes::get(),
		pallet_connectors::TOKEN_SYMBOL_SIZE as u32
	);
}

mod utils {
	use super::*;

	pub fn setup_pre_requirements() {
		let moonbeam_location = MultiLocation {
			parents: 1,
			interior: X1(Parachain(PARA_ID_MOONBEAM)),
		};
		let moonbeam_native_token = MultiLocation {
			parents: 1,
			interior: X2(Parachain(PARA_ID_MOONBEAM), general_key(&[0, 1])),
		};

		// We need to set the Transact info for Moonbeam in the XcmTransactor pallet
		assert_ok!(XcmTransactor::set_transact_info(
			RuntimeOrigin::root(),
			Box::new(VersionedMultiLocation::V1(moonbeam_location.clone())),
			1,
			8_000_000_000_000_000,
			Some(3)
		));

		assert_ok!(XcmTransactor::set_fee_per_second(
			RuntimeOrigin::root(),
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
			RuntimeOrigin::root(),
			meta,
			Some(glmr_currency_id.clone())
		));

		// Give Alice and BOB enough glimmer to pay for fees
		OrmlTokens::deposit(glmr_currency_id, &ALICE.into(), 10 * dollar(18));
		OrmlTokens::deposit(glmr_currency_id, &BOB.into(), 10 * dollar(18));

		assert_ok!(Connectors::set_domain_router(
			RuntimeOrigin::root(),
			Domain::EVM(1284),
			Router::Xcm(XcmDomain {
				location: moonbeam_location
					.clone()
					.try_into()
					.expect("Bad xcm version"),
				ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
				contract_address: H160::from(
					<[u8; 20]>::from_hex("cE0Cb9BB900dfD0D378393A041f3abAb6B182882")
						.expect("Invalid address"),
				),
				fee_currency: glmr_currency_id,
				max_gas_limit: 700_000,
			}),
		));

		// We first need to register AUSD in the asset registry
		let ausd_meta: AssetMetadata<Balance, CustomMetadata> = AssetMetadata {
			decimals: 12,
			name: "Acala Dollar".into(),
			symbol: "AUSD".into(),
			existential_deposit: 1_000,
			location: None,
			additional: CustomMetadata::default(),
		};
		assert_ok!(OrmlAssetRegistry::register_asset(
			RuntimeOrigin::root(),
			ausd_meta,
			Some(CurrencyId::AUSD)
		));
	}

	pub fn create_pool(pool_id: u64) {
		assert_ok!(PoolSystem::create(
			BOB.into(),
			BOB.into(),
			pool_id.clone(),
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						// NOTE: For now, we have to set these metadata fields of the first tranche
						// to be convertible to the 32-byte size expected by the connectors AddTranche
						// message.
						token_name: BoundedVec::<
							u8,
							cfg_types::consts::pools::MaxTrancheNameLengthBytes,
						>::try_from("A highly advanced tranche".as_bytes().to_vec(),)
						.expect(""),
						token_symbol: BoundedVec::<
							u8,
							cfg_types::consts::pools::MaxTrancheSymbolLengthBytes,
						>::try_from("TrNcH".as_bytes().to_vec())
						.expect(""),
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
		));
	}
}
