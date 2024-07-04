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

use axelar_gateway_precompile::SourceConverter;
use cfg_primitives::{Balance, PoolId, CFG, SECONDS_PER_HOUR, SECONDS_PER_YEAR};
use cfg_traits::Seconds;
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	permissions::PoolRole,
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata, LocalAssetId},
};
use ethabi::{
	ethereum_types::{H160, U128, U256},
	FixedBytes, Token, Uint,
};
use frame_support::{
	assert_ok, dispatch::RawOrigin, pallet_prelude::ConstU32, traits::OriginTrait, BoundedVec,
};
use frame_system::pallet_prelude::OriginFor;
use hex_literal::hex;
use liquidity_pools_gateway_routers::{
	AxelarEVMRouter, DomainRouter, EVMDomain, EVMRouter, FeeValues, MAX_AXELAR_EVM_CHAIN_SIZE,
};
use pallet_evm::FeeCalculator;
use runtime_common::account_conversion::AccountConverter;
use sp_core::Get;
use sp_runtime::traits::{BlakeTwo256, Hash};

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

pub mod investments;
pub mod pool_management;
pub mod transfers;

pub mod utils {
	use std::{cmp::min, fmt::Debug};

	use cfg_primitives::{Balance, TrancheId};
	use cfg_types::domain_address::DomainAddress;
	use ethabi::ethereum_types::{H160, H256, U256};
	use fp_evm::CallInfo;
	use frame_support::traits::{OriginTrait, PalletInfo};
	use frame_system::pallet_prelude::OriginFor;
	use pallet_evm::ExecutionInfo;
	use sp_core::{ByteArray, Get};
	use sp_runtime::{
		traits::{Convert, EnsureAdd},
		DispatchError,
	};
	use staging_xcm::{
		v4::{
			Junction::{AccountKey20, GlobalConsensus, PalletInstance},
			NetworkId,
		},
		VersionedLocation,
	};

	use crate::{
		cases::lp::{EVM_DOMAIN_CHAIN_ID, POOL_A, POOL_B, POOL_C},
		config::Runtime,
		utils::{accounts::Keyring, evm::receipt_ok, last_event, pool::get_tranche_ids},
	};

	pub fn remote_account_of<T: Runtime>(
		keyring: Keyring,
	) -> <T as frame_system::Config>::AccountId {
		<T as pallet_liquidity_pools::Config>::DomainAddressToAccountId::convert(
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, keyring.into()),
		)
	}

	pub const REVERT_ERR: Result<CallInfo, DispatchError> =
		Err(DispatchError::Other("EVM call failed: Revert"));

	pub fn lp_asset_location<T: Runtime>(address: H160) -> VersionedLocation {
		[
			PalletInstance(
				<T as frame_system::Config>::PalletInfo::index::<pallet_liquidity_pools::Pallet<T>>()
					.unwrap()
					.try_into()
					.unwrap(),
			),
			GlobalConsensus(NetworkId::Ethereum {
				chain_id: EVM_DOMAIN_CHAIN_ID,
			}),
			AccountKey20 {
				key: address.into(),
				network: None,
			}
		].into()
	}

	pub fn pool_a_tranche_1_id<T: Runtime>() -> TrancheId {
		*get_tranche_ids::<T>(POOL_A)
			.get(0)
			.expect("Pool A has one non-residuary tranche")
	}
	pub fn pool_b_tranche_1_id<T: Runtime>() -> TrancheId {
		*get_tranche_ids::<T>(POOL_B)
			.get(0)
			.expect("Pool B has two non-residuary tranches")
	}
	pub fn pool_b_tranche_2_id<T: Runtime>() -> TrancheId {
		*get_tranche_ids::<T>(POOL_B)
			.get(1)
			.expect("Pool B has two non-residuary tranches")
	}

	pub fn pool_c_tranche_1_id<T: Runtime>() -> TrancheId {
		*get_tranche_ids::<T>(POOL_C)
			.get(0)
			.expect("Pool B has two non-residuary tranches")
	}

	pub fn verify_outbound_failure_on_lp<T: Runtime>(to: H160) {
		let (_tx, status, receipt) = pallet_ethereum::Pending::<T>::get()
			.last()
			.expect("Queue triggered evm tx.")
			.clone();

		// The sender is the sender account on the gateway
		assert_eq!(
			status.from.0,
			<T as pallet_liquidity_pools_gateway::Config>::Sender::get().as_slice()[0..20]
		);
		assert_eq!(status.to.unwrap().0, to.0);
		assert!(!receipt_ok(receipt));
		assert!(matches!(
			last_event::<T, pallet_liquidity_pools_gateway::Event::<T>>(),
			pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionFailure { .. }
		));
	}

	pub fn verify_outbound_success<T: Runtime>(
		message: <T as pallet_liquidity_pools_gateway::Config>::Message,
	) {
		assert!(matches!(
			last_event::<T, pallet_liquidity_pools_gateway::Event::<T>>(),
			pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionSuccess {
				message: processed_message,
				..
			} if processed_message == message
		));
	}

	pub fn process_outbound<T: Runtime>(
		mut verifier: impl FnMut(<T as pallet_liquidity_pools_gateway::Config>::Message),
	) {
		let msgs = pallet_liquidity_pools_gateway::OutboundMessageQueue::<T>::iter()
			.map(|(nonce, (_, _, msg))| (nonce, msg))
			.collect::<Vec<_>>();

		// The function should panic if there is nothing to be processed.
		assert!(msgs.len() > 0);

		msgs.into_iter().for_each(|(nonce, msg)| {
			pallet_liquidity_pools_gateway::Pallet::<T>::process_outbound_message(
				OriginFor::<T>::signed(Keyring::Alice.into()),
				nonce,
			)
			.unwrap();

			verifier(msg);
		});
	}

	pub fn to_fixed_array<const S: usize>(src: &[u8]) -> [u8; S] {
		let mut dest = [0; S];
		let len = min(src.len(), S);
		dest[..len].copy_from_slice(&src[..len]);

		dest
	}

	pub fn as_h160_32bytes(who: Keyring) -> [u8; 32] {
		let mut address = [0u8; 32];
		address[..20].copy_from_slice(H160::from(who).as_bytes());
		address
	}

	trait Input {
		fn input(&self) -> &[u8];
	}

	impl Input for Vec<u8> {
		fn input(&self) -> &[u8] {
			self.as_slice()
		}
	}

	impl<E: Debug> Input for Result<Vec<u8>, E> {
		fn input(&self) -> &[u8] {
			match self {
				Ok(arr) => arr.as_slice(),
				Err(e) => panic!("Input received error: {:?}", e),
			}
		}
	}

	impl<E: Debug> Input for Result<ExecutionInfo<Vec<u8>>, E> {
		fn input(&self) -> &[u8] {
			match self {
				Ok(arr) => arr.value.as_slice(),
				Err(e) => panic!("Input received error: {:?}", e),
			}
		}
	}

	pub trait Decoder<T> {
		fn decode(&self) -> T;
	}

	impl<T: Input> Decoder<(bool, u64)> for T {
		fn decode(&self) -> (bool, u64) {
			assert!(self.input().len() > 32);

			let left = &self.input()[..32];
			let right = &self.input()[32..];

			let unsigned64 = match right.len() {
				1 => u64::from(u8::from_be_bytes(to_fixed_array(&right))),
				2 => u64::from(u16::from_be_bytes(to_fixed_array(&right))),
				4 => u64::from(u32::from_be_bytes(to_fixed_array(&right))),
				8 => u64::from_be_bytes(to_fixed_array(&right)),
				// EVM stores in 32 byte slots with left-padding
				16 => u64::from_be_bytes(to_fixed_array::<8>(&right[28..])),
				32 => u64::from_be_bytes(to_fixed_array::<8>(&right[24..])),
				_ => {
					panic!("Invalid slice length for u64 derivation");
				}
			};

			(left[31] == 1u8, unsigned64)
		}
	}

	impl<T: Input> Decoder<H160> for T {
		fn decode(&self) -> H160 {
			assert_eq!(self.input().len(), 32usize);

			H160::from(to_fixed_array(&self.input()[12..]))
		}
	}

	impl<T: Input> Decoder<H256> for T {
		fn decode(&self) -> H256 {
			assert_eq!(self.input().len(), 32usize);

			H256::from(to_fixed_array(self.input()))
		}
	}

	impl<T: Input> Decoder<bool> for T {
		fn decode(&self) -> bool {
			assert_eq!(self.input().len(), 32usize);

			// In EVM the last byte of the U256 is set to 1 if true else to false
			self.input()[31] == 1u8
		}
	}

	impl<T: Input> Decoder<Balance> for T {
		fn decode(&self) -> Balance {
			assert_eq!(self.input().len(), 32usize);

			Balance::from_be_bytes(to_fixed_array(&self.input()[16..]))
		}
	}

	impl<T: Input> Decoder<U256> for T {
		fn decode(&self) -> U256 {
			match self.input().len() {
				1 => U256::from(u8::from_be_bytes(to_fixed_array(&self.input()))),
				2 => U256::from(u16::from_be_bytes(to_fixed_array(&self.input()))),
				4 => U256::from(u32::from_be_bytes(to_fixed_array(&self.input()))),
				8 => U256::from(u64::from_be_bytes(to_fixed_array(&self.input()))),
				16 => U256::from(u128::from_be_bytes(to_fixed_array(&self.input()))),
				32 => U256::from_big_endian(to_fixed_array::<32>(&self.input()).as_slice()),
				_ => {
					panic!("Invalid slice length for u256 derivation")
				}
			}
		}
	}

	impl<T: Input> Decoder<(u128, u64)> for T {
		fn decode(&self) -> (u128, u64) {
			assert!(self.input().len() >= 32);

			let left = &self.input()[..32];
			let right = &self.input()[32..];

			let unsigned128 = match left.len() {
				1 => u128::from(u8::from_be_bytes(to_fixed_array(&left))),
				2 => u128::from(u16::from_be_bytes(to_fixed_array(&left))),
				4 => u128::from(u32::from_be_bytes(to_fixed_array(&left))),
				8 => u128::from(u64::from_be_bytes(to_fixed_array(&left))),
				16 => u128::from(u128::from_be_bytes(to_fixed_array(&left))),
				32 => {
					let x = u128::from_be_bytes(to_fixed_array::<16>(&left[..16]));
					let y = u128::from_be_bytes(to_fixed_array::<16>(&left[16..]));
					x.ensure_add(y)
						.expect("Price is initialized as u128 on EVM side")
				}
				_ => {
					panic!("Invalid slice length for u128 derivation");
				}
			};

			let unsigned64 = match right.len() {
				1 => u64::from(u8::from_be_bytes(to_fixed_array(&right))),
				2 => u64::from(u16::from_be_bytes(to_fixed_array(&right))),
				4 => u64::from(u32::from_be_bytes(to_fixed_array(&right))),
				8 => u64::from_be_bytes(to_fixed_array(&right)),
				// EVM stores in 32 byte slots with left-padding
				16 => u64::from_be_bytes(to_fixed_array::<8>(&right[28..])),
				32 => u64::from_be_bytes(to_fixed_array::<8>(&right[24..])),
				_ => {
					panic!("Invalid slice length for u64 derivation");
				}
			};

			(unsigned128, unsigned64)
		}
	}

	impl<T: Input> Decoder<u8> for T {
		fn decode(&self) -> u8 {
			assert_eq!(self.input().len(), 32usize);

			self.input()[31]
		}
	}
}

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
const INVESTOR_VALIDIDITY: Seconds = Seconds::MAX;

// TODO(@william): Write tests which ensures all contracts exists!

pub mod contracts {
	pub const ROOT: &str = "Root";
	pub const ESCROW: &str = "Escrow";
	pub const POOL_MANAGER: &str = "PoolManager";
	pub const LP_FACTORY: &str = "ERC7540VaultFactory";
	pub const RESTRICTION_MANAGER: &str = "RestrictionManager";
	pub const TRANCHE_FACTORY: &str = "TrancheFactory";
	pub const TRANCHE_TOKEN: &str = "TrancheToken";
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
	pub const AXELAR_ADAPTER: &str = "axelar-adapter";
	pub const GATEWAY: &str = "gateway";
	pub const ROUTER_ESCROW: &str = "router_escrow";
	pub const ROUTER: &str = "centrifuge_router";
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

pub const EVM_DOMAIN: Domain = Domain::EVM(EVM_DOMAIN_CHAIN_ID);

pub fn setup_full<T: Runtime>() -> impl EnvEvmExtension<T> {
	setup::<T, _>(|evm| {
		setup_currencies(evm);
		setup_pools(evm);
		setup_tranches(evm);
		setup_investment_currencies(evm);
		setup_deploy_lps(evm);
		setup_investors(evm)
	})
}

/// Default setup required for EVM <> CFG communication
pub fn setup<T: Runtime, F: FnOnce(&mut <RuntimeEnv<T> as EnvEvmExtension<T>>::EvmEnv)>(
	additional: F,
) -> impl EnvEvmExtension<T> {
	let mut env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(DEFAULT_BALANCE * CFG))
			.storage(),
	);
	env.state_mut(|evm| {
		evm_balances::<T>(DEFAULT_BALANCE * CFG);
		set_order_book_feeder::<T>(T::RuntimeOriginExt::root());

		evm.load_contracts();

		// Fund gateway sender
		give_balance::<T>(
			<T as pallet_liquidity_pools_gateway::Config>::Sender::get(),
			DEFAULT_BALANCE * CFG,
		);

		// Register general local pool-currency
		register_currency::<T>(LocalUSDC, |_| {});

		/* TODO: Use that but index needed contracts afterwards
		   env.deploy("LocalRouterScript", "lp_deploy", Keyring::Alice, None);
		   env.call_mut(Keyring::Alice, Default::default(), "lp_deploy", "run", None)
			   .unwrap();
		*/

		// ------------------ EVM Side ----------------------- //
		// The flow is based in the following code from the Solidity and needs to be
		// adapted if this deployment script changes in the future
		// * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Axelar.s.sol#L17-L31
		// * NEW: https://github.com/centrifuge/liquidity-pools/blob/b19bf62a3a49b8452999b9250dbd3229f60ee757/script/Axelar.s.sol#L10-L27
		//
		// PART: Deploy InvestmentManager
		//   * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L45-L69
		evm.deploy(
			contracts::ESCROW,
			names::ESCROW,
			Keyring::Alice,
			Some(&[Token::Address(Keyring::Alice.into())]),
		);
		evm.deploy(
			contracts::ROOT,
			names::ROOT,
			Keyring::Alice,
			Some(&[
				Token::Address(evm.deployed(names::ESCROW).address()),
				Token::Uint(U256::from(48 * SECONDS_PER_HOUR)),
				Token::Address(Keyring::Alice.into()),
			]),
		);
		evm.deploy(
			contracts::LP_FACTORY,
			names::LP_FACTORY,
			Keyring::Alice,
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		);
		evm.deploy(
			contracts::RESTRICTION_MANAGER,
			names::RESTRICTION_MANAGER,
			Keyring::Alice,
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		);
		evm.deploy(
			contracts::TRANCHE_FACTORY,
			names::TRANCHE_FACTORY,
			Keyring::Alice,
			Some(&[
				Token::Address(evm.deployed(names::ROOT).address()),
				Token::Address(Keyring::Alice.into()),
			]),
		);
		evm.deploy(
			contracts::INVESTMENT_MANAGER,
			names::INVESTMENT_MANAGER,
			Keyring::Alice,
			Some(&[
				Token::Address(evm.deployed(names::ROOT).address()),
				Token::Address(evm.deployed(names::ESCROW).address()),
			]),
		);
		evm.deploy(
			contracts::POOL_MANAGER,
			names::POOL_MANAGER,
			Keyring::Alice,
			Some(&[
				Token::Address(evm.deployed(names::ESCROW).address()),
				Token::Address(evm.deployed(names::LP_FACTORY).address()),
				Token::Address(evm.deployed(names::TRANCHE_FACTORY).address()),
			]),
		);
		evm.deploy(
			contracts::TRANSFER_PROXY_FACTORY,
			names::TRANSFER_PROXY_FACTORY,
			Keyring::Alice,
			Some(&[Token::Address(evm.deployed(names::POOL_MANAGER).address())]),
		);
		evm.deploy(
			contracts::GAS_SERVICE,
			names::GAS_SERVICE,
			Keyring::Alice,
			Some(&[
				// NOTE: u64 in solidity
				Token::Uint(Uint::from(gas::PROOF_COST).into()),
				// NOTE: u64 in solidity
				Token::Uint(Uint::from(gas::MSG_COST).into()),
				Token::Uint(U128::from(gas::GAS_PRICE).into()),
				Token::Uint(U256::from(gas::TOKEN_PRICE).into()),
			]),
		);
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::GAS_SERVICE,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.deploy(
			contracts::GATEWAY,
			names::GATEWAY,
			Keyring::Alice,
			Some(&[
				Token::Address(evm.deployed(names::ROOT).address()),
				Token::Address(evm.deployed(names::INVESTMENT_MANAGER).address()),
				Token::Address(evm.deployed(names::POOL_MANAGER).address()),
				Token::Address(evm.deployed(names::GAS_SERVICE).address()),
			]),
		);
		evm.deploy(
			contracts::ESCROW,
			names::ROUTER_ESCROW,
			Keyring::Alice,
			Some(&[Token::Address(Keyring::Alice.into())]),
		);
		evm.deploy(
			contracts::ROUTER,
			names::ROUTER,
			Keyring::Alice,
			Some(&[
				Token::Address(evm.deployed(names::ROUTER_ESCROW).address()),
				Token::Address(evm.deployed(names::GATEWAY).address()),
				Token::Address(evm.deployed(names::POOL_MANAGER).address()),
			]),
		);
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROUTER_ESCROW,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROUTER).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROOT,
			"endorse",
			Some(&[Token::Address(evm.deployed(names::ROUTER).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROOT,
			"endorse",
			Some(&[Token::Address(evm.deployed(names::ESCROW).address())]),
		)
		.unwrap();

		evm.call(
			Keyring::Alice,
			Default::default(),
			names::LP_FACTORY,
			"rely",
			Some(&[Token::Address(evm.deployed(names::POOL_MANAGER).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::TRANCHE_FACTORY,
			"rely",
			Some(&[Token::Address(evm.deployed(names::POOL_MANAGER).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::RESTRICTION_MANAGER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::POOL_MANAGER).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::LP_FACTORY,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::TRANCHE_FACTORY,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::RESTRICTION_MANAGER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.deploy(
			contracts::GUARDIAN,
			names::GUARDIAN,
			Keyring::Alice,
			Some(&[
				// Based on https://github.com/centrifuge/liquidity-pools/blob/da4e46577712c762d069670077280112ea1c8ce8/test/integration/LocalAdapter.s.sol#L12-L14
				Token::Address(H160::from(Keyring::Admin)),
				Token::Address(evm.deployed(names::ROOT).address()),
				Token::Address(evm.deployed(names::GATEWAY).address()),
			]),
		);

		// PART: Deploy router (using the testing LocalRouter here)
		//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Axelar.s.sol#L24
		//  * NEW: https://github.com/centrifuge/liquidity-pools/blob/b19bf62a3a49b8452999b9250dbd3229f60ee757/script/Axelar.s.sol#L19-L21
		evm.deploy(contracts::ADAPTER, names::ADAPTER, Keyring::Alice, None);

		// PART: Wire router + file gateway
		//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L71-L98
		//  * NEW: https://github.com/centrifuge/liquidity-pools/blob/b19bf62a3a49b8452999b9250dbd3229f60ee757/script/Deployer.sol#L74-L101
		// Wire guardian
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROOT,
			"rely",
			Some(&[Token::Address(evm.deployed(names::GUARDIAN).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::GATEWAY,
			"rely",
			Some(&[Token::Address(evm.deployed(names::GUARDIAN).address())]),
		)
		.unwrap();
		// Wire gateway
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::GATEWAY,
			"file",
			Some(&[
				Token::FixedBytes("adapters".as_bytes().to_vec()),
				Token::Array(vec![Token::Address(evm.deployed(names::ADAPTER).address())]),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROOT,
			"rely",
			Some(&[Token::Address(evm.deployed(names::GATEWAY).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::INVESTMENT_MANAGER,
			"file",
			Some(&[
				Token::FixedBytes("poolManager".as_bytes().to_vec()),
				Token::Address(evm.deployed(names::POOL_MANAGER).address()),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::POOL_MANAGER,
			"file",
			Some(&[
				Token::FixedBytes("investmentManager".as_bytes().to_vec()),
				Token::Address(evm.deployed(names::INVESTMENT_MANAGER).address()),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::POOL_MANAGER,
			"file",
			Some(&[
				Token::FixedBytes("gasService".as_bytes().to_vec()),
				Token::Address(evm.deployed(names::GAS_SERVICE).address()),
			]),
		)
		.unwrap();

		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROUTER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::INVESTMENT_MANAGER,
			"file",
			Some(&[
				Token::FixedBytes(names::GATEWAY.as_bytes().to_vec()),
				Token::Address(evm.deployed(names::GATEWAY).address()),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::POOL_MANAGER,
			"file",
			Some(&[
				Token::FixedBytes(names::GATEWAY.as_bytes().to_vec()),
				Token::Address(evm.deployed(names::GATEWAY).address()),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::INVESTMENT_MANAGER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::INVESTMENT_MANAGER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::GATEWAY).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::INVESTMENT_MANAGER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::LP_FACTORY).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::POOL_MANAGER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::POOL_MANAGER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::GATEWAY).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::GATEWAY,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		/* NOTE: This rely is NOT needed as the LocalRouter is not permissioned
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ADAPTER,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		 */
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ESCROW,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROUTER_ESCROW,
			"rely",
			Some(&[Token::Address(evm.deployed(names::ROOT).address())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ESCROW,
			"rely",
			Some(&[Token::Address(evm.deployed(names::POOL_MANAGER).address())]),
		)
		.unwrap();

		// PART: File LocalRouter
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ADAPTER,
			"file",
			Some(&[
				Token::FixedBytes(names::GATEWAY.as_bytes().to_vec()),
				Token::Address(evm.deployed(names::GATEWAY).address()),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ADAPTER,
			"file",
			Some(&[
				Token::FixedBytes("sourceChain".as_bytes().to_vec()),
				Token::String(EVM_DOMAIN_STR.to_string()),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ADAPTER,
			"file",
			Some(&[
				Token::FixedBytes("sourceAddress".as_bytes().to_vec()),
				Token::String("0x1111111111111111111111111111111111111111".into()),
			]),
		)
		.unwrap();

		// PART: Remove deployer access
		//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Deployer.sol#L108-L121
		//  * NEW: https://github.com/centrifuge/liquidity-pools/blob/da4e46577712c762d069670077280112ea1c8ce8/script/Deployer.sol#L106-L118
		/* NOTE: This rely is NOT needed as the LocalRouter is not permissioned
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ADAPTER,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		*/
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::LP_FACTORY,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::TRANCHE_FACTORY,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::RESTRICTION_MANAGER,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROOT,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::INVESTMENT_MANAGER,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::POOL_MANAGER,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ESCROW,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROUTER_ESCROW,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::GATEWAY,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::ROUTER,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();
		evm.call(
			Keyring::Alice,
			Default::default(),
			names::GAS_SERVICE,
			"deny",
			Some(&[Token::Address(Keyring::Alice.into())]),
		)
		.unwrap();

		// ------------------ Substrate Side ----------------------- //
		// Create router
		let (base_fee, _) = <T as pallet_evm::Config>::FeeCalculator::min_gas_price();

		let evm_domain = EVMDomain {
			target_contract_address: evm.deployed(names::ADAPTER).address(),
			target_contract_hash: BlakeTwo256::hash_of(
				&evm.deployed(names::ADAPTER).deployed_bytecode,
			),
			fee_values: FeeValues {
				value: sp_core::U256::zero(),
				// FIXME: Diverges from prod (500_000)
				gas_limit: sp_core::U256::from(500_000_000),
				gas_price: sp_core::U256::from(base_fee),
			},
		};

		let axelar_evm_router = AxelarEVMRouter::<T>::new(
			EVMRouter::new(evm_domain),
			BoundedVec::<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>::try_from(
				EVM_DOMAIN_STR.as_bytes().to_vec(),
			)
			.unwrap(),
			evm.deployed(names::ADAPTER).address(),
		);

		assert_ok!(
			pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_router(
				RawOrigin::Root.into(),
				Domain::EVM(EVM_DOMAIN_CHAIN_ID),
				DomainRouter::<T>::AxelarEVM(axelar_evm_router),
			)
		);

		assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::add_instance(
			RawOrigin::Root.into(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, EVM_LP_INSTANCE)
		));

		assert_ok!(axelar_gateway_precompile::Pallet::<T>::set_gateway(
			RawOrigin::Root.into(),
			evm.deployed(names::ADAPTER).address()
		));

		assert_ok!(axelar_gateway_precompile::Pallet::<T>::set_converter(
			RawOrigin::Root.into(),
			BlakeTwo256::hash(EVM_DOMAIN_STR.as_bytes()),
			SourceConverter::new(EVM_DOMAIN),
		));

		additional(evm);
	});

	env.pass(Blocks::ByNumber(1));
	env
}

/// Enables USDC, DAI and FRAX as investment currencies for both pools A nand B.
pub fn setup_investment_currencies<T: Runtime>(_evm: &mut impl EvmEnv<T>) {
	for currency in [DAI.id(), FRAX.id(), USDC.id()] {
		for pool in [POOL_A, POOL_B, POOL_C] {
			assert_ok!(
				pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
					OriginFor::<T>::signed(Keyring::Admin.into()),
					pool,
					currency,
				),
			);
		}
	}
	utils::process_outbound::<T>(utils::verify_outbound_success::<T>)
}

/// Deploys both Liquidity Pools for USDC, DAI and FRAX by calling
/// `DeployLiquidityPool` for each possible triplet of pool, tranche and
/// investment currency id.
///
/// NOTE: EVM Side
pub fn setup_deploy_lps<T: Runtime>(evm: &mut impl EvmEnv<T>) {
	let lp_name = |pool, tranche, currency| -> &str {
		match (pool, tranche, currency) {
			(POOL_A, tranche, names::USDC) if tranche == utils::pool_a_tranche_1_id::<T>() => {
				names::POOL_A_T_1_USDC
			}
			(POOL_B, tranche, names::USDC) if tranche == utils::pool_b_tranche_1_id::<T>() => {
				names::POOL_B_T_1_USDC
			}
			(POOL_B, tranche, names::USDC) if tranche == utils::pool_b_tranche_2_id::<T>() => {
				names::POOL_B_T_2_USDC
			}
			(POOL_C, tranche, names::USDC) if tranche == utils::pool_c_tranche_1_id::<T>() => {
				names::POOL_C_T_1_USDC
			}

			(POOL_A, tranche, names::FRAX) if tranche == utils::pool_a_tranche_1_id::<T>() => {
				names::POOL_A_T_1_FRAX
			}
			(POOL_B, tranche, names::FRAX) if tranche == utils::pool_b_tranche_1_id::<T>() => {
				names::POOL_B_T_1_FRAX
			}
			(POOL_B, tranche, names::FRAX) if tranche == utils::pool_b_tranche_2_id::<T>() => {
				names::POOL_B_T_2_FRAX
			}
			(POOL_C, tranche, names::FRAX) if tranche == utils::pool_c_tranche_1_id::<T>() => {
				names::POOL_C_T_1_FRAX
			}

			(POOL_A, tranche, names::DAI) if tranche == utils::pool_a_tranche_1_id::<T>() => {
				names::POOL_A_T_1_DAI
			}
			(POOL_B, tranche, names::DAI) if tranche == utils::pool_b_tranche_1_id::<T>() => {
				names::POOL_B_T_1_DAI
			}
			(POOL_B, tranche, names::DAI) if tranche == utils::pool_b_tranche_2_id::<T>() => {
				names::POOL_B_T_2_DAI
			}
			(POOL_C, tranche, names::DAI) if tranche == utils::pool_c_tranche_1_id::<T>() => {
				names::POOL_C_T_1_DAI
			}

			(_, _, _) => {
				unimplemented!("pool, tranche, currency combination does not have a name.")
			}
		}
	};

	for (pool, tranche_id) in [
		(POOL_A, utils::pool_a_tranche_1_id::<T>()),
		(POOL_B, utils::pool_b_tranche_1_id::<T>()),
		(POOL_B, utils::pool_b_tranche_2_id::<T>()),
		(POOL_C, utils::pool_c_tranche_1_id::<T>()),
	] {
		for currency in [names::USDC, names::FRAX, names::DAI] {
			evm.call(
				Keyring::Alice,
				Default::default(),
				names::POOL_MANAGER,
				"deployVault",
				Some(&[
					Token::Uint(Uint::from(pool)),
					Token::FixedBytes(FixedBytes::from(tranche_id)),
					Token::Address(evm.deployed(currency).address()),
				]),
			)
			.unwrap();

			evm.register(
				lp_name(pool, tranche_id, currency),
				"LiquidityPool",
				Decoder::<H160>::decode(
					&evm.view(
						Keyring::Alice,
						names::POOL_MANAGER,
						"getVault",
						Some(&[
							Token::Uint(Uint::from(pool)),
							Token::FixedBytes(FixedBytes::from(tranche_id)),
							Token::Address(evm.deployed(currency).address()),
						]),
					)
					.unwrap()
					.value,
				),
			);
		}
	}
}

/// Initiates tranches on EVM via `DeployTranche` contract and then sends
/// `add_tranche(pool, tranche_id)` messages for a total of three tranches of
/// pool A and B.
pub fn setup_tranches<T: Runtime>(evm: &mut impl EvmEnv<T>) {
	// AddTranche 1 of A
	let tranche_id = {
		let tranche_id = utils::pool_a_tranche_1_id::<T>();
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_A,
			tranche_id,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

		tranche_id
	};
	evm.call(
		Keyring::Alice,
		Default::default(),
		names::POOL_MANAGER,
		"deployTranche",
		Some(&[
			Token::Uint(Uint::from(POOL_A)),
			Token::FixedBytes(FixedBytes::from(tranche_id)),
		]),
	)
	.unwrap();
	evm.register(
		names::POOL_A_T_1,
		contracts::TRANCHE_TOKEN,
		Decoder::<H160>::decode(
			&evm.view(
				Keyring::Alice,
				names::POOL_MANAGER,
				"getTranche",
				Some(&[
					Token::Uint(POOL_A.into()),
					Token::FixedBytes(tranche_id.to_vec()),
				]),
			)
			.unwrap()
			.value,
		),
	);
	// FIXME: Fails
	// evm.register(
	// 	names::RM_POOL_A_T_1,
	// 	contracts::RESTRICTION_MANAGER,
	// 	Decoder::<H160>::decode(
	// 		&evm.view(
	// 			Keyring::Alice,
	// 			names::POOL_A_T_1,
	// 			"restrictionManager",
	// 			None,
	// 		)
	// 		.unwrap()
	// 		.value,
	// 	),
	// );

	// AddTranche 1 of B
	let tranche_id = {
		let tranche_id = utils::pool_b_tranche_1_id::<T>();
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_B,
			tranche_id,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

		tranche_id
	};
	evm.call(
		Keyring::Alice,
		Default::default(),
		names::POOL_MANAGER,
		"deployTranche",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id)),
		]),
	)
	.unwrap();
	evm.register(
		names::POOL_B_T_1,
		contracts::TRANCHE_TOKEN,
		Decoder::<H160>::decode(
			&evm.view(
				Keyring::Alice,
				names::POOL_MANAGER,
				"getTranche",
				Some(&[
					Token::Uint(POOL_B.into()),
					Token::FixedBytes(tranche_id.to_vec()),
				]),
			)
			.unwrap()
			.value,
		),
	);
	// FIXME: Fails
	// evm.register(
	// 	names::RM_POOL_B_T_1,
	// 	contracts::RESTRICTION_MANAGER,
	// 	Decoder::<H160>::decode(
	// 		&evm.view(
	// 			Keyring::Alice,
	// 			names::POOL_B_T_1,
	// 			"restrictionManager",
	// 			None,
	// 		)
	// 		.unwrap()
	// 		.value,
	// 	),
	// );

	// AddTranche 2 of B
	let tranche_id = {
		let tranche_id = utils::pool_b_tranche_2_id::<T>();
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_B,
			utils::pool_b_tranche_2_id::<T>(),
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

		tranche_id
	};
	evm.call(
		Keyring::Alice,
		Default::default(),
		names::POOL_MANAGER,
		"deployTranche",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id)),
		]),
	)
	.unwrap();
	evm.register(
		names::POOL_B_T_2,
		contracts::TRANCHE_TOKEN,
		Decoder::<H160>::decode(
			&evm.view(
				Keyring::Alice,
				names::POOL_MANAGER,
				"getTranche",
				Some(&[
					Token::Uint(POOL_B.into()),
					Token::FixedBytes(tranche_id.to_vec()),
				]),
			)
			.unwrap()
			.value,
		),
	);
	// FIXME: Fails
	// evm.register(
	// 	names::RM_POOL_B_T_2,
	// 	contracts::RESTRICTION_MANAGER,
	// 	Decoder::<H160>::decode(
	// 		&evm.view(
	// 			Keyring::Alice,
	// 			names::POOL_B_T_2,
	// 			"restrictionManager",
	// 			None,
	// 		)
	// 		.unwrap()
	// 		.value,
	// 	),
	// );

	// AddTranche 1 of C
	let tranche_id = {
		let tranche_id = utils::pool_c_tranche_1_id::<T>();
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_C,
			tranche_id,
			Domain::EVM(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

		tranche_id
	};
	evm.call(
		Keyring::Alice,
		Default::default(),
		names::POOL_MANAGER,
		"deployTranche",
		Some(&[
			Token::Uint(Uint::from(POOL_C)),
			Token::FixedBytes(FixedBytes::from(tranche_id)),
		]),
	)
	.unwrap();
	evm.register(
		names::POOL_C_T_1,
		contracts::TRANCHE_TOKEN,
		Decoder::<H160>::decode(
			&evm.view(
				Keyring::Alice,
				names::POOL_MANAGER,
				"getTranche",
				Some(&[
					Token::Uint(POOL_C.into()),
					Token::FixedBytes(tranche_id.to_vec()),
				]),
			)
			.unwrap()
			.value,
		),
	);
	// FIXME: Fails
	// evm.register(
	// 	names::RM_POOL_C_T_1,
	// 	contracts::RESTRICTION_MANAGER,
	// 	Decoder::<H160>::decode(
	// 		&evm.view(
	// 			Keyring::Alice,
	// 			names::POOL_C_T_1,
	// 			"restrictionManager",
	// 			None,
	// 		)
	// 		.unwrap()
	// 		.value,
	// 	),
	// );
}

/// Create two pools A, B and send `add_pool` message to EVM
/// * Pool A with 1 tranche
/// * Pool B with 2 tranches
pub fn setup_pools<T: Runtime>(_evm: &mut impl EvmEnv<T>) {
	crate::utils::pool::create_one_tranched::<T>(Keyring::Admin.into(), POOL_A, LocalUSDC.id());

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
		OriginFor::<T>::signed(Keyring::Admin.into()),
		POOL_A,
		Domain::EVM(EVM_DOMAIN_CHAIN_ID)
	));

	utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

	crate::utils::pool::create_two_tranched::<T>(Keyring::Admin.into(), POOL_B, LocalUSDC.id());

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
		OriginFor::<T>::signed(Keyring::Admin.into()),
		POOL_B,
		Domain::EVM(EVM_DOMAIN_CHAIN_ID)
	));

	crate::utils::pool::create_one_tranched::<T>(Keyring::Admin.into(), POOL_C, USDC.id());

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
		OriginFor::<T>::signed(Keyring::Admin.into()),
		POOL_C,
		Domain::EVM(EVM_DOMAIN_CHAIN_ID)
	));

	utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
}

/// Create 3x ERC-20 currencies as Stablecoins on EVM, register them on
/// Centrifuge Chain and trigger `AddAsset` from Centrifuge Chain to EVM
pub fn setup_currencies<T: Runtime>(evm: &mut impl EvmEnv<T>) {
	// EVM: Create currencies
	// NOTE: Called by Keyring::Admin, as admin controls all in this setup
	evm.deploy(
		"ERC20",
		names::USDC,
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(6))]),
	);
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::USDC,
		"file",
		Some(&[
			Token::FixedBytes("name".as_bytes().to_vec()),
			Token::String("USD Coin".to_string()),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::USDC,
		"file",
		Some(&[
			Token::FixedBytes("symbol".as_bytes().to_vec()),
			Token::String("USDC".to_string()),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::USDC,
		"mint",
		Some(&[
			Token::Address(Keyring::Alice.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::USDC,
		"mint",
		Some(&[
			Token::Address(Keyring::Bob.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::USDC,
		"mint",
		Some(&[
			Token::Address(Keyring::Charlie.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();

	evm.deploy(
		"ERC20",
		names::FRAX,
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(18))]),
	);
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::FRAX,
		"file",
		Some(&[
			Token::FixedBytes("name".as_bytes().to_vec()),
			Token::String("Frax Coin".to_string()),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::FRAX,
		"file",
		Some(&[
			Token::FixedBytes("symbol".as_bytes().to_vec()),
			Token::String("FRAX".to_string()),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::FRAX,
		"mint",
		Some(&[
			Token::Address(Keyring::Alice.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::FRAX,
		"mint",
		Some(&[
			Token::Address(Keyring::Bob.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::FRAX,
		"mint",
		Some(&[
			Token::Address(Keyring::Charlie.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();

	evm.deploy(
		"ERC20",
		names::DAI,
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(18))]),
	);
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::DAI,
		"file",
		Some(&[
			Token::FixedBytes("name".as_bytes().to_vec()),
			Token::String("Dai Coin".to_string()),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::DAI,
		"file",
		Some(&[
			Token::FixedBytes("symbol".as_bytes().to_vec()),
			Token::String("DAI".to_string()),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::DAI,
		"mint",
		Some(&[
			Token::Address(Keyring::Alice.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::DAI,
		"mint",
		Some(&[
			Token::Address(Keyring::Bob.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();
	evm.call(
		Keyring::Admin,
		Default::default(),
		names::DAI,
		"mint",
		Some(&[
			Token::Address(Keyring::Charlie.into()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();

	// Centrifuge Chain: Register currencies and trigger `AddAsset`
	register_currency::<T>(USDC, |meta| {
		meta.location = Some(utils::lp_asset_location::<T>(
			evm.deployed(names::USDC).address(),
		));
	});

	register_currency::<T>(DAI, |meta| {
		meta.location = Some(utils::lp_asset_location::<T>(
			evm.deployed(names::DAI).address(),
		));
	});

	register_currency::<T>(FRAX, |meta| {
		meta.location = Some(utils::lp_asset_location::<T>(
			evm.deployed(names::FRAX).address(),
		));
	});

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
		OriginFor::<T>::signed(Keyring::Alice.into()),
		USDC.id()
	));
	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
		OriginFor::<T>::signed(Keyring::Alice.into()),
		DAI.id()
	));
	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_currency(
		OriginFor::<T>::signed(Keyring::Alice.into()),
		FRAX.id()
	));

	utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
}

/// Sets up investors for all tranches in Pool A and B on
/// Centrifuge Chain as well as EVM. Also mints default balance on both sides.
pub fn setup_investors<T: Runtime>(evm: &mut impl EvmEnv<T>) {
	default_investors().into_iter().for_each(|investor| {
		// Allow investor to locally invest
		crate::utils::pool::give_role::<T>(
			investor.into(),
			POOL_A,
			PoolRole::TrancheInvestor(pool_a_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		// Centrifuge Chain setup: Add permissions and dispatch LP message
		crate::utils::pool::give_role::<T>(
			AccountConverter::convert_evm_address(EVM_DOMAIN_CHAIN_ID, investor.into()),
			POOL_A,
			PoolRole::TrancheInvestor(pool_a_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			investor.as_origin(),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, investor.into()),
			SECONDS_PER_YEAR,
		));

		// Allow investor to locally invest
		crate::utils::pool::give_role::<T>(
			investor.into(),
			POOL_B,
			PoolRole::TrancheInvestor(pool_b_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		crate::utils::pool::give_role::<T>(
			AccountConverter::convert_evm_address(EVM_DOMAIN_CHAIN_ID, investor.into()),
			POOL_B,
			PoolRole::TrancheInvestor(pool_b_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			investor.as_origin(),
			POOL_B,
			pool_b_tranche_1_id::<T>(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, investor.into()),
			SECONDS_PER_YEAR,
		));

		// Allow investor to locally invest
		crate::utils::pool::give_role::<T>(
			investor.into(),
			POOL_B,
			PoolRole::TrancheInvestor(pool_b_tranche_2_id::<T>(), SECONDS_PER_YEAR),
		);
		crate::utils::pool::give_role::<T>(
			AccountConverter::convert_evm_address(EVM_DOMAIN_CHAIN_ID, investor.into()),
			POOL_B,
			PoolRole::TrancheInvestor(pool_b_tranche_2_id::<T>(), SECONDS_PER_YEAR),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			investor.as_origin(),
			POOL_B,
			pool_b_tranche_2_id::<T>(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, investor.into()),
			SECONDS_PER_YEAR,
		));

		// Allow investor to locally invest
		crate::utils::pool::give_role::<T>(
			investor.into(),
			POOL_C,
			PoolRole::TrancheInvestor(utils::pool_c_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		crate::utils::pool::give_role::<T>(
			AccountConverter::convert_evm_address(EVM_DOMAIN_CHAIN_ID, investor.into()),
			POOL_C,
			PoolRole::TrancheInvestor(utils::pool_c_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			investor.as_origin(),
			POOL_C,
			utils::pool_c_tranche_1_id::<T>(),
			DomainAddress::evm(EVM_DOMAIN_CHAIN_ID, investor.into()),
			SECONDS_PER_YEAR,
		));

		// Fund investor on EVM side
		evm.call(
			Keyring::Admin,
			Default::default(),
			names::USDC,
			"mint",
			Some(&[
				Token::Address(investor.into()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Admin,
			Default::default(),
			names::FRAX,
			"mint",
			Some(&[
				Token::Address(investor.into()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			Keyring::Admin,
			Default::default(),
			names::DAI,
			"mint",
			Some(&[
				Token::Address(investor.into()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();

		// Approve stable transfers on EVM side

		// Pool A - Tranche 1
		evm.call(
			investor,
			Default::default(),
			names::USDC,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_A_T_1_USDC).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			investor,
			Default::default(),
			names::DAI,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_A_T_1_DAI).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			investor,
			Default::default(),
			names::FRAX,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_A_T_1_FRAX).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();

		// Pool B - Tranche 1
		evm.call(
			investor,
			Default::default(),
			names::USDC,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_B_T_1_USDC).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			investor,
			Default::default(),
			names::DAI,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_B_T_1_DAI).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			investor,
			Default::default(),
			names::FRAX,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_B_T_1_FRAX).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();

		// Pool B - Tranche 2
		evm.call(
			investor,
			Default::default(),
			names::USDC,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_B_T_2_USDC).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			investor,
			Default::default(),
			names::DAI,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_B_T_2_DAI).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			investor,
			Default::default(),
			names::FRAX,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_B_T_2_FRAX).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();

		// Pool C - Tranche 1
		evm.call(
			investor,
			Default::default(),
			names::USDC,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_C_T_1_USDC).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			investor,
			Default::default(),
			names::DAI,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_C_T_1_DAI).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
		evm.call(
			investor,
			Default::default(),
			names::FRAX,
			"approve",
			Some(&[
				Token::Address(evm.deployed(names::POOL_C_T_1_FRAX).address()),
				Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
			]),
		)
		.unwrap();
	});

	utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
}

fn ensure_contracts_exist<T: Runtime>() {}
