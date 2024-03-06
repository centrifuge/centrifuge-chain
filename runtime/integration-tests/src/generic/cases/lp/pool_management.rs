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
use cfg_types::{
	domain_address::Domain,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use ethabi::{ethereum_types::H160, Token, Uint};
use frame_support::{assert_ok, traits::OriginTrait};
use frame_system::pallet_prelude::OriginFor;
use pallet_liquidity_pools::GeneralCurrencyIndexOf;

use crate::{
	generic::{
		cases::lp::{
			utils, utils::Decoder, LocalUSDC, DAI, EVM_DOMAIN_CHAIN_ID, FRAX, POOL_A, POOL_B, USDC,
		},
		config::Runtime,
		env::{Blocks, Env, EvmEnv},
		utils::currency::{register_currency, CurrencyInfo},
	},
	utils::accounts::Keyring,
};

#[test]
fn _test() {
	allow_investment_currency::<development_runtime::Runtime>()
}

fn add_currency<T: Runtime>() {
	let mut env = super::setup::<T>(|_| {});

	#[allow(non_camel_case_types)]
	pub struct TestCurrency;
	impl CurrencyInfo for TestCurrency {
		fn custom(&self) -> CustomMetadata {
			CustomMetadata {
				pool_currency: true,
				transferability: CrossChainTransferability::LiquidityPools,
				permissioned: false,
				mintable: false,
				local_representation: None,
			}
		}

		fn decimals(&self) -> u32 {
			12
		}

		fn ed(&self) -> Balance {
			10_000_000_000
		}

		fn id(&self) -> CurrencyId {
			CurrencyId::ForeignAsset(200_001)
		}

		fn symbol(&self) -> &'static str {
			"FRAX"
		}
	}

	env.deploy(
		"ERC20",
		"test_erc20",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(TestCurrency.decimals()))]),
	);

	let test_erc20_address = env.deployed("test_erc20").address();

	env.parachain_state_mut(|| {
		register_currency::<T>(USDC, |meta| {
			meta.location = Some(utils::lp_asset_location::<T>(test_erc20_address));
		});

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			TestCurrency.id()
		));

		utils::process_outbound::<T>()
	});

	let index = GeneralCurrencyIndexOf::<T>::try_from(TestCurrency.id()).unwrap();

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
	let mut env = super::setup::<T>(|_| {});
	const POOL: PoolId = 1;

	env.parachain_state_mut(|| {
		crate::generic::utils::pool::create_one_tranched::<T>(
			Keyring::Admin.into(),
			POOL,
			LocalUSDC.id(),
		);

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>()
	});

	let creation_time = env.parachain_state(<pallet_timestamp::Pallet<T> as TimeAsSecs>::now);
	// FIXME(william): Parachain is t=24 (block 2) while EVM created at t=0
	let offset = 24;
	let creation_time_with_offset = creation_time - offset;

	// Compare the pool.created_at field that is returned
	let evm_pool_time = Decoder::<Uint>::decode(
		&env.view(
			Keyring::Alice,
			"pool_manager",
			"pools",
			Some(&[Token::Uint(Uint::from(POOL))]),
		)
		.unwrap()
		.value,
	);
	assert_eq!(evm_pool_time, Uint::from(creation_time_with_offset));

	env.pass(Blocks::ByNumber(1));

	env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>()
	});

	// TODO: Actually find the revert event here.
	//
	// NOTE: That is really relevant for the router too.
	//     We need to check `Pending` and see for errors
	//     unfortunately and then error in the router instead of returning success.

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
		Uint::from(creation_time_with_offset)
	);
}

fn add_tranche<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
	});

	env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_A,
			utils::pool_a_tranche_id::<T>(),
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>()
	});
}

fn allow_investment_currency<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
		super::setup_tranches(env);
	});

	env.parachain_state_mut(|| {
		assert_ok!(
			pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
				OriginFor::<T>::signed(Keyring::Admin.into()),
				POOL_A,
				USDC.id(),
			),
		);
		utils::process_outbound::<T>()
	})
}

fn disallow_investment_currency<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
		super::setup_tranches(env);
		super::setup_investment_currencies(env);
		super::setup_deploy_lps(env);
	});

	// disallow investment currencies
	for currency in [DAI.id(), FRAX.id(), USDC.id()] {
		for pool in [POOL_A, POOL_B] {
			env.parachain_state_mut(|| {
				assert_ok!(
					pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
						OriginFor::<T>::signed(Keyring::Admin.into()),
						pool,
						currency
					),
				);
				utils::process_outbound::<T>()
			})
		}
	}
}

fn update_member<T: Runtime>() {
	todo!("use setup full because of tranche id req")
}

fn update_tranche_token_metadata<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
		super::setup_tranches(env);
	});

	todo!("update_tranche_token_metadata")
}

fn update_tranche_token_price<T: Runtime>() {
	let mut env = super::setup::<T>(|env| {
		super::setup_currencies(env);
		super::setup_pools(env);
		super::setup_tranches(env);
	});

	todo!("update_tranche_token_price")
}

crate::test_for_runtimes!(all, add_currency);
crate::test_for_runtimes!(all, add_pool);
crate::test_for_runtimes!(all, add_tranche);
crate::test_for_runtimes!(all, allow_investment_currency);
crate::test_for_runtimes!(all, disallow_investment_currency);
crate::test_for_runtimes!(all, update_member);
crate::test_for_runtimes!(all, update_tranche_token_metadata);
crate::test_for_runtimes!(all, update_tranche_token_price);
