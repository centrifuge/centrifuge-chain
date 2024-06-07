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
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	tokens::CurrencyId,
};
use ethabi::{ethereum_types::U256, Token};
use frame_support::traits::OriginTrait;
use frame_system::pallet_prelude::OriginFor;
use pallet_liquidity_pools::Message;
use sp_core::ByteArray;
use sp_runtime::traits::Convert;

use crate::{
	generic::{
		cases::lp::{
			self, names,
			utils::{as_h160_32bytes, pool_a_tranche_1_id, Decoder},
			LocalUSDC, DECIMALS_6, DEFAULT_BALANCE, EVM_DOMAIN_CHAIN_ID, POOL_A, USDC,
		},
		config::Runtime,
		env::{EnvEvmExtension, EvmEnv},
		utils::{currency::CurrencyInfo, give_tokens, invest_and_collect},
	},
	utils::accounts::Keyring,
};

// The default amount of invested stable coins
const AMOUNT: Balance = DEFAULT_BALANCE * DECIMALS_6;

mod utils {
	use super::*;
	use crate::generic::env::Blocks;

	pub fn prepare_hold_tt_domain<T: Runtime>(env: &mut impl EnvEvmExtension<T>) {
		env.state_mut(|evm| {
			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::Alice,
					names::POOL_A_T_1,
					"balanceOf",
					Some(&[Token::Address(Keyring::TrancheInvestor(1).into())]),
				)),
				0
			);
		});

		// Invest, close epoch and collect tranche tokens with 1-to-1 conversion
		env.pass(Blocks::ByNumber(2));
		env.state_mut(|_evm| {
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
			lp::utils::process_outbound::<T>(lp::utils::verify_outbound_success::<T>);
		});

		env.state(|evm| {
			assert_eq!(
				Decoder::<Balance>::decode(&evm.view(
					Keyring::Alice,
					names::POOL_A_T_1,
					"balanceOf",
					Some(&[Token::Address(Keyring::TrancheInvestor(1).into())]),
				)),
				AMOUNT
			);
		});
	}

	pub fn prepare_hold_usdc_local<T: Runtime>(env: &mut impl EnvEvmExtension<T>) {
		env.state_mut(|evm| {
			evm.call(
				Keyring::Alice,
				Default::default(),
				names::USDC,
				"approve",
				Some(&[
					Token::Address(evm.deployed(names::POOL_MANAGER).address()),
					Token::Uint(U256::from(AMOUNT)),
				]),
			)
			.unwrap();
			evm.call(
				Keyring::Alice,
				Default::default(),
				names::POOL_MANAGER,
				"transfer",
				Some(&[
					Token::Address(evm.deployed(names::USDC).address()),
					Token::FixedBytes(Keyring::Ferdie.id().to_raw_vec()),
					Token::Uint(U256::from(AMOUNT)),
				]),
			)
			.unwrap();

			assert_eq!(
				orml_tokens::Accounts::<T>::get(Keyring::Ferdie.id(), USDC.id()).free,
				AMOUNT
			);
		});
	}
}

#[test_runtimes(all)]
fn transfer_tokens_from_local<T: Runtime>() {
	let mut env = super::setup_full::<T>();
	utils::prepare_hold_usdc_local::<T>(&mut env);

	env.state_mut(|_evm| {
		let call = pallet_liquidity_pools::Pallet::<T>::transfer(
			OriginFor::<T>::signed(Keyring::Ferdie.into()),
			USDC.id(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, Keyring::Ferdie.into()),
			AMOUNT,
		);
		call.unwrap();
		lp::utils::process_outbound::<T>(lp::utils::verify_outbound_success::<T>);
	});

	env.state(|evm| {
		assert_eq!(
			Decoder::<Balance>::decode(&evm.view(
				Keyring::Alice,
				"usdc",
				"balanceOf",
				Some(&[Token::Address(Keyring::Ferdie.into())]),
			)),
			AMOUNT
		);
	});
}

#[test_runtimes(all)]
fn transfer_tranche_tokens_from_local<T: Runtime>() {
	let mut env = super::setup_full::<T>();

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
	// env.pass(Blocks::ByNumber(2));
	env.state_mut(|_evm| {
		give_tokens::<T>(Keyring::TrancheInvestor(1).id(), LocalUSDC.id(), AMOUNT);
		invest_and_collect::<T>(
			Keyring::TrancheInvestor(1).into(),
			Keyring::Admin,
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			AMOUNT,
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
		lp::utils::process_outbound::<T>(lp::utils::verify_outbound_success::<T>);
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

#[test_runtimes(all)]
fn transfer_tranche_tokens_domain_to_local_to_domain<T: Runtime>() {
	let mut env = super::setup_full::<T>();
	utils::prepare_hold_tt_domain::<T>(&mut env);

	env.state_mut(|evm| {
		evm.call(
			Keyring::TrancheInvestor(1),
			Default::default(),
			names::POOL_A_T_1,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_MANAGER).address()),
				Token::Uint(U256::from(AMOUNT)),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::TrancheInvestor(1),
			sp_core::U256::zero(),
			names::POOL_MANAGER,
			"transferTrancheTokensToEVM",
			Some(&[
				Token::Uint(POOL_A.into()),
				Token::FixedBytes(pool_a_tranche_1_id::<T>().into()),
				Token::Uint(EVM_DOMAIN_CHAIN_ID.into()),
				Token::Address(Keyring::TrancheInvestor(2).into()),
				Token::Uint(AMOUNT.into()),
			]),
		)
		.unwrap();
	});

	env.state_mut(|_evm| {
		lp::utils::process_outbound::<T>(|msg| {
			assert_eq!(
				msg,
				Message::TransferTrancheTokens {
					pool_id: POOL_A,
					tranche_id: pool_a_tranche_1_id::<T>(),
					sender:
						<T as pallet_liquidity_pools::Config>::DomainAddressToAccountId::convert(
							DomainAddress::evm(
								EVM_DOMAIN_CHAIN_ID,
								Keyring::TrancheInvestor(2).into()
							)
						)
						.into(),
					domain: Domain::EVM(EVM_DOMAIN_CHAIN_ID),
					receiver: as_h160_32bytes(Keyring::TrancheInvestor(2)),
					amount: AMOUNT,
				}
			);
		});
	});

	env.state(|evm| {
		assert_eq!(
			Decoder::<Balance>::decode(
				&evm.view(
					Keyring::Alice,
					names::POOL_A_T_1,
					"balanceOf",
					Some(&[Token::Address(Keyring::TrancheInvestor(2).into())]),
				)
				.unwrap()
				.value,
			),
			AMOUNT
		);
	});
}

#[test_runtimes(all)]
fn transfer_tranche_tokens_domain_to_local<T: Runtime>() {
	let mut env = super::setup_full::<T>();
	utils::prepare_hold_tt_domain::<T>(&mut env);

	env.state_mut(|evm| {
		evm.call(
			Keyring::TrancheInvestor(1),
			Default::default(),
			names::POOL_A_T_1,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_MANAGER).address()),
				Token::Uint(U256::from(AMOUNT)),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::TrancheInvestor(1),
			sp_core::U256::zero(),
			names::POOL_MANAGER,
			"transferTrancheTokensToCentrifuge",
			Some(&[
				Token::Uint(POOL_A.into()),
				Token::FixedBytes(pool_a_tranche_1_id::<T>().into()),
				Token::FixedBytes(Keyring::TrancheInvestor(2).id().to_raw_vec()),
				Token::Uint(AMOUNT.into()),
			]),
		)
		.unwrap();
	});

	env.state(|_evm| {
		assert_eq!(
			orml_tokens::Accounts::<T>::get(
				Keyring::TrancheInvestor(2).id(),
				CurrencyId::Tranche(POOL_A, pool_a_tranche_1_id::<T>()),
			)
			.free,
			AMOUNT
		);
	});
}
