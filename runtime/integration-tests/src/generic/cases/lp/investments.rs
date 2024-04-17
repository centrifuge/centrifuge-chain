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
use ethabi::Token;
use sp_core::U256;

use crate::{
	generic::{
		cases::lp::{names::POOL_A_T_1_USDC, setup_full, DECIMALS_6},
		config::Runtime,
		env::{EnvEvmExtension, EvmEnv},
	},
	utils::accounts::Keyring,
};
const DEFAULT_INVESTMENT_AMOUNT: Balance = 100 * DECIMALS_6;

#[test]
fn _test() {
	invest_collect::<centrifuge_runtime::Runtime>()
}

fn invest_collect<T: Runtime>() {
	let mut env = setup_full::<T>();

	env.state_mut(|evm| {
		evm.call(
			Keyring::TrancheInvestor(1),
			U256::zero(),
			POOL_A_T_1_USDC,
			"requestDeposit",
			Some(&[
				Token::Uint(DEFAULT_INVESTMENT_AMOUNT.into()),
				Token::Address(Keyring::TrancheInvestor(1).into()),
				Token::Address(Keyring::TrancheInvestor(1).into()),
				Token::Bytes(Default::default()),
			]),
		)
		.unwrap();
	});
}
