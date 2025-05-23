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

use cfg_primitives::{AccountId, Balance, BlockNumber, PoolId, SECONDS_PER_YEAR};
use cfg_traits::{PoolMetadata, TrancheTokenPrice};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	permissions::PoolRole,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use ethabi::{ethereum_types::H160, Token, Uint};
use frame_support::{assert_ok, traits::OriginTrait};
use frame_system::pallet_prelude::OriginFor;
use pallet_liquidity_pools::GeneralCurrencyIndexOf;
use sp_runtime::FixedPointNumber;

use crate::{
	cases::lp::{
		names,
		names::POOL_A_T_1,
		utils,
		utils::{pool_a_tranche_1_id, Decoder},
		LocalUSDC, DECIMALS_6, DEFAULT_BALANCE, EVM_DOMAIN, EVM_DOMAIN_CHAIN_ID,
		LOCAL_RESTRICTION_MANAGER_ADDRESS, POOL_A, USDC,
	},
	config::Runtime,
	env::{EnvEvmExtension, EvmEnv},
	utils::{
		accounts::Keyring,
		currency::{register_currency, CurrencyInfo},
		pool::{give_role, remove_role},
	},
};

#[test_runtimes([centrifuge, development])]
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

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>)
	});

	let index = GeneralCurrencyIndexOf::<T>::try_from(TestCurrency.id()).unwrap();

	env.state_mut(|evm| {
		// Verify the  test currencies are correctly added to the pool manager
		assert_eq!(
			Decoder::<H160>::decode(
				&evm.view(
					Keyring::Alice,
					names::POOL_MANAGER,
					"idToAsset",
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
					names::POOL_MANAGER,
					"assetToId",
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

		utils::process_gateway_message::<T>(|_| {
			utils::verify_outbound_failure_on_lp::<T>(evm.deployed(names::ADAPTER).address())
		});
	});
}

#[test_runtimes([centrifuge, development])]
fn add_pool<T: Runtime>() {
	let mut env = super::setup::<T, _>(|_| {});
	const POOL: PoolId = 1;

	env.state_mut(|evm| {
		crate::utils::pool::create_one_tranched::<T>(Keyring::Admin.into(), POOL, LocalUSDC.id());

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL,
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);

		assert!(Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::POOL_MANAGER,
				"isPoolActive",
				Some(&[Token::Uint(Uint::from(POOL))]),
			)
			.unwrap()
			.value
		));
	});

	env.state_mut(|evm| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
			T::RuntimeOriginExt::signed(Keyring::Admin.into()),
			POOL,
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_gateway_message::<T>(|_| {
			utils::verify_outbound_failure_on_lp::<T>(evm.deployed(names::ADAPTER).address())
		});
	});
}

#[test_runtimes([centrifuge, development])]
fn hook_address<T: Runtime>() {
	let env = super::setup::<T, _>(|_| {});
	env.state(|evm| {
		let solidity = evm.deployed(names::RESTRICTION_MANAGER).address();
		let rust = LOCAL_RESTRICTION_MANAGER_ADDRESS.into();
		assert_eq!(
            solidity, rust,
            "Hook address changed, please change our stored value (right) to the new address (left)"
        );
	})
}

#[test_runtimes([centrifuge, development])]
fn add_tranche<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
	});

	env.state(|evm| {
		assert_eq!(
			evm.call(
				Keyring::Alice,
				Default::default(),
				names::POOL_MANAGER,
				"deployTranche",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::FixedBytes(pool_a_tranche_1_id::<T>().to_vec()),
				]),
			),
			utils::REVERT_ERR
		);
	});

	env.state_mut(|_| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});

	env.state_mut(|evm| {
		// Tranche id does not exist before adding and deploying tranche
		assert_eq!(
			Decoder::<H160>::decode(
				&evm.view(
					Keyring::Alice,
					names::POOL_MANAGER,
					"getTranche",
					Some(&[
						Token::Uint(Uint::from(POOL_A)),
						Token::FixedBytes(pool_a_tranche_1_id::<T>().to_vec()),
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
			names::POOL_MANAGER,
			"deployTranche",
			Some(&[
				Token::Uint(Uint::from(POOL_A)),
				Token::FixedBytes(pool_a_tranche_1_id::<T>().to_vec()),
			]),
		));
		assert_ne!(
			Decoder::<H160>::decode(
				&evm.view(
					Keyring::Alice,
					names::POOL_MANAGER,
					"getTranche",
					Some(&[
						Token::Uint(Uint::from(POOL_A)),
						Token::FixedBytes(pool_a_tranche_1_id::<T>().to_vec()),
					]),
				)
				.unwrap()
				.value,
			),
			[0u8; 20].into()
		);
	});
}

#[test_runtimes([centrifuge, development])]
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
				names::POOL_MANAGER,
				"isAllowedAsset",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::Address(evm.deployed(names::USDC).address()),
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
		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});

	env.state(|evm| {
		assert!(Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::POOL_MANAGER,
				"isAllowedAsset",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::Address(evm.deployed(names::USDC).address()),
				]),
			)
			.unwrap()
			.value,
		));
	});
}

#[test_runtimes([centrifuge, development])]
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
				names::POOL_MANAGER,
				"isAllowedAsset",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::Address(evm.deployed(names::USDC).address()),
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
		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});

	env.state(|evm| {
		assert!(!Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::POOL_MANAGER,
				"isAllowedAsset",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::Address(evm.deployed(names::USDC).address()),
				]),
			)
			.unwrap()
			.value,
		));
	});
}

#[test_runtimes([centrifuge, development])]
fn update_member<T: Runtime>() {
	let validity_block: BlockNumber = 2;
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_investment_currencies(evm);
		super::setup_deploy_lps(evm);
	});

	env.state(|evm| {
		assert!(
			!Decoder::<(bool, u64)>::decode(
				&evm.view(
					Keyring::Alice,
					names::RESTRICTION_MANAGER,
					"isMember",
					Some(&[
						Token::Address(evm.deployed(names::POOL_A_T_1).address()),
						Token::Address(Keyring::Bob.in_eth())
					]),
				)
				.unwrap()
				.value
			)
			.0
		);
	});

	env.state_mut(|_| {
		give_role::<T>(
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, Keyring::Bob.in_eth()).account(),
			POOL_A,
			PoolRole::TrancheInvestor(pool_a_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			Keyring::Bob.as_origin(),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, Keyring::Bob.in_eth()),
			crate::utils::pool::get_default_tranche_investor_validity::<T>(validity_block),
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});

	env.state(|evm| {
		assert!(
			Decoder::<(bool, u64)>::decode(
				&evm.view(
					Keyring::Alice,
					names::RESTRICTION_MANAGER,
					"isMember",
					Some(&[
						Token::Address(evm.deployed(names::POOL_A_T_1).address()),
						Token::Address(Keyring::Bob.in_eth())
					]),
				)
				.unwrap()
				.value
			)
			.0
		);

		assert!(
			!Decoder::<(bool, u64)>::decode(
				&evm.view(
					Keyring::Alice,
					names::RESTRICTION_MANAGER,
					"isMember",
					Some(&[
						Token::Address(evm.deployed(names::POOL_A_T_1).address()),
						Token::Address(Keyring::Alice.in_eth())
					]),
				)
				.unwrap()
				.value
			)
			.0
		);
	});
}

#[test_runtimes([centrifuge, development])]
fn update_tranche_token_metadata<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_market_ratios::<T>();
	});

	let decimals_new = 42;
	let name_new = b"NEW_NAME".to_vec();
	let symbol_new = b"NEW_SYMBOL".to_vec();

	let (decimals_old, name_evm, symbol_evm) = env.state(|evm| {
		let meta = orml_asset_registry::module::Metadata::<T>::get(CurrencyId::Tranche(
			POOL_A,
			pool_a_tranche_1_id::<T>(),
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
				pool_a_tranche_1_id::<T>().into(),
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
				pool_a_tranche_1_id::<T>(),
				Domain::Evm(EVM_DOMAIN_CHAIN_ID)
			)
		);
		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
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

#[test_runtimes([centrifuge, development])]
fn update_tranche_token_price<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
	});

	let (price, last_updated) = env.state_mut(|_evm| {
		let price =
			<pallet_pool_system::Pallet<T> as TrancheTokenPrice<AccountId, CurrencyId>>::get_price(
				POOL_A,
				pool_a_tranche_1_id::<T>(),
			)
			.unwrap();

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_token_price(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			USDC.id(),
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));
		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);

		price
	});

	env.state(|evm| {
		let (price_evm, computed_at_evm) = Decoder::<(u128, u64)>::decode(
			&evm.view(
				Keyring::Alice,
				names::POOL_MANAGER,
				"getTranchePrice",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::FixedBytes(pool_a_tranche_1_id::<T>().to_vec()),
					Token::Address(evm.deployed(names::USDC).address()),
				]),
			)
			.unwrap()
			.value,
		);

		assert_eq!(last_updated, computed_at_evm);
		assert_eq!(price.into_inner(), price_evm);
	});
}

#[test_runtimes([centrifuge, development])]
fn freeze_member<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_investment_currencies(evm);
		super::setup_deploy_lps(evm);
		super::setup_investors(evm);
	});

	env.state(|evm| {
		assert!(!Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::RESTRICTION_MANAGER,
				"isFrozen",
				Some(&[
					Token::Address(evm.deployed(names::POOL_A_T_1).address()),
					Token::Address(Keyring::TrancheInvestor(2).in_eth())
				]),
			)
			.unwrap()
			.value
		));
	});

	env.state_mut(|_| {
		give_role::<T>(
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, Keyring::TrancheInvestor(2).in_eth()).account(),
			POOL_A,
			PoolRole::FrozenTrancheInvestor(pool_a_tranche_1_id::<T>()),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::freeze_investor(
			Keyring::Admin.as_origin(),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, Keyring::TrancheInvestor(2).in_eth()),
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});

	env.state(|evm| {
		assert!(Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::RESTRICTION_MANAGER,
				"isFrozen",
				Some(&[
					Token::Address(evm.deployed(names::POOL_A_T_1).address()),
					Token::Address(Keyring::TrancheInvestor(2).in_eth())
				]),
			)
			.unwrap()
			.value
		));
	});
}

#[test_runtimes([centrifuge, development])]
fn unfreeze_member<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_investment_currencies(evm);
		super::setup_deploy_lps(evm);
		super::setup_investors(evm);
	});

	env.state_mut(|_| {
		give_role::<T>(
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, Keyring::TrancheInvestor(2).in_eth()).account(),
			POOL_A,
			PoolRole::FrozenTrancheInvestor(pool_a_tranche_1_id::<T>()),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::freeze_investor(
			Keyring::Admin.as_origin(),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, Keyring::TrancheInvestor(2).in_eth()),
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});
	env.state(|evm| {
		assert!(Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::RESTRICTION_MANAGER,
				"isFrozen",
				Some(&[
					Token::Address(evm.deployed(names::POOL_A_T_1).address()),
					Token::Address(Keyring::TrancheInvestor(2).in_eth())
				]),
			)
			.unwrap()
			.value
		));
	});

	env.state_mut(|_| {
		remove_role::<T>(
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, Keyring::TrancheInvestor(2).in_eth()).account(),
			POOL_A,
			PoolRole::FrozenTrancheInvestor(pool_a_tranche_1_id::<T>()),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::unfreeze_investor(
			Keyring::Admin.as_origin(),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, Keyring::TrancheInvestor(2).in_eth()),
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});
	env.state(|evm| {
		assert!(!Decoder::<bool>::decode(
			&evm.view(
				Keyring::Alice,
				names::RESTRICTION_MANAGER,
				"isFrozen",
				Some(&[
					Token::Address(evm.deployed(names::POOL_A_T_1).address()),
					Token::Address(Keyring::TrancheInvestor(2).in_eth())
				]),
			)
			.unwrap()
			.value
		));
	});
}

#[test_runtimes([centrifuge, development])]
fn update_tranche_hook<T: Runtime>() {
	let new_hook: [u8; 20] = [1u8; 20];
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
		super::setup_pools(evm);
		super::setup_tranches(evm);
		super::setup_investment_currencies(evm);
		super::setup_deploy_lps(evm);
	});

	env.state(|evm| {
		let solidity = evm.deployed(names::RESTRICTION_MANAGER).address();
		let rust = LOCAL_RESTRICTION_MANAGER_ADDRESS.into();
		assert_eq!(
            solidity, rust,
            "Hook address changed, please change our stored value (right) to the new address (left)"
        );
		let hook_address = Decoder::<H160>::decode(
			&evm.view(Keyring::Alice, POOL_A_T_1, "hook", None)
				.unwrap()
				.value,
		);
		assert_eq!(hook_address, solidity);
	});

	env.state_mut(|_| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_tranche_hook(
			Keyring::Admin.as_origin(),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			EVM_DOMAIN,
			new_hook
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});

	env.state(|evm| {
		let hook_address = Decoder::<H160>::decode(
			&evm.view(Keyring::Alice, POOL_A_T_1, "hook", None)
				.unwrap()
				.value,
		);
		assert_eq!(hook_address, H160::from(new_hook));
	});
}

#[test]
fn tmp() {
	recover_assets::<development_runtime::Runtime>()
}

#[test_runtimes([development])]
fn recover_assets<T: Runtime>() {
	let mut env = super::setup::<T, _>(|evm| {
		super::setup_currencies(evm);
	});
	let investor = Keyring::Custom("WrongTransfer");
	let amount = DEFAULT_BALANCE * DECIMALS_6;

	// Transfer assets into wrong contract
	let (token, wrong_contract) = env.state_mut(|evm| {
		let wrong_contract = evm.deployed(names::POOL_MANAGER).address();
		let token = evm.deployed(names::USDC).address();

		// Need to mint here instead of executing `transferAssets` because this would
		// transfer to escrow instead of pool manager
		evm.call(
			Keyring::Admin,
			Default::default(),
			names::USDC,
			"mint",
			Some(&[
				Token::Address(wrong_contract.into()),
				Token::Uint(sp_core::U256::from(amount)),
			]),
		)
		.unwrap();

		assert_eq!(
			Decoder::<Balance>::decode(&evm.view(
				Keyring::Alice,
				names::USDC,
				"balanceOf",
				Some(&[Token::Address(wrong_contract.into())]),
			)),
			amount
		);
		assert_eq!(
			Decoder::<Balance>::decode(&evm.view(
				Keyring::Alice,
				names::USDC,
				"balanceOf",
				Some(&[Token::Address(investor.in_eth())]),
			)),
			0
		);

		(token, wrong_contract)
	});

	env.state_mut(|_| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::recover_assets(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()),
			utils::to_fixed_array(wrong_contract.as_bytes()),
			utils::to_fixed_array(token.as_bytes()),
			sp_core::U256::from(amount),
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});

	env.state(|evm| {
		assert_eq!(
			Decoder::<Balance>::decode(&evm.view(
				Keyring::Alice,
				names::USDC,
				"balanceOf",
				Some(&[Token::Address(wrong_contract)]),
			)),
			0
		);
		assert_eq!(
			Decoder::<Balance>::decode(&evm.view(
				Keyring::Alice,
				names::USDC,
				"balanceOf",
				Some(&[Token::Address(investor.in_eth())]),
			)),
			amount
		);
	});
}

#[test_runtimes([development])]
fn schedule_upgrade<T: Runtime>() {
	let mut env = super::setup_full::<T>();
	env.state_mut(|evm| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::schedule_upgrade(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			EVM_DOMAIN_CHAIN_ID,
			evm.deployed(names::POOL_MANAGER).address().into()
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});
}

#[test_runtimes([development])]
fn cancel_upgrade<T: Runtime>() {
	let mut env = super::setup_full::<T>();
	env.state_mut(|evm| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::schedule_upgrade(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			EVM_DOMAIN_CHAIN_ID,
			evm.deployed(names::POOL_MANAGER).address().into()
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});
	env.state_mut(|evm| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::cancel_upgrade(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			EVM_DOMAIN_CHAIN_ID,
			evm.deployed(names::POOL_MANAGER).address().into()
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	});
}
