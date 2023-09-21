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

use std::collections::BTreeMap;

use axelar_gateway_precompile::SourceConverter;
use cfg_primitives::{Balance, PoolId, TrancheId, CFG};
use cfg_traits::{ethereum::EthereumTransactor, liquidity_pools::Codec};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Rate,
	tokens::{CurrencyId, CustomMetadata, GeneralCurrencyIndex},
};
use codec::Encode;
use ethabi::{Contract, Function, Param, ParamType, Token};
use ethereum::{LegacyTransaction, TransactionAction, TransactionSignature, TransactionV2};
use frame_support::{assert_err, assert_ok, dispatch::RawOrigin};
use fudge::primitives::Chain;
use hex::ToHex;
use orml_traits::{asset_registry::AssetMetadata, MultiCurrency};
use pallet_evm::{AddressMapping, FeeCalculator};
use pallet_liquidity_pools::Message;
use runtime_common::{account_conversion::AccountConverter, evm::precompile::LP_AXELAR_GATEWAY};
use sp_core::{Get, H160, H256, U256};
use sp_runtime::traits::{BlakeTwo256, Hash};
use tokio::runtime::Handle;
use xcm::{v3::MultiLocation, VersionedMultiLocation};

use crate::{
	chain::centrifuge::{
		AccountId, CouncilCollective, FastTrackVotingPeriod, MinimumDeposit, Runtime, RuntimeCall,
		RuntimeEvent, RuntimeOrigin, PARA_ID,
	},
	evm::ethereum_transaction::TEST_CONTRACT_CODE,
	utils::{
		env,
		env::{ChainState, EventRange, TestEnv},
		evm::{deploy_contract, mint_balance_into_derived_account},
	},
};

#[tokio::test]
async fn axelar_precompile_execute() {
	let mut env = env::test_env_default(Handle::current());

	env.evolve().unwrap();

	let currency_id = CurrencyId::ForeignAsset(123456);

	let sender_address = H160::from_low_u64_be(1_000_002);

	mint_balance_into_derived_account(&mut env, sender_address, 1_000_000 * CFG);

	let derived_sender_account = env
		.with_state(Chain::Para(PARA_ID), || {
			<Runtime as pallet_evm::Config>::AddressMapping::into_account_id(sender_address)
		})
		.unwrap();

	let receiver_address = H160::from_low_u64_be(1_000_003);

	let derived_receiver_account = env
		.with_state(Chain::Para(PARA_ID), || {
			<Runtime as pallet_evm::Config>::AddressMapping::into_account_id(receiver_address)
		})
		.unwrap();

	env.with_state(Chain::Para(PARA_ID), || {
		let derived_receiver_balance =
			orml_tokens::Pallet::<Runtime>::free_balance(currency_id, &derived_receiver_account);

		assert_eq!(derived_receiver_balance, 0)
	})
	.unwrap();

	let source_address = H160::from_low_u64_be(1111);
	let evm_chain_name = String::from("Ethereum");
	let evm_chain_id = 0;

	let currency_metadata = AssetMetadata {
		decimals: 18,
		name: "Test".into(),
		symbol: "TST".into(),
		existential_deposit: 1_000_000,
		location: Some(VersionedMultiLocation::V3(MultiLocation::here())),
		additional: CustomMetadata {
			transferability: Default::default(),
			mintable: true,
			permissioned: false,
			pool_currency: false,
		},
	};

	env.with_mut_state(Chain::Para(PARA_ID), || {
		orml_asset_registry::Pallet::<Runtime>::register_asset(
			RuntimeOrigin::root(),
			currency_metadata,
			Some(currency_id),
		)
		.unwrap();

		orml_tokens::Pallet::<Runtime>::deposit(
			currency_id,
			&derived_sender_account,
			1_000_000_000_000 * 10u128.saturating_pow(18),
		)
		.unwrap();
	})
	.unwrap();

	let general_currency_id = env
		.with_state(Chain::Para(PARA_ID), || {
			pallet_liquidity_pools::Pallet::<Runtime>::try_get_general_index(currency_id).unwrap()
		})
		.unwrap();

	let transfer_amount = 100;
	let msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::Transfer {
		currency: general_currency_id,
		sender: derived_sender_account.clone().into(),
		receiver: derived_receiver_account.clone().into(),
		amount: transfer_amount,
	};

	env.with_mut_state(Chain::Para(PARA_ID), || {
		axelar_gateway_precompile::Pallet::<Runtime>::set_gateway(
			RuntimeOrigin::root(),
			sender_address,
		)
		.unwrap();

		axelar_gateway_precompile::Pallet::<Runtime>::set_converter(
			RuntimeOrigin::root(),
			BlakeTwo256::hash(evm_chain_name.as_bytes()),
			SourceConverter {
				domain: Domain::EVM(evm_chain_id),
			},
		)
		.unwrap();

		pallet_liquidity_pools_gateway::Pallet::<Runtime>::add_instance(
			RuntimeOrigin::root(),
			DomainAddress::EVM(evm_chain_id, source_address.0),
		)
		.unwrap();
	});

	let command_id = H256::from_low_u64_be(5678);

	#[allow(deprecated)]
	let test_input = Contract {
		constructor: None,
		functions: BTreeMap::<String, Vec<Function>>::from([(
			"execute".into(),
			vec![Function {
				name: "execute".into(),
				inputs: vec![
					Param {
						name: "commandId".into(),
						kind: ParamType::FixedBytes(32),
						internal_type: None,
					},
					Param {
						name: "sourceChain".into(),
						kind: ParamType::String,
						internal_type: None,
					},
					Param {
						name: "sourceAddress".into(),
						kind: ParamType::String,
						internal_type: None,
					},
					Param {
						name: "payload".into(),
						kind: ParamType::Bytes,
						internal_type: None,
					},
				],
				outputs: vec![],
				constant: false,
				state_mutability: Default::default(),
			}],
		)]),
		events: Default::default(),
		errors: Default::default(),
		receive: false,
		fallback: false,
	}
	.function("execute".into())
	.map_err(|_| "cannot retrieve test contract function")
	.unwrap()
	.encode_input(&[
		Token::FixedBytes(command_id.0.to_vec()),
		Token::String(evm_chain_name),
		Token::String(String::from_utf8(source_address.as_fixed_bytes().to_vec()).unwrap()),
		Token::Bytes(msg.serialize()),
	])
	.map_err(|_| "cannot encode input for test contract function")
	.unwrap();

	env.with_mut_state(Chain::Para(PARA_ID), || {
		assert_ok!(pallet_evm::Pallet::<Runtime>::call(
			RawOrigin::Signed(derived_sender_account.clone()).into(),
			sender_address,
			LP_AXELAR_GATEWAY.into(),
			test_input.to_vec(),
			U256::from(0),
			0x100000,
			U256::from(1_000_000_000),
			None,
			Some(U256::from(0)),
			Vec::new(),
		));
	})
	.unwrap();

	env.with_state(Chain::Para(PARA_ID), || {
		let derived_receiver_balance =
			orml_tokens::Pallet::<Runtime>::free_balance(currency_id, &derived_receiver_account);

		assert_eq!(derived_receiver_balance, transfer_amount)
	})
	.unwrap();
}
