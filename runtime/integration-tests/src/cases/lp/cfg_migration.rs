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

use cfg_primitives::{Balance, OrderId, CFG};
use ethabi::{Token, Uint};
use pallet_investments::OrderOf;
use sp_core::U256;
use sp_runtime::traits::Zero;

use crate::{
	cases::lp::{
		self, names, setup,
		utils::{pool_c_tranche_1_id, Decoder},
		DECIMALS_6, DEFAULT_BALANCE, POOL_C,
	},
	config::Runtime,
	env::{Blocks, Env, EnvEvmExtension, EvmEnv},
	utils::accounts::Keyring,
};

const AXELAR_FEE: Balance = 100 * CFG;
const USER: Keyring = Keyring::Alice;

mod utils {
	use cfg_primitives::Balance;
	use cfg_types::tokens::{CrossChainTransferability, CurrencyId, CustomMetadata};
	use ethabi::{ParamType::Uint, Token};
	use frame_support::assert_ok;

	use crate::{
		cases::lp::{contracts, names, utils},
		config::Runtime,
		env::{Env, EvmEnv},
		utils::{
			accounts::Keyring,
			currency::{register_currency, CurrencyInfo},
		},
	};

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
			CurrencyId::ForeignAsset(999_999)
		}

		fn ed(&self) -> Balance {
			100_000_000_000_000
		}

		fn decimals(&self) -> u32 {
			18
		}
	}

	pub fn setup_axelar_gateway<T: Runtime>(env: &mut Env<T>) {
		env.state_mut(|evm| {
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
				names::IOU_CFG,
				"rely",
				Some(&[Token::Address(evm.deployed(names::POOL_MANAGER).address())]),
			)
			.unwrap();
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

			// Store deployed adapter contract in storage
			assert_ok!(pallet_axelar_router::Pallet::<T>::set_axelar_gas_service(
				OriginFor::<T>::root(),
				evm.deployed(names::ADAPTER).address()
			));

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
		});
	}
}

#[test_runtimes([centrifuge, development])]
fn full_cfg_migration_flow<T: Runtime>() {
	let mut env = setup::<T>(utils::setup_axelar_gateway);

	// Execute migration
	env.state_mut(|_evm| {
		assert_ok!(Pallet::<T>::migrate(
			OriginFor::<T>::signed(USER.into()),
			AXELAR_FEE,
			USER.in_eth()
		));
	});

	// Verify balances after migration
	let lock_account = T::CfgLockAccount::get().into_account_truncating();
	env.state(|evm| {
		// User balance should be zero
		assert_eq!(T::Tokens::balance(T::NativeCfg::get(), &USER.id()), 0);

		// Lock account should have remaining funds after fee
		assert_eq!(
			orml_tokens::Pallet::<T>::balance(T::NativeCfg::get(), &lock_account),
			DEFAULT_BALANCE * CFG - AXELAR_FEE
		);

		// Verify IOU tokens on Ethereum side
		assert_eq!(
			Decoder::<U256>::decode(&evm.view(
				user,
				evm.deployed(names::NEW_CFG).address(),
				"balanceOf",
				Some(&[Token::Address(USER.in_eth())]),
			)),
			DEFAULT_BALANCE * CFG - AXELAR_FEE
		);
	});
}
