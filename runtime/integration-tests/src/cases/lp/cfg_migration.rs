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

use cfg_primitives::{Balance, OrderId};
use ethabi::{Token, Uint};
use pallet_investments::OrderOf;
use sp_core::U256;
use sp_runtime::traits::Zero;

use crate::{
	cases::lp::{
		self, names, setup,
		utils::{pool_c_tranche_1_id, Decoder},
		DECIMALS_6, POOL_C,
	},
	config::Runtime,
	env::{Blocks, Env, EnvEvmExtension, EvmEnv},
	utils::accounts::Keyring,
};

const AXELAR_FEE: Balance = 100 * CFG;
const USER: Keyring = Keyring::Alice;

mod utils {
	pub fn setup_axelar_gateway<T: Runtime>(env: &mut Env<T>) {
		let axelar_gateway = T::AxelarGateway::get();
		let axelar_id = T::AxelarId::get();
		let axelar_config = AxelarConfig {
			chain_id: T::EVMChainId::get(),
			router_id: RouterId::Evm(axelar_id),
			fee_values: FeeValues {
				base_fee: 0.into(),
				price_per_byte: 0.into(),
			},
		};
		env.state_mut(|evm| {
			evm.deploy_contract(
				axelar_gateway,
				"AxelarGateway",
				include_bytes!("../../../res/axelar/AxelarGateway.bin"),
				axelar_config.encode(),
			);
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
			fee,
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
			T::Tokens::balance(T::NativeCfg::get(), &lock_account),
			initial_balance - fee
		);

		// Axelar gateway should have received fee
		let axelar_gateway = T::AxelarGateway::get();
		assert_eq!(evm.balance(axelar_gateway), fee.into());

		// Verify IOU tokens on Ethereum side
		let iou_contract = evm.deployed("IouCfg");
		assert_eq!(
			Decoder::<U256>::decode(&evm.view(
				user,
				iou_contract.address(),
				"balanceOf",
				Some(&[Token::Address(USER.in_eth())]),
			)),
			initial_balance - fee
		);
	});
}
