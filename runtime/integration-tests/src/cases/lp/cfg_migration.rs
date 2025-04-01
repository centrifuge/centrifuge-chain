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

use cfg_primitives::{Balance, CFG};
use ethabi::Token;
use frame_support::{
	assert_err, assert_ok,
	traits::{fungibles::Inspect, OriginTrait},
};
use frame_system::pallet_prelude::OriginFor;
use sp_core::Get;
use sp_runtime::traits::AccountIdConversion;

use crate::{
	cases::lp::{
		names, setup,
		utils::{process_gateway_message, verify_gateway_message_success, Decoder},
		DEFAULT_BALANCE,
	},
	config::Runtime,
	env::{EnvEvmExtension, EvmEnv},
	utils::accounts::Keyring,
};

const AXELAR_FEE: Balance = 100 * CFG;
const USER: Keyring = Keyring::Charlie;

mod utils {
	use cfg_primitives::{Balance, MICRO_CFG};
	use cfg_types::{
		domain_address::DomainAddress,
		tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	};
	use ethabi::{Token, Uint};
	use frame_support::{assert_ok, traits::OriginTrait};
	use frame_system::pallet_prelude::OriginFor;

	use crate::{
		cases::lp::{contracts, names, utils, utils::to_fixed_array},
		config::Runtime,
		env::EvmEnv,
		utils::{
			accounts::Keyring,
			currency::{register_currency, CurrencyInfo},
		},
	};

	pub fn to_domain_addr(who: Keyring) -> DomainAddress {
		use crate::cases::lp::EVM_DOMAIN_CHAIN_ID;

		DomainAddress::new(
			cfg_types::domain_address::Domain::Evm(EVM_DOMAIN_CHAIN_ID),
			to_fixed_array(who.in_eth().as_bytes()),
		)
	}

	#[allow(non_camel_case_types)]
	pub struct IOU_CFG;
	impl CurrencyInfo for IOU_CFG {
		fn custom(&self) -> CustomMetadata {
			CustomMetadata {
				pool_currency: false,
				transferability: CrossChainTransferability::LiquidityPools,
				permissioned: false,
				mintable: false,
				local_representation: None,
			}
		}

		fn symbol(&self) -> &'static str {
			"IOU_CFG"
		}

		fn id(&self) -> CurrencyId {
			cfg_types::tokens::usdc::CURRENCY_ID_IOU_CFG
		}

		fn ed(&self) -> Balance {
			1 * MICRO_CFG
		}

		fn decimals(&self) -> u32 {
			18
		}
	}

	pub fn setup_axelar_gateway<T: Runtime>(evm: &mut impl EvmEnv<T>) {
		evm.deploy(
			contracts::ERC_20,
			names::W_CFG,
			Keyring::Admin,
			Some(&[Token::Uint(Uint::from(18))]),
		);
		evm.deploy(
			contracts::ERC_20,
			names::NEW_CFG,
			Keyring::Admin,
			Some(&[Token::Uint(Uint::from(18))]),
		);
		evm.deploy(
			contracts::IOU_CFG,
			names::IOU_CFG,
			Keyring::Admin,
			Some(&[
				Token::Address(evm.deployed(names::POOL_MANAGER).address()),
				Token::Address(evm.deployed(names::ESCROW).address()),
				Token::Address(evm.deployed(names::NEW_CFG).address()),
				Token::Address(evm.deployed(names::W_CFG).address()),
			]),
		);
		evm.call(
			Keyring::Admin,
			Default::default(),
			names::W_CFG,
			"rely",
			Some(&[Token::Address(evm.deployed(names::IOU_CFG).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Admin,
			Default::default(),
			names::NEW_CFG,
			"rely",
			Some(&[Token::Address(evm.deployed(names::IOU_CFG).address())]),
		)
		.unwrap();

		// REGISTER IOU_CFG
		register_currency::<T>(IOU_CFG, |meta| {
			meta.location = Some(utils::lp_asset_location::<T>(
				evm.deployed(names::IOU_CFG).address(),
			));
		});

		// ADD ASSET
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			IOU_CFG.id()
		));
		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
	}
}

#[test]
fn _test() {
	full_cfg_migration_flow::<centrifuge_runtime::Runtime>();
}

#[test_runtimes([centrifuge])]
fn full_cfg_migration_flow<T: Runtime + pallet_cfg_migration::Config>() {
	let mut env = setup::<T, _>(utils::setup_axelar_gateway);

	//  Ensure errors are triggered
	env.state_mut(|_evm| {
		assert_err!(
			pallet_cfg_migration::Pallet::<T>::migrate(
				OriginFor::<T>::signed(USER.into()),
				utils::to_domain_addr(USER)
			),
			pallet_cfg_migration::Error::<T>::FeeAmountNotSet,
		);
		assert_ok!(pallet_cfg_migration::Pallet::<T>::set_fee_amount(
			OriginFor::<T>::root(),
			AXELAR_FEE
		));

		assert_err!(
			pallet_cfg_migration::Pallet::<T>::migrate(
				OriginFor::<T>::signed(USER.into()),
				utils::to_domain_addr(USER)
			),
			pallet_cfg_migration::Error::<T>::FeeReceiverNotSet,
		);
		assert_ok!(pallet_cfg_migration::Pallet::<T>::set_fee_receiver(
			OriginFor::<T>::root(),
			Keyring::Admin.into()
		));
	});

	// Execute migration
	env.state_mut(|_evm| {
		assert_ok!(pallet_cfg_migration::Pallet::<T>::migrate(
			OriginFor::<T>::signed(USER.into()),
			utils::to_domain_addr(USER)
		));
		process_gateway_message::<T>(verify_gateway_message_success::<T>);
	});

	// Verify balances after migration
	let lock_account = T::CfgLockAccount::get().into_account_truncating();
	env.state(|evm| {
		// User balance should be zero
		assert_eq!(
			<T as pallet_pool_system::Config>::Tokens::balance(T::NativeCfg::get(), &USER.id()),
			0
		);

		// Lock account should have remaining funds after fee
		assert_eq!(
			<T as pallet_pool_system::Config>::Tokens::balance(T::NativeCfg::get(), &lock_account),
			DEFAULT_BALANCE * CFG - AXELAR_FEE
		);

		// Verify IOU tokens on Ethereum side
		assert_eq!(
			Decoder::<Balance>::decode(&evm.view(
				USER,
				names::NEW_CFG,
				"balanceOf",
				Some(&[Token::Address(USER.in_eth())]),
			)),
			DEFAULT_BALANCE * CFG - AXELAR_FEE
		);
	});
}
