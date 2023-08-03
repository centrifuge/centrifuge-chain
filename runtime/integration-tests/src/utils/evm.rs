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

use frame_support::{dispatch::RawOrigin, traits::fungible::Mutate};
use fudge::primitives::Chain;
use pallet_evm::FeeCalculator;
use runtime_common::account_conversion::AccountConverter;
use sp_core::{Get, H160, U256};

use crate::{
	chain::centrifuge::{Balances, Runtime, PARA_ID},
	utils::env::TestEnv,
};

pub fn mint_balance_into_derived_account(env: &mut TestEnv, address: H160, balance: u128) {
	let chain_id = env
		.with_state(Chain::Para(PARA_ID), || {
			pallet_evm_chain_id::Pallet::<Runtime>::get()
		})
		.unwrap();

	let derived_account =
		AccountConverter::<Runtime>::convert_evm_address(chain_id, address.to_fixed_bytes());

	env.with_mut_state(Chain::Para(PARA_ID), || {
		Balances::mint_into(&derived_account.into(), balance).unwrap()
	})
	.unwrap();
}

pub fn deploy_contract(env: &mut TestEnv, address: H160, code: Vec<u8>) {
	let chain_id = env
		.with_state(Chain::Para(PARA_ID), || {
			pallet_evm_chain_id::Pallet::<Runtime>::get()
		})
		.unwrap();

	let derived_address =
		AccountConverter::<Runtime>::convert_evm_address(chain_id, address.to_fixed_bytes());

	let transaction_create_cost = env
		.with_state(Chain::Para(PARA_ID), || {
			<Runtime as pallet_evm::Config>::config().gas_transaction_create
		})
		.unwrap();

	let base_fee = env
		.with_state(Chain::Para(PARA_ID), || {
			let (base_fee, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();
			base_fee
		})
		.unwrap();

	env.with_mut_state(Chain::Para(PARA_ID), || {
		pallet_evm::Pallet::<Runtime>::create(
			RawOrigin::from(Some(derived_address)).into(),
			address,
			code,
			U256::from(0),
			transaction_create_cost * 10,
			U256::from(base_fee + 10),
			None,
			None,
			Vec::new(),
		)
		.unwrap();
	})
	.unwrap();
}
