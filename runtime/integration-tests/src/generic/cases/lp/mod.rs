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

use cfg_primitives::{Balance, CFG, SECONDS_PER_HOUR};
use ethabi::{ethereum_types::U256, Token};

use crate::{
	generic::{
		config::Runtime,
		env::{Env, EvmEnv},
		envs::runtime_env::RuntimeEnv,
		utils::{genesis, genesis::Genesis},
	},
	utils::accounts::Keyring,
};

mod utils {}

pub mod deploy_pool;

const DEFAULT_BALANCE: Balance = 100 * CFG;

pub fn setup<T: Runtime>() -> impl EvmEnv<T> {
	let mut env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(DEFAULT_BALANCE))
			.storage(),
	)
	.load_contracts();

	// The flow is based in the following code from the Solidity and needs to be
	// adapted if this deployment script changes in the future
	// * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Axelar.s.sol#L17-L31
	//
	// PART: Deploy InvestmentManager
	//   * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L45-L69
	env.deploy(
		"Escrow",
		"escrow",
		Keyring::Alice,
		Some(&[Token::Address(Keyring::Alice.into())]),
	);
	env.deploy("UserEscrow", "user_escrow", Keyring::Alice, None);
	env.deploy(
		"Root",
		"root",
		Keyring::Alice,
		Some(&[
			Token::Address(env.deployed("escrow").address()),
			Token::Uint(U256::from(48 * SECONDS_PER_HOUR)),
			Token::Address(Keyring::Alice.into()),
		]),
	);
	env.deploy(
		"LiquidityPoolFactory",
		"lp_pool_factory",
		Keyring::Alice,
		Some(&[Token::Address(env.deployed("root").address())]),
	);
	env.deploy(
		"RestrictionManagerFactory",
		"restriction_manager_factory",
		Keyring::Alice,
		Some(&[Token::Address(env.deployed("root").address())]),
	);
	env.deploy(
		"TrancheTokenFactory",
		"tranche_token_factory",
		Keyring::Alice,
		Some(&[
			Token::Address(env.deployed("root").address()),
			Token::Address(Keyring::Alice.into()),
		]),
	);
	env.deploy(
		"InvestmentManager",
		"investment_manager",
		Keyring::Alice,
		Some(&[
			Token::Address(env.deployed("escrow").address()),
			Token::Address(env.deployed("user_escrow").address()),
		]),
	);
	env.deploy(
		"PoolManager",
		"pool_manager",
		Keyring::Alice,
		Some(&[
			Token::Address(env.deployed("escrow").address()),
			Token::Address(env.deployed("lp_pool_factory").address()),
			Token::Address(env.deployed("restriction_manager_factory").address()),
			Token::Address(env.deployed("tranche_token_factory").address()),
		]),
	);
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"lp_pool_factory",
		"rely",
		Some(&[Token::Address(env.deployed("pool_manager").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"tranche_token_factory",
		"rely",
		Some(&[Token::Address(env.deployed("pool_manager").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"restriction_manager_factory",
		"rely",
		Some(&[Token::Address(env.deployed("pool_manager").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"lp_pool_factory",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"tranche_token_factory",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"restriction_manager_factory",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();

	// PART: Deploy router (using the testing LocalRouter here)
	//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Axelar.s.sol#L24
	env.deploy("LocalRouter", "router", Keyring::Alice, None);

	// PART: Wire router + file gateway
	//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L71-L98
	/*
	   pauseAdmin = new PauseAdmin(address(root));
	   delayedAdmin = new DelayedAdmin(address(root), address(pauseAdmin));
	   gateway = new Gateway(address(root), address(investmentManager), address(poolManager), address(router));

	   pauseAdmin.rely(address(delayedAdmin));
	   root.rely(address(pauseAdmin));
	   root.rely(address(delayedAdmin));
	   root.rely(address(gateway));

	   investmentManager.file("poolManager", address(poolManager));
	   poolManager.file("investmentManager", address(investmentManager));
	   investmentManager.file("gateway", address(gateway));
	   poolManager.file("gateway", address(gateway));
	   investmentManager.rely(address(root));
	   investmentManager.rely(address(poolManager));
	   poolManager.rely(address(root));
	   gateway.rely(address(root));
	   AuthLike(router).rely(address(root));
	   AuthLike(address(escrow)).rely(address(root));
	   AuthLike(address(escrow)).rely(address(investmentManager));
	   AuthLike(address(userEscrow)).rely(address(root));
	   AuthLike(address(userEscrow)).rely(address(investmentManager));
	   AuthLike(address(escrow)).rely(address(poolManager));
	*/

	// PART: Give admin access
	//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L100-L106
	/*
		delayedAdmin.rely(address(admin));

		for (uint256 i = 0; i < pausers.length; i++) {
			pauseAdmin.addPauser(pausers[i]);
		}
	*/

	// PART: Remove deployer access
	//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L108-L121
	/*
	   AuthLike(router).deny(deployer);
	   AuthLike(liquidityPoolFactory).deny(deployer);
	   AuthLike(trancheTokenFactory).deny(deployer);
	   AuthLike(restrictionManagerFactory).deny(deployer);
	   root.deny(deployer);
	   investmentManager.deny(deployer);
	   poolManager.deny(deployer);
	   escrow.deny(deployer);
	   userEscrow.deny(deployer);
	   gateway.deny(deployer);
	   pauseAdmin.deny(deployer);
	   delayedAdmin.deny(deployer);
	*/

	env
}
