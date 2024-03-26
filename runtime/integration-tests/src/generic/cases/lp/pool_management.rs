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
	domain_address::{Domain, DomainAddress},
	permissions::PoolRole,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use ethabi::{ethereum_types::H160, Token, Uint};
use frame_support::{assert_noop, assert_ok, traits::OriginTrait};
use frame_system::pallet_prelude::OriginFor;
use pallet_liquidity_pools::GeneralCurrencyIndexOf;
use pallet_pool_system::Config;
use runtime_common::account_conversion::AccountConverter;
use sp_runtime::traits::Hash;

use crate::{
	generic::{
		cases::lp::{
			names, utils,
			utils::{pool_a_tranche_id, Decoder},
			LocalUSDC, DAI, EVM_DOMAIN_CHAIN_ID, FRAX, POOL_A, POOL_B, USDC,
		},
		config::Runtime,
		env::{EnvEvmExtension, EvmEnv},
		utils::currency::{register_currency, CurrencyInfo},
	},
	utils::{accounts::Keyring, time::secs::SECONDS_PER_YEAR},
};

#[test]
fn _test() {
	update_member::<development_runtime::Runtime>()
}

fn add_currency<T: Runtime>() {
	let mut env = super::setup::<T, _>(|_| {});

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

		fn id(&self) -> CurrencyId {
			CurrencyId::ForeignAsset(200_001)
		}
	}

	env.state_mut(|evm| {
		evm.deploy(
			"ERC20",
			"test_erc20",
			Keyring::Admin,
			Some(&[Token::Uint(Uint::from(TestCurrency.decimals()))]),
		);

		register_currency::<T>(TestCurrency, |meta| {
			meta.location = Some(utils::lp_asset_location::<T>(
				evm.deployed("test_erc20").address(),
			));
		});

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			TestCurrency.id()
		));

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>)
	});

	let index = GeneralCurrencyIndexOf::<T>::try_from(TestCurrency.id()).unwrap();

	env.state_mut(|evm| {
		// Verify the  test currencies are correctly added to the pool manager
		assert_eq!(
			Decoder::<H160>::decode(
				&evm.view(
					Keyring::Alice,
					"pool_manager",
					"currencyIdToAddress",
					Some(&[Token::Uint(Uint::from(index.index))])
				)
				.unwrap()
				.value
			),
			evm.deployed("test_erc20").address()
		);

		assert_eq!(
			Decoder::<Balance>::decode(
				&evm.view(
					Keyring::Alice,
					"pool_manager",
					"currencyAddressToId",
					Some(&[Token::Address(evm.deployed("test_erc20").address())]),
				)
				.unwrap()
				.value
			),
			index.index
		);
	});

	env.state_mut(|evm| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			TestCurrency.id()
		));

		utils::process_outbound::<T>(|| {
			utils::verify_outbound_failure_on_lp::<T>(evm.deployed("router").address())
		});
	});
}

fn add_pool<T: Runtime>() {
	let mut env = super::setup::<T, _>(|_| {});
	const POOL: PoolId = 1;

	env.state_mut(|evm| {
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

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

		let creation_time = <pallet_timestamp::Pallet<T> as TimeAsSecs>::now();

		// Compare the pool.created_at field that is returned
		let evm_pool_time = Decoder::<Uint>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"pools",
				Some(&[Token::Uint(Uint::from(POOL))]),
			)
			.unwrap()
			.value,
		);
		assert_eq!(evm_pool_time, Uint::from(creation_time));
	});

	env.state_mut(|evm| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
			T::RuntimeOriginExt::signed(Keyring::Admin.into()),
			POOL,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>(|| {
			utils::verify_outbound_failure_on_lp::<T>(evm.deployed("router").address())
		});
	});
}

fn add_tranche<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
	});

	env.state_mut(|evm| {
		let tranche_id = utils::pool_a_tranche_id::<T>();
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_A,
			tranche_id,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

		// TODO(william): Actually check tranche was added on EVM side

		// TODO: Check EVM side and deploy tranche there
	});
}

fn allow_investment_currency<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
	});

	env.state_mut(|_evm| {
		assert_ok!(
			pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
				OriginFor::<T>::signed(Keyring::Admin.into()),
				POOL_A,
				USDC.id(),
			),
		);
		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

		// TODO(william): Check allowed investment currencies on EVM side and
		// deploy lp there
	})
}

fn disallow_investment_currency<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_investment_currencies(evm);
		super::setup_deploy_lps(evm);
	});

	// disallow investment currencies
	for currency in [DAI.id(), FRAX.id(), USDC.id()] {
		for pool in [POOL_A, POOL_B] {
			env.state_mut(|_evm| {
				assert_ok!(
					pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
						OriginFor::<T>::signed(Keyring::Admin.into()),
						pool,
						currency
					),
				);
				utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

				// TODO(william): Actually check whether LP is blocked
			})
		}
	}
}

fn update_member<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_investment_currencies(evm);
		super::setup_deploy_lps(evm);
	});

	env.state(|evm| {
		assert!(!Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::RM_POOL_A_T_1,
				"hasMember",
				Some(&[Token::Address(Keyring::Bob.into())]),
			)
			.unwrap()
			.value
		));
	});

	env.state_mut(|evm| {
		crate::generic::utils::pool::give_role::<T>(
			AccountConverter::<T, ()>::convert_evm_address(
				EVM_DOMAIN_CHAIN_ID,
				Keyring::Bob.into(),
			),
			POOL_A,
			PoolRole::TrancheInvestor(pool_a_tranche_id::<T>(), SECONDS_PER_YEAR),
		);

		// Address given MUST match derived allowlisted address for that domain
		assert_noop!(
			pallet_liquidity_pools::Pallet::<T>::update_member(
				Keyring::Bob.as_origin(),
				POOL_A,
				pool_a_tranche_id::<T>(),
				DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, Keyring::Alice.into()),
				SECONDS_PER_YEAR,
			),
			pallet_liquidity_pools::Error::<T>::InvestorDomainAddressNotAMember
		);

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			Keyring::Bob.as_origin(),
			POOL_A,
			pool_a_tranche_id::<T>(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, Keyring::Bob.into()),
			SECONDS_PER_YEAR,
		));

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
	});

	env.state(|evm| {
		assert!(Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::RM_POOL_A_T_1,
				"hasMember",
				Some(&[Token::Address(Keyring::Bob.into())]),
			)
			.unwrap()
			.value
		));

		assert!(!Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::RM_POOL_A_T_1,
				"hasMember",
				Some(&[Token::Address(Keyring::Alice.into())]),
			)
			.unwrap()
			.value
		));
	});
}

fn update_tranche_token_metadata<T: Runtime>() {
	let _env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
	});

	todo!("update_tranche_token_metadata")
}

fn update_tranche_token_price<T: Runtime>() {
	let _env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
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
