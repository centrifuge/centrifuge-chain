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

use cfg_primitives::{Balance, PoolId};
use cfg_traits::TimeAsSecs;
use cfg_types::{domain_address::Domain, tokens::CurrencyId};
use ethabi::{ethereum_types::H160, Token, Uint};
use frame_support::{assert_ok, traits::OriginTrait};
use frame_system::pallet_prelude::OriginFor;
use pallet_liquidity_pools::GeneralCurrencyIndexOf;

use crate::{
	generic::{
		cases::lp::{process_outbound, utils, utils::Decoder, EVM_DOMAIN_CHAIN_ID, USDC},
		config::Runtime,
		env::{Env, EvmEnv},
	},
	utils::accounts::Keyring,
};

#[test]
fn _test() {
	add_pool::<centrifuge_runtime::Runtime>()
}

fn add_currency<T: Runtime>() {
	let mut env = super::setup::<T>(|_| {});

	env.deploy(
		"ERC20",
		"test_erc20",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(12))]),
	);

	let test_erc20_address = env.deployed("test_erc20").address();
	let test_foreign = CurrencyId::ForeignAsset(200_001);

	env.parachain_state_mut(|| {
		super::register_asset::<T>(
			"Test Coin",
			"TEST",
			18,
			10_000,
			test_erc20_address,
			test_foreign,
		);

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			test_foreign
		));

		process_outbound::<T>()
	});

	let index = GeneralCurrencyIndexOf::<T>::try_from(test_foreign).unwrap();

	// Verify the  test currencies are correctly added to the pool manager
	assert_eq!(
		Decoder::<H160>::decode(
			&env.view(
				Keyring::Alice,
				"pool_manager",
				"currencyIdToAddress",
				Some(&[Token::Uint(Uint::from(index.index))])
			)
			.unwrap()
			.value
		),
		test_erc20_address
	);

	assert_eq!(
		Decoder::<Balance>::decode(
			&env.view(
				Keyring::Alice,
				"pool_manager",
				"currencyAddressToId",
				Some(&[Token::Address(test_erc20_address)]),
			)
			.unwrap()
			.value
		),
		index.index
	);
}

fn add_pool<T: Runtime>() {
	let mut env = super::setup::<T>(super::setup_currencies);
	const POOL: PoolId = 1;

	env.parachain_state_mut(|| {
		crate::generic::utils::pool::create_one_tranched::<T>(Keyring::Admin.into(), POOL, USDC);

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		process_outbound::<T>()
	});

	let creation_time = env.parachain_state(<pallet_timestamp::Pallet<T> as TimeAsSecs>::now);

	// Compare the pool.created_at field that is returned
	assert_eq!(
		Decoder::<Uint>::decode(
			&env.view(
				Keyring::Alice,
				"pool_manager",
				"pools",
				Some(&[Token::Uint(Uint::from(POOL))]),
			)
			.unwrap()
			.value
		),
		Uint::from(creation_time)
	);

	env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		process_outbound::<T>()
	});

	// Adding a pool again DOES NOT change creation time - i.e. not override storage
	assert_eq!(
		Decoder::<Uint>::decode(
			&env.view(
				Keyring::Alice,
				"pool_manager",
				"pools",
				Some(&[Token::Uint(Uint::from(POOL))]),
			)
			.unwrap()
			.value
		),
		Uint::from(creation_time)
	);
}

fn add_tranche<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
	});
}

fn allow_investment_currency<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
		super::setup_tranches(env);
	});
}

fn disallow_investment_currency<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
		super::setup_tranches(env);
		super::setup_investment_currencies(env);
		super::setup_deploy_lps(env);
	});
}

fn update_member<T: Runtime>() {
	todo!()
}

fn update_tranche_token_metadata<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
		super::setup_tranches(env);
	});
}

fn update_tranche_token_price<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
		super::setup_tranches(env);
	});
}

crate::test_for_runtimes!(all, add_currency);
crate::test_for_runtimes!(all, add_pool);
crate::test_for_runtimes!(all, add_tranche);
crate::test_for_runtimes!(all, allow_investment_currency);
crate::test_for_runtimes!(all, disallow_investment_currency);
crate::test_for_runtimes!(all, update_member);
crate::test_for_runtimes!(all, update_tranche_token_metadata);
crate::test_for_runtimes!(all, update_tranche_token_price);
