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

/// The faked router address on the EVM side. Needed for the precompile to
/// verify the origin of messages.
///
/// NOTE: This is NOT the real address of the
/// router, but the one we are faking on the EVM side.
pub const EVM_ROUTER: &str = "0x1111111111111111111111111111111111111111";

/// The faked domain name the LP messages are coming from and going to.
pub const EVM_DOMAIN: &str = "TestDomain";

pub fn setup<T: Runtime>() -> impl EvmEnv<T> {
	let mut env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(DEFAULT_BALANCE))
			.storage(),
	)
	.load_contracts();

	env.deploy("LocalRouterScript", "lp_deploy", Keyring::Alice, None);
	env.call_mut(Keyring::Alice, Default::default(), "lp_deploy", "run", None)
		.unwrap();

	/*
	// ------------------ EVM Side ----------------------- //
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
	env.deploy(
		"PauseAdmin",
		"pause_admin",
		Keyring::Alice,
		Some(&[Token::Address(env.deployed("root").address())]),
	);
	env.deploy(
		"DelayedAdmin",
		"delay_admin",
		Keyring::Alice,
		Some(&[
			Token::Address(env.deployed("root").address()),
			Token::Address(env.deployed("pause_admin").address()),
		]),
	);
	// Enable once https://github.com/foundry-rs/foundry/issues/7032 is resolved
	env.deploy(
		"Gateway",
		"gateway",
		Keyring::Alice,
		Some(&[
			Token::Address(env.deployed("root").address()),
			Token::Address(env.deployed("investment_manager").address()),
			Token::Address(env.deployed("pool_manager").address()),
			Token::Address(env.deployed("router").address()),
		]),
	);

	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"pause_admin",
		"rely",
		Some(&[Token::Address(env.deployed("delay_admin").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"root",
		"rely",
		Some(&[Token::Address(env.deployed("delay_admin").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"root",
		"rely",
		Some(&[Token::Address(env.deployed("pause_admin").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"root",
		"rely",
		Some(&[Token::Address(env.deployed("gateway").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"investment_manager",
		"file",
		Some(&[
			Token::FixedBytes("poolManager".as_bytes().to_vec()),
			Token::Address(env.deployed("pool_manager").address()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"file",
		Some(&[
			Token::FixedBytes("investmentManager".as_bytes().to_vec()),
			Token::Address(env.deployed("investment_manager").address()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"investment_manager",
		"file",
		Some(&[
			Token::FixedBytes("gateway".as_bytes().to_vec()),
			Token::Address(env.deployed("gateway").address()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"file",
		Some(&[
			Token::FixedBytes("gateway".as_bytes().to_vec()),
			Token::Address(env.deployed("gateway").address()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"investment_manager",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"investment_manager",
		"rely",
		Some(&[Token::Address(env.deployed("pool_manager").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"gateway",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();
	/* NOTE: This rely is NOT needed as the LocalRouter is not permissioned
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"router",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();
	 */
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"escrow",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"escrow",
		"rely",
		Some(&[Token::Address(env.deployed("investment_manager").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"user_escrow",
		"rely",
		Some(&[Token::Address(env.deployed("root").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"user_escrow",
		"rely",
		Some(&[Token::Address(env.deployed("investment_manager").address())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"escrow",
		"rely",
		Some(&[Token::Address(env.deployed("pool_manager").address())]),
	)
	.unwrap();

	// PART: File LocalRouter
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"router",
		"file",
		Some(&[
			Token::FixedBytes("gateway".as_bytes().to_vec()),
			Token::Address(env.deployed("gateway").address()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"router",
		"file",
		Some(&[
			Token::FixedBytes("sourceChain".as_bytes().to_vec()),
			Token::String(EVM_DOMAIN.to_string()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"router",
		"file",
		Some(&[
			Token::FixedBytes("sourceAddress".as_bytes().to_vec()),
			Token::String(EVM_ROUTER.to_string()),
		]),
	)
	.unwrap();

	// PART: Give admin access - Keyring::Admin in our case
	//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L100-L106
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"delay_admin",
		"rely",
		Some(&[Token::Address(Keyring::Admin.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"pause_admin",
		"addPauser",
		Some(&[Token::Address(Keyring::Admin.into())]),
	)
	.unwrap();

	// PART: Remove deployer access
	//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L108-L121
	/* NOTE: This rely is NOT needed as the LocalRouter is not permissioned
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"router",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	*/
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"lp_pool_factory",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"tranche_token_factory",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"restriction_manager_factory",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"root",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"investment_manager",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"escrow",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"user_escrow",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"gateway",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"pause_admin",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"delay_admin",
		"deny",
		Some(&[Token::Address(Keyring::Alice.into())]),
	)
	.unwrap();
	 */

	// ------------------ Substrate Side ----------------------- //
	// Create router

	// Create pool

	env
}
