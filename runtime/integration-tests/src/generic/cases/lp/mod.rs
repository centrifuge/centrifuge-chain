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
use cfg_types::{
	domain_address::Domain,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
};
use ethabi::{ethereum_types::U256, Token, Uint};
use frame_support::{
	assert_ok,
	dispatch::RawOrigin,
	pallet_prelude::ConstU32,
	traits::{OriginTrait, PalletInfo},
	BoundedVec,
};
use frame_system::pallet_prelude::OriginFor;
use liquidity_pools_gateway_routers::{
	AxelarEVMRouter, DomainRouter, EVMDomain, EVMRouter, FeeValues, MAX_AXELAR_EVM_CHAIN_SIZE,
};
use orml_traits::asset_registry::AssetMetadata;
use pallet_evm::FeeCalculator;
use sp_core::Get;
use sp_runtime::traits::{BlakeTwo256, Hash};
use xcm::v3::{
	Junction::{AccountKey20, GlobalConsensus, PalletInstance},
	Junctions::X3,
	NetworkId,
};

use crate::{
	generic::{
		config::Runtime,
		env::{Env, EvmEnv},
		envs::runtime_env::RuntimeEnv,
		utils::{genesis, genesis::Genesis, give_balance, last_event},
	},
	utils::accounts::Keyring,
};

pub mod utils {
	use std::cmp::min;

	use cfg_primitives::Balance;
	use ethabi::{
		ethereum_types::{H160, H256, U256},
		Log, Token,
	};
	use frame_support::traits::Len;
	use pallet_evm::CallInfo;
	use sp_runtime::DispatchError;

	pub fn to_fixed_array<const S: usize>(src: &[u8]) -> [u8; S] {
		let mut dest = [0; S];
		let len = min(src.len(), S);
		dest[..len].copy_from_slice(&src[..len]);

		dest
	}

	/*
	struct TokenWrapper(Token);

	impl From<Token> for TokenWrapper {
		fn from(value: Token) -> Self {
			TokenWrapper(value)
		}
	}

	impl Input for dyn Into<TokenWrapper> {
		fn input(&self) -> &[u8] {
			let wrapper: TokenWrapper = self.into();
			match wrapper.0 {
				Token::Address(addr) => addr.as_bytes(),
				Token::FixedBytes(bytes) => bytes.as_slice(),
				Token::Bytes(bytes) => bytes.as_slice(),
				Token::Int(int) => int.0.as_slice(),
				Token::Uint(uint) => uint.0.as_slice(),
				Token::Bool(b) => {
					if b {
						&[1]
					} else {
						&[0]
					}
				}
				Token::String(str) => str.as_bytes(),
				Token::FixedArray(fixed) => {
					todo!()
				}
				Token::Array(arr) => arr.as_slice(),
				Token::Tuple(t) => {
					todo!()
				}
			}
		}
	}
	*/

	trait Input {
		fn input(&self) -> &[u8];
	}

	impl Input for Vec<u8> {
		fn input(&self) -> &[u8] {
			self.as_slice()
		}
	}

	pub trait Decoder<T> {
		fn decode(&self) -> T;
	}

	impl<T: Input> Decoder<H160> for T {
		fn decode(&self) -> H160 {
			assert!(self.input().len() == 32);

			H160::from(to_fixed_array(&self.input()[12..]))
		}
	}

	impl<T: Input> Decoder<H256> for T {
		fn decode(&self) -> H256 {
			assert!(self.input().len() == 32);

			H256::from(to_fixed_array(self.input()))
		}
	}

	impl<T: Input> Decoder<Balance> for T {
		fn decode(&self) -> Balance {
			assert!(self.input().len() == 32);

			Balance::from_be_bytes(to_fixed_array(&self.input()[16..]))
		}
	}

	impl<T: Input> Decoder<U256> for T {
		fn decode(&self) -> U256 {
			let len = self.input().len();
			if len == 1 {
				U256::from(u8::from_be_bytes(to_fixed_array(&self.input())))
			} else if len == 2 {
				U256::from(u16::from_be_bytes(to_fixed_array(&self.input())))
			} else if len == 4 {
				U256::from(u32::from_be_bytes(to_fixed_array(&self.input())))
			} else if len == 8 {
				U256::from(u64::from_be_bytes(to_fixed_array(&self.input())))
			} else if len == 16 {
				U256::from(u128::from_be_bytes(to_fixed_array(&self.input())))
			} else if len == 32 {
				U256::from_big_endian(to_fixed_array::<32>(&self.input()).as_slice())
			} else {
				panic!("Invalid slice length.")
			}
		}
	}
}

pub mod pool_management;

pub const DEFAULT_BALANCE: Balance = 1_000_000;

pub const DECIMALS_6: Balance = 1_000_000;
pub const DECIMALS_18: Balance = 1_000_000_000_000_000_000;

pub const USDC: CurrencyId = CurrencyId::ForeignAsset(100_001);
pub const DAI: CurrencyId = CurrencyId::ForeignAsset(100_002);
pub const FRAX: CurrencyId = CurrencyId::ForeignAsset(100_003);

/// The faked router address on the EVM side. Needed for the precompile to
/// verify the origin of messages.
///
/// NOTE: This is NOT the real address of the
/// router, but the one we are faking on the EVM side.
pub const EVM_ROUTER: &str = "0x1111111111111111111111111111111111111111";

/// The faked domain name the LP messages are coming from and going to.
pub const EVM_DOMAIN: &str = "TestDomain";

/// The test domain ChainId for the tests.
pub const EVM_DOMAIN_CHAIN_ID: u64 = 1;

pub fn process_outbound<T: Runtime>() {
	pallet_liquidity_pools_gateway::OutboundMessageQueue::<T>::iter()
		.map(|(nonce, _)| nonce)
		.collect::<Vec<_>>()
		.into_iter()
		.for_each(|nonce| {
			pallet_liquidity_pools_gateway::Pallet::<T>::process_outbound_message(
				OriginFor::<T>::signed(Keyring::Alice.into()),
				nonce,
			)
			.unwrap();

			assert!(matches!(
				last_event::<T, pallet_liquidity_pools_gateway::Event::<T>>(),
				pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionSuccess { .. }
			));
		});
}

pub fn setup_full<T: Runtime>() -> impl EvmEnv<T> {
	setup::<T>(|env| {
		setup_currencies(env);
		setup_pools(env);
		setup_tranches(env);
		setup_investment_currencies(env);
		setup_deploy_lps(env);
	})
}

pub fn setup<T: Runtime>(additional: impl FnOnce(&mut RuntimeEnv<T>)) -> impl EvmEnv<T> {
	let mut env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(DEFAULT_BALANCE * CFG))
			.storage(),
	)
	.load_contracts();

	// Fund gateway sender
	env.parachain_state_mut(|| {
		give_balance::<T>(
			<T as pallet_liquidity_pools_gateway::Config>::Sender::get(),
			DEFAULT_BALANCE * CFG,
		)
	});

	/* TODO: Use that but index needed contracts afterwards
	   env.deploy("LocalRouterScript", "lp_deploy", Keyring::Alice, None);
	   env.call_mut(Keyring::Alice, Default::default(), "lp_deploy", "run", None)
		   .unwrap();
	*/

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

	// ------------------ Substrate Side ----------------------- //
	// Create router
	let (base_fee, _) =
		env.parachain_state(<T as pallet_evm::Config>::FeeCalculator::min_gas_price);

	let evm_domain = EVMDomain {
		target_contract_address: sp_core::H160::from(env.deployed("router").address().0),
		target_contract_hash: BlakeTwo256::hash_of(&env.deployed("router").deployed_bytecode),
		fee_values: FeeValues {
			value: sp_core::U256::zero(),
			gas_limit: sp_core::U256::from(500_000),
			gas_price: sp_core::U256::from(base_fee),
		},
	};

	let axelar_evm_router = AxelarEVMRouter::<T>::new(
		EVMRouter::new(evm_domain),
		BoundedVec::<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>::try_from(
			EVM_DOMAIN.as_bytes().to_vec(),
		)
		.unwrap(),
		sp_core::H160::from(env.deployed("router").address().0),
	);

	env.parachain_state_mut(|| {
		assert_ok!(
			pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_router(
				RawOrigin::Root.into(),
				Domain::EVM(EVM_DOMAIN_CHAIN_ID),
				DomainRouter::<T>::AxelarEVM(axelar_evm_router),
			)
		);
	});

	additional(&mut env);

	env.__priv_build_block(2);
	env
}

pub fn setup_investment_currencies<T: Runtime>(env: &mut impl EvmEnv<T>) {
	// Pool 1, Tranche 1
	// AllowInvestmentCurrency 1
	// AllowInvestmentCurrency 2
	// AllowInvestmentCurrency 3

	// Pool 2, Tranche 2
	// AllowInvestmentCurrency 1
	// AllowInvestmentCurrency 2
	// AllowInvestmentCurrency 3
}

pub fn setup_deploy_lps<T: Runtime>(env: &mut impl EvmEnv<T>) {
	// ------------------ EVM Side ----------------------- //
	// Deploy LP and more for both pools and all currencies
}

pub fn setup_tranches<T: Runtime>(env: &mut impl EvmEnv<T>) {
	// AddTranche 1 A

	// AddTranche 1 B
	// AddTranche 2 B
}

pub fn setup_pools<T: Runtime>(env: &mut impl EvmEnv<T>) {
	setup_currencies(env);

	// Create 2x pools
	// * single tranched pool A
	// * double tranched pool B

	// AddPool A

	// AddPool B
}

pub fn setup_currencies<T: Runtime>(env: &mut impl EvmEnv<T>) {
	// Create 3x ERC-20 currency as Stablecoins
	//
	// NOTE: Called by Keyring::Admin, as admin controls all in this setup
	env.deploy(
		"ERC20",
		"usdc",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(6))]),
	);
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"usdc",
		"file",
		Some(&[
			Token::FixedBytes("name".as_bytes().to_vec()),
			Token::String("USD Coin".to_string()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"usdc",
		"file",
		Some(&[
			Token::FixedBytes("symbol".as_bytes().to_vec()),
			Token::String("USDC".to_string()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"usdc",
		"mint",
		Some(&[
			Token::Address(Keyring::Alice.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"usdc",
		"mint",
		Some(&[
			Token::Address(Keyring::Bob.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"usdc",
		"mint",
		Some(&[
			Token::Address(Keyring::Charlie.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();

	env.deploy(
		"ERC20",
		"frax",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(18))]),
	);
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"frax",
		"file",
		Some(&[
			Token::FixedBytes("name".as_bytes().to_vec()),
			Token::String("Frax Coin".to_string()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"frax",
		"file",
		Some(&[
			Token::FixedBytes("symbol".as_bytes().to_vec()),
			Token::String("FRAX".to_string()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"frax",
		"mint",
		Some(&[
			Token::Address(Keyring::Alice.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"frax",
		"mint",
		Some(&[
			Token::Address(Keyring::Bob.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"frax",
		"mint",
		Some(&[
			Token::Address(Keyring::Charlie.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();

	env.deploy(
		"ERC20",
		"dai",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(18))]),
	);
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"dai",
		"file",
		Some(&[
			Token::FixedBytes("name".as_bytes().to_vec()),
			Token::String("Dai Coin".to_string()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"dai",
		"file",
		Some(&[
			Token::FixedBytes("symbol".as_bytes().to_vec()),
			Token::String("DAI".to_string()),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"dai",
		"mint",
		Some(&[
			Token::Address(Keyring::Alice.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"dai",
		"mint",
		Some(&[
			Token::Address(Keyring::Bob.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	env.call_mut(
		Keyring::Admin,
		Default::default(),
		"dai",
		"mint",
		Some(&[
			Token::Address(Keyring::Charlie.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();

	// AddCurrency
	// * register in OrmlAssetRegistry
	// * trigger `AddCurrency`
	let usdc_address = env.deployed("usdc").address();
	env.parachain_state_mut(|| {
		register_asset::<T>("USD Coin", "USDC", 6, 10_000, usdc_address, USDC)
	});
	let dai_address = env.deployed("dai").address();
	env.parachain_state_mut(|| {
		register_asset::<T>("Dai Coin", "DAI", 18, 100_000_000_000_000, dai_address, DAI)
	});
	let frax_address = env.deployed("frax").address();
	env.parachain_state_mut(|| {
		register_asset::<T>(
			"Frax Coin",
			"FRAX",
			18,
			100_000_000_000_000,
			frax_address,
			FRAX,
		)
	});

	env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			USDC
		));
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			DAI
		));
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			FRAX
		));

		process_outbound::<T>()
	});
}

pub fn register_asset<T: Runtime>(
	name: impl Into<Vec<u8>>,
	symbol: impl Into<Vec<u8>>,
	decimals: u32,
	existential_deposit: Balance,
	address: impl Into<[u8; 20]>,
	currency: CurrencyId,
) {
	assert_ok!(orml_asset_registry::Pallet::<T>::register_asset(
		RawOrigin::Root.into(),
		AssetMetadata {
			decimals: decimals,
			name: name.into(),
			symbol: symbol.into(),
			existential_deposit: existential_deposit,
			location: Some(
				X3(
					PalletInstance(
						<T as frame_system::Config>::PalletInfo::index::<
							pallet_liquidity_pools::Pallet<T>,
						>()
						.unwrap()
						.try_into()
						.unwrap()
					),
					GlobalConsensus(NetworkId::Ethereum {
						chain_id: EVM_DOMAIN_CHAIN_ID
					}),
					AccountKey20 {
						key: address.into(),
						network: None,
					}
				)
				.into()
			),
			additional: CustomMetadata {
				transferability: CrossChainTransferability::LiquidityPools,
				mintable: false,
				permissioned: false,
				pool_currency: true
			}
		},
		Some(currency),
	));
}
