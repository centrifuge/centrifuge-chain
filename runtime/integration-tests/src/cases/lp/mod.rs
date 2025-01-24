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

use cfg_primitives::{AccountId, Balance, PoolId, CFG, SECONDS_PER_HOUR, SECONDS_PER_YEAR};
use cfg_traits::Seconds;
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Ratio,
	oracles::OracleKey,
	permissions::PoolRole,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata, LocalAssetId},
};
use ethabi::{
	ethereum_types::{H160, U128, U256},
	FixedBytes, Token, Uint,
};
use frame_support::{assert_ok, dispatch::RawOrigin, traits::OriginTrait};
use frame_system::pallet_prelude::OriginFor;
use hex_literal::hex;
use pallet_axelar_router::{AxelarConfig, AxelarId, DomainConfig, EvmConfig, FeeValues};
use pallet_evm::FeeCalculator;
use runtime_common::{oracle::Feeder, routing::RouterId};
pub use setup_lp::*;
use sp_core::{bounded_vec::BoundedVec, Get};
use sp_runtime::traits::One;

use crate::{
	cases::lp::utils::{pool_a_tranche_1_id, pool_b_tranche_1_id, pool_b_tranche_2_id, Decoder},
	config::Runtime,
	env::{Blocks, Env, EnvEvmExtension, EvmEnv},
	envs::runtime_env::RuntimeEnv,
	utils::{
		accounts::{default_investors, Keyring},
		currency::{register_currency, CurrencyInfo},
		genesis,
		genesis::Genesis,
		give_balance,
		oracle::set_order_book_feeder,
		tokens::evm_balances,
	},
};

pub mod cfg_migration;
pub mod investments;
pub mod pool_management;
pub mod setup_evm;
pub mod setup_lp;
pub mod transfers;
pub mod utils;

/// A single tranched pool.
/// Pool currency: LocalUsdc
pub const POOL_A: PoolId = 1;

/// A two tranched pool.
/// Pool currency: LocalUsdc
pub const POOL_B: PoolId = 2;

/// A single tranched pool.
/// Pool currency: Usdc from other domain
pub const POOL_C: PoolId = 3;

pub const DEFAULT_BALANCE: Balance = 1_000_000;
const DECIMALS_6: Balance = 1_000_000;
const DECIMALS_18: Balance = 1_000_000_000_000_000_000;
const LOCAL_ASSET_ID: LocalAssetId = LocalAssetId(1);
const INVESTOR_VALIDITY: Seconds = Seconds::MAX;

/// The faked router address on the EVM side. Needed for the precompile to
/// verify the origin of messages.
///
/// NOTE: This is NOT the real address of the
///       router, but the one we are faking on the EVM side. Hence, it is fix
///       coded here in the same way it is fixed code on the EVM testing router.
pub const EVM_LP_INSTANCE: [u8; 20] = hex!("1111111111111111111111111111111111111111");

/// The faked domain name the LP messages are coming from and going to.
pub const EVM_DOMAIN_STR: &str = "TestDomain";

/// The test domain ChainId for the tests.
pub const EVM_DOMAIN_CHAIN_ID: u64 = 1;

pub const EVM_DOMAIN: Domain = Domain::Evm(EVM_DOMAIN_CHAIN_ID);

pub const EVM_ROUTER_ID: RouterId = RouterId::Axelar(AxelarId::Evm(EVM_DOMAIN_CHAIN_ID));

/// Represents Solidity enum Domain.Centrifuge
pub const DOMAIN_CENTRIFUGE: u8 = 0;

/// Represents Solidity enum Domain.Evm
pub const DOMAIN_EVM: u8 = 1;
/// Represents Centrifuge Chain id which is 0
pub const CENTRIFUGE_CHAIN_ID: u8 = 0;

/// The address of the local restriction manager contract required for
/// `AddTranche` message
pub const LOCAL_RESTRICTION_MANAGER_ADDRESS: [u8; 20] =
	hex_literal::hex!("193356f6df34af00288f98bbb34d6ec98512ed32");

pub mod contracts {
	pub const ROOT: &str = "Root";
	pub const ESCROW: &str = "Escrow";
	pub const POOL_MANAGER: &str = "PoolManager";
	pub const LP_FACTORY: &str = "ERC7540VaultFactory";
	pub const LP: &str = "ERC7540Vault";
	pub const RESTRICTION_MANAGER: &str = "RestrictionManager";
	pub const TRANCHE_FACTORY: &str = "TrancheFactory";
	pub const TRANCHE_TOKEN: &str = "Tranche";
	pub const INVESTMENT_MANAGER: &str = "InvestmentManager";
	pub const GAS_SERVICE: &str = "GasService";
	pub const ADAPTER: &str = "LocalAdapter";
	pub const GATEWAY: &str = "Gateway";
	pub const ROUTER: &str = "CentrifugeRouter";
	pub const GUARDIAN: &str = "Guardian";
	pub const TRANSFER_PROXY_FACTORY: &str = "TransferProxyFactory";
}

pub mod names {
	pub const ROOT: &str = "root";
	pub const ESCROW: &str = "escrow";
	pub const POOL_MANAGER: &str = "pool_manager";
	pub const LP_FACTORY: &str = "vault_factory";
	pub const RESTRICTION_MANAGER: &str = "restriction_manager";
	pub const TRANCHE_FACTORY: &str = "tranche_factory";
	pub const INVESTMENT_MANAGER: &str = "investment_manager";
	pub const GAS_SERVICE: &str = "gas_service";
	pub const ADAPTER: &str = "adapter";
	pub const ADAPTERS: &str = "adapters";
	pub const GATEWAY: &str = "gateway";
	pub const ROUTER_ESCROW: &str = "router_escrow";
	pub const ROUTER: &str = "router";
	pub const GUARDIAN: &str = "guardian";
	pub const TRANSFER_PROXY_FACTORY: &str = "transfer_proxy_factory";

	pub const USDC: &str = "usdc";
	pub const FRAX: &str = "frax";
	pub const DAI: &str = "dai";
	pub const POOL_A_T_1: &str = "lp_pool_a_tranche_1";
	pub const RM_POOL_A_T_1: &str = "rm_lp_pool_a_tranche_1";
	pub const POOL_A_T_1_DAI: &str = "lp_pool_a_tranche_1_dai";
	pub const POOL_A_T_1_FRAX: &str = "lp_pool_a_tranche_1_frax";
	pub const POOL_A_T_1_USDC: &str = "lp_pool_a_tranche_1_usdc";

	pub const POOL_B_T_1: &str = "lp_pool_b_tranche_1";
	pub const RM_POOL_B_T_1: &str = "rm_lp_pool_b_tranche_1";
	pub const POOL_B_T_1_DAI: &str = "lp_pool_b_tranche_1_dai";
	pub const POOL_B_T_1_FRAX: &str = "lp_pool_b_tranche_1_frax";
	pub const POOL_B_T_1_USDC: &str = "lp_pool_b_tranche_1_usdc";

	pub const POOL_B_T_2: &str = "lp_pool_b_tranche_2";
	pub const RM_POOL_B_T_2: &str = "rm_lp_pool_b_tranche_2";
	pub const POOL_B_T_2_DAI: &str = "lp_pool_b_tranche_2_dai";
	pub const POOL_B_T_2_FRAX: &str = "lp_pool_b_tranche_2_frax";
	pub const POOL_B_T_2_USDC: &str = "lp_pool_b_tranche_2_usdc";
	pub const POOL_C_T_1: &str = "lp_pool_c_tranche_1";
	pub const RM_POOL_C_T_1: &str = "rm_lp_pool_c_tranche_1";
	pub const POOL_C_T_1_USDC: &str = "lp_pool_b_tranche_1_usdc";
	pub const POOL_C_T_1_FRAX: &str = "lp_pool_b_tranche_1_frax";
	pub const POOL_C_T_1_DAI: &str = "lp_pool_b_tranche_1_dai";
}

// Values based on deployer script: https://github.com/centrifuge/liquidity-pools/blob/b19bf62a3a49b8452999b9250dbd3229f60ee757/script/Deployer.sol#L53
pub mod gas {
	pub const PROOF_COST: u64 = 20000000000000000;
	pub const MSG_COST: u64 = 20000000000000000;
	pub const GAS_PRICE: u128 = 2500000000000000000;
	pub const TOKEN_PRICE: u128 = 178947400000000;
}

#[allow(non_camel_case_types)]
pub struct USDC;
impl CurrencyInfo for USDC {
	fn custom(&self) -> CustomMetadata {
		CustomMetadata {
			pool_currency: true,
			transferability: CrossChainTransferability::LiquidityPools,
			permissioned: false,
			mintable: false,
			local_representation: Some(LOCAL_ASSET_ID),
		}
	}

	fn ed(&self) -> Balance {
		10_000
	}

	fn symbol(&self) -> &'static str {
		"USDC"
	}

	fn id(&self) -> CurrencyId {
		CurrencyId::ForeignAsset(100_001)
	}

	fn decimals(&self) -> u32 {
		6
	}
}

#[allow(non_camel_case_types)]
pub struct DAI;
impl CurrencyInfo for DAI {
	fn custom(&self) -> CustomMetadata {
		CustomMetadata {
			pool_currency: true,
			transferability: CrossChainTransferability::LiquidityPools,
			permissioned: false,
			mintable: false,
			local_representation: None,
		}
	}

	fn symbol(&self) -> &'static str {
		"DAI"
	}

	fn id(&self) -> CurrencyId {
		CurrencyId::ForeignAsset(100_002)
	}

	fn ed(&self) -> Balance {
		100_000_000_000_000
	}

	fn decimals(&self) -> u32 {
		18
	}
}

#[allow(non_camel_case_types)]
pub struct FRAX;
impl CurrencyInfo for FRAX {
	fn custom(&self) -> CustomMetadata {
		CustomMetadata {
			pool_currency: true,
			transferability: CrossChainTransferability::LiquidityPools,
			permissioned: false,
			mintable: false,
			local_representation: None,
		}
	}

	fn symbol(&self) -> &'static str {
		"FRAX"
	}

	fn id(&self) -> CurrencyId {
		CurrencyId::ForeignAsset(100_003)
	}

	fn ed(&self) -> Balance {
		100_000_000_000_000
	}

	fn decimals(&self) -> u32 {
		18
	}
}

#[allow(non_camel_case_types)]
pub struct LocalUSDC;
impl CurrencyInfo for LocalUSDC {
	fn custom(&self) -> CustomMetadata {
		CustomMetadata {
			pool_currency: true,
			transferability: CrossChainTransferability::None,
			permissioned: false,
			mintable: false,
			local_representation: None,
		}
	}

	fn symbol(&self) -> &'static str {
		"LocalUSDC"
	}

	fn id(&self) -> CurrencyId {
		CurrencyId::LocalAsset(LOCAL_ASSET_ID)
	}

	fn ed(&self) -> Balance {
		10_000
	}

	fn decimals(&self) -> u32 {
		6
	}
}
