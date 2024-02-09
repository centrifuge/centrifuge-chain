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

use cfg_types::tokens::CurrencyId;
use ethabi::{ethereum_types::H160, Token, Uint};
use frame_support::{
	assert_ok,
	traits::{OriginTrait, PalletInfo},
};
use frame_system::pallet_prelude::OriginFor;
use pallet_liquidity_pools::GeneralCurrencyIndexOf;

use crate::{
	generic::{
		cases::lp::process_outbound,
		config::Runtime,
		env::{Env, EvmEnv},
	},
	utils::accounts::Keyring,
};

#[test]
fn _test() {
	add_currency::<centrifuge_runtime::Runtime>()
}

fn add_currency<T: Runtime>() {
	let mut env = super::setup::<T>();

	env.deploy(
		"ERC20",
		"test_erc20",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(12))]),
	);

	let test_erc20_address = env.deployed("test_erc20").address();
	let test_foreign = CurrencyId::ForeignAsset(200_001);

	env.parachain_state_mut(|| {
		super::register_asset::<T>(
			"Test Coin",
			"TEST",
			18,
			10_000,
			test_erc20_address,
			test_foreign,
		);

		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			test_foreign
		));

		process_outbound::<T>()
	});

	let index = GeneralCurrencyIndexOf::<T>::try_from(test_foreign).unwrap();

	// Verify the  test currencies are correctly added to the pool manager
	assert_eq!(
		H160::from(cfg_utils::vec_to_fixed_array(
			env.view(
				Keyring::Alice,
				"pool_manager",
				"currencyIdToAddress",
				Some(&[Token::Uint(Uint::from(index.index))])
			)
			.unwrap()
			.value
			.split_off(12)
		)),
		test_erc20_address
	);

	assert_eq!(
		u128::from_be_bytes(cfg_utils::vec_to_fixed_array(
			env.view(
				Keyring::Alice,
				"pool_manager",
				"currencyAddressToId",
				Some(&[Token::Address(test_erc20_address)]),
			)
			.unwrap()
			.value
			.split_off(16)
			.to_vec()
		)),
		index.index
	);
}

crate::test_for_runtimes!(all, add_currency);
