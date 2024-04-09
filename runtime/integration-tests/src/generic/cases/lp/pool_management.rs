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
use cfg_traits::{PoolMetadata, TimeAsSecs, TrancheTokenPrice};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	permissions::PoolRole,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use ethabi::{
	ethereum_types::{H160, U256},
	Token, Uint,
};
use frame_support::{assert_noop, assert_ok, traits::OriginTrait};
use frame_system::pallet_prelude::OriginFor;
use pallet_liquidity_pools::GeneralCurrencyIndexOf;
use runtime_common::account_conversion::AccountConverter;
use sp_runtime::{DispatchError, FixedPointNumber};

use crate::{
	generic::{
		cases::lp::{
			names,
			names::POOL_A_T_1_USDC,
			utils,
			utils::{pool_a_tranche_id, Decoder},
			LocalUSDC, DECIMALS_6, DEFAULT_BALANCE, EVM_DOMAIN_CHAIN_ID, POOL_A, USDC,
		},
		config::Runtime,
		env::{Blocks, Env, EnvEvmExtension, EvmEnv},
		utils::{
			currency::{register_currency, CurrencyInfo},
			give_tokens, invest_and_collect,
		},
	},
	utils::{accounts::Keyring, time::secs::SECONDS_PER_YEAR},
};

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

	env.state(|evm| {
		assert_noop!(
			evm.call(
				Keyring::Alice,
				Default::default(),
				"pool_manager",
				"deployTranche",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::FixedBytes(pool_a_tranche_id::<T>().to_vec()),
				]),
			),
			DispatchError::Other("EVM call failed: Revert")
		);
	});

	env.state_mut(|_| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_A,
			pool_a_tranche_id::<T>(),
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
	});

	env.state_mut(|evm| {
		// Tranche id does not exist before adding and deploying tranche
		assert_eq!(
			Decoder::<sp_core::H160>::decode(
				&evm.view(
					Keyring::Alice,
					"pool_manager",
					"getTrancheToken",
					Some(&[
						Token::Uint(Uint::from(POOL_A)),
						Token::FixedBytes(pool_a_tranche_id::<T>().to_vec()),
					]),
				)
				.unwrap()
				.value,
			),
			[0u8; 20].into()
		);

		assert_ok!(evm.call(
			Keyring::Alice,
			Default::default(),
			"pool_manager",
			"deployTranche",
			Some(&[
				Token::Uint(Uint::from(POOL_A)),
				Token::FixedBytes(pool_a_tranche_id::<T>().to_vec()),
			]),
		));
		assert_ne!(
			Decoder::<sp_core::H160>::decode(
				&evm.view(
					Keyring::Alice,
					"pool_manager",
					"getTrancheToken",
					Some(&[
						Token::Uint(Uint::from(POOL_A)),
						Token::FixedBytes(pool_a_tranche_id::<T>().to_vec()),
					]),
				)
				.unwrap()
				.value,
			),
			[0u8; 20].into()
		);
	});
}

fn allow_investment_currency<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
	});

	env.state(|evm| {
		assert!(!Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"isAllowedAsInvestmentCurrency",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		));
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
	});

	env.state(|evm| {
		assert!(Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"isAllowedAsInvestmentCurrency",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		));
	});
}

fn disallow_investment_currency<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_investment_currencies(evm);
	});

	env.state(|evm| {
		assert!(Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"isAllowedAsInvestmentCurrency",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		));
	});

	env.state_mut(|_evm| {
		assert_ok!(
			pallet_liquidity_pools::Pallet::<T>::disallow_investment_currency(
				OriginFor::<T>::signed(Keyring::Admin.into()),
				POOL_A,
				USDC.id()
			),
		);
		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
	});

	env.state(|evm| {
		assert!(!Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"isAllowedAsInvestmentCurrency",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		));
	});
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

	env.state_mut(|_| {
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
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
	});

	let decimals_new = 42;
	let name_new = b"NEW_NAME".to_vec();
	let symbol_new = b"NEW_SYMBOL".to_vec();

	let (decimals_old, name_evm, symbol_evm) = env.state(|evm| {
		let meta = orml_asset_registry::Metadata::<T>::get(CurrencyId::Tranche(
			POOL_A,
			pool_a_tranche_id::<T>(),
		))
		.unwrap();
		assert!(meta.name.is_empty());
		assert!(meta.symbol.is_empty());

		let decimals = Decoder::<u8>::decode(
			&evm.view(Keyring::Alice, names::POOL_A_T_1, "decimals", Some(&[]))
				.unwrap()
				.value,
		);

		// name and decimals are of EVM type String
		let name = &evm
			.view(Keyring::Alice, names::POOL_A_T_1, "name", Some(&[]))
			.unwrap()
			.value;
		let symbol = &evm
			.view(Keyring::Alice, names::POOL_A_T_1, "symbol", Some(&[]))
			.unwrap()
			.value;
		assert_eq!(u32::from(decimals), meta.decimals);

		(meta.decimals, name.clone(), symbol.clone())
	});

	env.state_mut(|_evm| {
		assert_ok!(
			pallet_pool_registry::Pallet::<T>::update_tranche_token_metadata(
				POOL_A,
				pool_a_tranche_id::<T>().into(),
				Some(decimals_new.clone()),
				Some(name_new.clone()),
				Some(symbol_new.clone()),
				None,
				None,
				None
			),
		);

		assert_ok!(
			pallet_liquidity_pools::Pallet::<T>::update_tranche_token_metadata(
				OriginFor::<T>::signed(Keyring::Alice.into()),
				POOL_A,
				pool_a_tranche_id::<T>(),
				Domain::EVM(EVM_DOMAIN_CHAIN_ID)
			)
		);
		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
	});

	env.state(|evm| {
		// Decimals cannot be changed
		let decimals = u32::from(Decoder::<u8>::decode(
			&evm.view(Keyring::Alice, names::POOL_A_T_1, "decimals", Some(&[]))
				.unwrap()
				.value,
		));
		assert_ne!(decimals, decimals_new);
		assert_eq!(decimals, decimals_old);

		// name and decimals are of EVM type String
		let name = &evm
			.view(Keyring::Alice, names::POOL_A_T_1, "name", Some(&[]))
			.unwrap()
			.value;
		let symbol = &evm
			.view(Keyring::Alice, names::POOL_A_T_1, "symbol", Some(&[]))
			.unwrap()
			.value;

		assert_ne!(*name, name_evm);
		assert_ne!(*symbol, symbol_evm);

		// contained in slice [64..71]
		assert!(name.windows(name_new.len()).any(|w| w == name_new));
		assert!(symbol.windows(symbol_new.len()).any(|w| w == symbol_new));
	});
}

fn update_tranche_token_price<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
	});

	// Neither price nor computed exists yet
	env.state(|evm| {
		let (price_evm, computed_evm) = Decoder::<(u128, u64)>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getTrancheTokenPrice",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::FixedBytes(pool_a_tranche_id::<T>().to_vec()),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		);

		assert_eq!(price_evm, 0);
		assert_eq!(computed_evm, 0);
	});

	let pre_price_cfg = env.state_mut(|_evm| {
		let price = <pallet_pool_system::Pallet<T> as TrancheTokenPrice<
			<T as frame_system::Config>::AccountId,
			CurrencyId,
		>>::get(POOL_A, pool_a_tranche_id::<T>())
		.unwrap();

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_token_price(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			POOL_A,
			pool_a_tranche_id::<T>(),
			USDC.id(),
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));
		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

		price
	});

	env.state(|evm| {
		let (price_evm, computed_at_evm) = Decoder::<(u128, u64)>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getTrancheTokenPrice",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::FixedBytes(pool_a_tranche_id::<T>().to_vec()),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		);

		assert_eq!(pre_price_cfg.last_updated, computed_at_evm);
		assert_eq!(price_evm, pre_price_cfg.price.into_inner());
	});
}

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
			PoolRole::TrancheInvestor(pool_a_tranche_id::<T>(), cfg_primitives::SECONDS_PER_YEAR),
		);
		give_tokens::<T>(Keyring::TrancheInvestor(1).id(), LocalUSDC.id(), AMOUNT);
		invest_and_collect::<T>(
			Keyring::TrancheInvestor(1).into(),
			Keyring::Admin,
			POOL_A,
			pool_a_tranche_id::<T>(),
			AMOUNT,
		);
		assert_eq!(
			orml_tokens::Accounts::<T>::get(
				Keyring::TrancheInvestor(1).id(),
				CurrencyId::Tranche(POOL_A, pool_a_tranche_id::<T>()),
			)
			.free,
			AMOUNT
		);
	});

	env.state_mut(|_evm| {
		pallet_liquidity_pools::Pallet::<T>::transfer_tranche_tokens(
			OriginFor::<T>::signed(Keyring::TrancheInvestor(1).into()),
			POOL_A,
			pool_a_tranche_id::<T>(),
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

crate::test_for_runtimes!(all, add_currency);
crate::test_for_runtimes!(all, add_pool);
crate::test_for_runtimes!(all, add_tranche);
crate::test_for_runtimes!(all, allow_investment_currency);
crate::test_for_runtimes!(all, disallow_investment_currency);
crate::test_for_runtimes!(all, transfer_tokens_from_local);
crate::test_for_runtimes!(all, transfer_tranche_tokens_from_local);
crate::test_for_runtimes!(all, update_member);
crate::test_for_runtimes!(all, update_tranche_token_metadata);
crate::test_for_runtimes!(all, update_tranche_token_price);
