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

use cfg_primitives::Balance;
use cfg_types::{domain_address::DomainAddress, permissions::PoolRole, tokens::CurrencyId};
use ethabi::{ethereum_types::U256, Token};
use frame_support::traits::OriginTrait;
use frame_system::pallet_prelude::OriginFor;

use crate::{
	generic::{
		cases::lp::{
			names, utils,
			utils::{pool_a_tranche_1_id, Decoder},
			LocalUSDC, DECIMALS_6, DEFAULT_BALANCE, EVM_DOMAIN_CHAIN_ID, POOL_A, USDC,
		},
		config::Runtime,
		env::{Blocks, Env, EnvEvmExtension, EvmEnv},
		utils::{currency::CurrencyInfo, give_tokens, invest_and_collect},
	},
	utils::accounts::Keyring,
};

#[test_runtimes(all)]
fn transfer_tokens_from_local<T: Runtime>() {
	const AMOUNT: Balance = DEFAULT_BALANCE * DECIMALS_6;

	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
	});

	env.state(|evm| {
		let balance = Decoder::<Balance>::decode(
			&evm.view(
				Keyring::Alice,
				"usdc",
				"balanceOf",
				Some(&[Token::Address(Keyring::Ferdie.into())]),
			)
			.unwrap()
			.value,
		);
		assert_eq!(balance, 0);
	});

	// Add funds
	env.state_mut(|evm| {
		give_tokens::<T>(Keyring::Ferdie.id(), USDC.id(), AMOUNT);
		assert_eq!(
			orml_tokens::Accounts::<T>::get(Keyring::Ferdie.id(), USDC.id()).free,
			AMOUNT
		);

		// Transferring from Centrifuge Chain requires EVM escrow to be sufficiently
		// funded
		evm.call(
			Keyring::Admin,
			Default::default(),
			"usdc",
			"mint",
			Some(&[
				Token::Address(evm.deployed("escrow").address),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
	});

	env.state_mut(|_evm| {
		let call = pallet_liquidity_pools::Pallet::<T>::transfer(
			OriginFor::<T>::signed(Keyring::Ferdie.into()),
			USDC.id(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, Keyring::Ferdie.into()),
			AMOUNT,
		);
		call.unwrap();
		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
	});

	env.state(|evm| {
		let balance = Decoder::<Balance>::decode(
			&evm.view(
				Keyring::Alice,
				"usdc",
				"balanceOf",
				Some(&[Token::Address(Keyring::Ferdie.into())]),
			)
			.unwrap()
			.value,
		);
		assert_eq!(balance, AMOUNT);
	});
}

#[test_runtimes(all)]
fn transfer_tranche_tokens_from_local<T: Runtime>() {
	const AMOUNT: Balance = DEFAULT_BALANCE * DECIMALS_6;

	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_investment_currencies(evm);
		super::setup_deploy_lps(evm);
		super::setup_investors(evm);
	});

	env.state_mut(|evm| {
		assert_eq!(
			Decoder::<Balance>::decode(
				&evm.view(
					Keyring::Alice,
					names::POOL_A_T_1,
					"balanceOf",
					Some(&[Token::Address(Keyring::TrancheInvestor(1).into())]),
				)
				.unwrap()
				.value,
			),
			0
		);
	});

	// Invest, close epoch and collect tranche tokens with 1-to-1 conversion
	env.pass(Blocks::ByNumber(2));
	env.state_mut(|_evm| {
		crate::generic::utils::pool::give_role::<T>(
			Keyring::TrancheInvestor(1).into(),
			POOL_A,
			PoolRole::TrancheInvestor(pool_a_tranche_1_id::<T>(), cfg_primitives::SECONDS_PER_YEAR),
		);
		give_tokens::<T>(Keyring::TrancheInvestor(1).id(), LocalUSDC.id(), AMOUNT);
		invest_and_collect::<T>(
			Keyring::TrancheInvestor(1).into(),
			Keyring::Admin,
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			AMOUNT,
		);
		assert_eq!(
			orml_tokens::Accounts::<T>::get(
				Keyring::TrancheInvestor(1).id(),
				CurrencyId::Tranche(POOL_A, pool_a_tranche_1_id::<T>()),
			)
			.free,
			AMOUNT
		);
	});

	env.state_mut(|_evm| {
		pallet_liquidity_pools::Pallet::<T>::transfer_tranche_tokens(
			OriginFor::<T>::signed(Keyring::TrancheInvestor(1).into()),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, Keyring::TrancheInvestor(1).into()),
			AMOUNT,
		)
		.unwrap();
		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
	});

	env.state(|evm| {
		assert_eq!(
			Decoder::<Balance>::decode(
				&evm.view(
					Keyring::Alice,
					names::POOL_A_T_1,
					"balanceOf",
					Some(&[Token::Address(Keyring::TrancheInvestor(1).into())]),
				)
				.unwrap()
				.value,
			),
			AMOUNT
		);
	});
}
