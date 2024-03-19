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
use cfg_primitives::{Balance, PoolId, CFG};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata, LocalAssetId},
};
use ethabi::{ethereum_types::U256, FixedBytes, Token, Uint};
use frame_support::{
	assert_ok, dispatch::RawOrigin, pallet_prelude::ConstU32, traits::OriginTrait, BoundedVec,
};
use frame_system::pallet_prelude::OriginFor;
use hex_literal::hex;
use liquidity_pools_gateway_routers::{
	AxelarEVMRouter, DomainRouter, EVMDomain, EVMRouter, FeeValues, MAX_AXELAR_EVM_CHAIN_SIZE,
};
use pallet_evm::FeeCalculator;
use sp_core::Get;
use sp_runtime::traits::{BlakeTwo256, Hash};

use crate::{
	generic::{
		cases::lp::utils::Decoder,
		config::Runtime,
		env::{Blocks, Env, EnvEvmExtension, EvmEnv},
		envs::runtime_env::RuntimeEnv,
		utils::{
			currency::{register_currency, CurrencyInfo},
			genesis,
			genesis::Genesis,
			give_balance,
		},
	},
	utils::accounts::Keyring,
};

pub mod investments;
pub mod pool_management;
pub mod utils {
	use std::cmp::min;

	use cfg_primitives::{Balance, TrancheId};
	use ethabi::ethereum_types::{H160, H256, U256};
	use frame_support::traits::{OriginTrait, PalletInfo};
	use frame_system::pallet_prelude::OriginFor;
	use sp_core::{ByteArray, Get};
	use xcm::{
		v3::{
			Junction::{AccountKey20, GlobalConsensus, PalletInstance},
			Junctions::X3,
			NetworkId,
		},
		VersionedMultiLocation,
	};

	use crate::{
		generic::{
			cases::lp::{EVM_DOMAIN_CHAIN_ID, POOL_A, POOL_B},
			config::Runtime,
			utils::{evm::receipt_ok, last_event, pool::get_tranche_ids},
		},
		utils::accounts::Keyring,
	};

	pub fn lp_asset_location<T: Runtime>(address: H160) -> VersionedMultiLocation {
		X3(
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
			},
		)
			.into()
	}

	pub fn pool_a_tranche_id<T: Runtime>() -> TrancheId {
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

	pub fn verify_outbound_success<T: Runtime>() {
		assert!(matches!(
			last_event::<T, pallet_liquidity_pools_gateway::Event::<T>>(),
			pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionSuccess { .. }
		));
	}

	pub fn process_outbound<T: Runtime>(verifier: impl Fn()) {
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

				verifier();
			});
	}

	pub fn to_fixed_array<const S: usize>(src: &[u8]) -> [u8; S] {
		let mut dest = [0; S];
		let len = min(src.len(), S);
		dest[..len].copy_from_slice(&src[..len]);

		dest
	}

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

	impl<T: Input> Decoder<sp_core::H160> for T {
		fn decode(&self) -> sp_core::H160 {
			assert!(self.input().len() == 32);

			sp_core::H160::from(to_fixed_array(&self.input()[12..]))
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

pub const POOL_A: PoolId = 1;
pub const POOL_B: PoolId = 2;
pub const DEFAULT_BALANCE: Balance = 1_000_000;
const DECIMALS_6: Balance = 1_000_000;
const DECIMALS_18: Balance = 1_000_000_000_000_000_000;
const LOCAL_ASSET_ID: LocalAssetId = LocalAssetId(1);

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
		// TODO: Needs setup investors too here
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

	env.pass(Blocks::ByNumber(1));

	env.state_mut(|evm| {
		// ------------------ EVM Side ---------------------------- //
		evm.load_contracts();
		evm.deploy("LocalRouterScript", "lp_deploy", Keyring::Alice, None);
		evm.call(Keyring::Alice, Default::default(), "lp_deploy", "run", None)
			.unwrap();
		evm.register("router", "LocalRouter", None);
		evm.register("pool_manager", "PoolManager", None);

		// ------------------ Substrate Side ----------------------- //
		// Create router
		// Fund gateway sender
		give_balance::<T>(
			<T as pallet_liquidity_pools_gateway::Config>::Sender::get(),
			DEFAULT_BALANCE * CFG,
		);

		// Register general local pool-currency
		register_currency::<T>(LocalUSDC, |_| {});

		let (base_fee, _) = <T as pallet_evm::Config>::FeeCalculator::min_gas_price();

		let evm_domain = EVMDomain {
			target_contract_address: sp_core::H160::from(evm.deployed("router").address().0),
			target_contract_hash: BlakeTwo256::hash_of(&evm.deployed("router").deployed_bytecode),
			fee_values: FeeValues {
				value: sp_core::U256::zero(),
				gas_limit: sp_core::U256::from(500_000),
				gas_price: sp_core::U256::from(base_fee),
			},
		};

		let axelar_evm_router = AxelarEVMRouter::<T>::new(
			EVMRouter::new(evm_domain),
			BoundedVec::<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>::try_from(
				EVM_DOMAIN_STR.as_bytes().to_vec(),
			)
			.unwrap(),
			sp_core::H160::from(evm.deployed("router").address().0),
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
			sp_core::H160::from(evm.deployed("router").address().0)
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
		for pool in [POOL_A, POOL_B] {
			assert_ok!(
				pallet_liquidity_pools::Pallet::<T>::allow_investment_currency(
					OriginFor::<T>::signed(Keyring::Admin.into()),
					pool,
					currency,
				),
			);
			utils::process_outbound::<T>(utils::verify_outbound_success::<T>)
		}
	}
}

/// Deploys both Liquidity Pools for USDC, DAI and FRAX by calling
/// `DeployLiquidityPool` for each possible triplet of pool, tranche and
/// investment currency id.
///
/// NOTE: EVM Side
pub fn setup_deploy_lps<T: Runtime>(evm: &mut impl EvmEnv<T>) {
	let (tranche_id_a, tranche_id_b_1, tranche_id_b_2) = (
		utils::pool_a_tranche_id::<T>(),
		utils::pool_b_tranche_1_id::<T>(),
		utils::pool_b_tranche_2_id::<T>(),
	);

	// POOL_A - TRANCHE 1
	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_A)),
			Token::FixedBytes(FixedBytes::from(tranche_id_a)),
			Token::Address(evm.deployed("usdc").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_a_tranche_1_usdc",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::FixedBytes(FixedBytes::from(tranche_id_a)),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);

	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_A)),
			Token::FixedBytes(FixedBytes::from(tranche_id_a)),
			Token::Address(evm.deployed("frax").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_a_tranche_1_frax",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::FixedBytes(FixedBytes::from(tranche_id_a)),
					Token::Address(evm.deployed("frax").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);

	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_A)),
			Token::FixedBytes(FixedBytes::from(tranche_id_a)),
			Token::Address(evm.deployed("dai").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_a_tranche_1_dai",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_A)),
					Token::FixedBytes(FixedBytes::from(tranche_id_a)),
					Token::Address(evm.deployed("dai").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);

	// POOL_B - TRANCHE 1
	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id_b_1)),
			Token::Address(evm.deployed("usdc").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_b_tranche_1_usdc",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_B)),
					Token::FixedBytes(FixedBytes::from(tranche_id_b_1)),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);

	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id_b_1)),
			Token::Address(evm.deployed("frax").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_b_tranche_1_frax",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_B)),
					Token::FixedBytes(FixedBytes::from(tranche_id_b_1)),
					Token::Address(evm.deployed("frax").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);

	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id_b_1)),
			Token::Address(evm.deployed("dai").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_b_tranche_1_dai",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_B)),
					Token::FixedBytes(FixedBytes::from(tranche_id_b_1)),
					Token::Address(evm.deployed("dai").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);

	// POOL_B - TRANCHE 2
	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id_b_2)),
			Token::Address(evm.deployed("usdc").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_b_tranche_2_usdc",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_B)),
					Token::FixedBytes(FixedBytes::from(tranche_id_b_2)),
					Token::Address(evm.deployed("usdc").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);

	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id_b_2)),
			Token::Address(evm.deployed("frax").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_b_tranche_2_frax",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_B)),
					Token::FixedBytes(FixedBytes::from(tranche_id_b_2)),
					Token::Address(evm.deployed("frax").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);

	evm.call(
		Keyring::Alice,
		Default::default(),
		"pool_manager",
		"deployLiquidityPool",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id_b_2)),
			Token::Address(evm.deployed("dai").address()),
		]),
	)
	.unwrap();

	evm.register(
		"lp_pool_b_tranche_2_dai",
		"LiquidityPool",
		Some(Decoder::<sp_core::H160>::decode(
			&evm.view(
				Keyring::Alice,
				"pool_manager",
				"getLiquidityPool",
				Some(&[
					Token::Uint(Uint::from(POOL_B)),
					Token::FixedBytes(FixedBytes::from(tranche_id_b_2)),
					Token::Address(evm.deployed("dai").address()),
				]),
			)
			.unwrap()
			.value,
		)),
	);
}

/// Initiates tranches on EVM via `DeployTranche` contract and then sends
/// `add_tranche(pool, tranche_id)` messages for a total of three tranches of
/// pool A and B.
pub fn setup_tranches<T: Runtime>(evm: &mut impl EvmEnv<T>) {
	// AddTranche 1 of A
	let tranche_id = {
		let tranche_id = utils::pool_a_tranche_id::<T>();
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
		"pool_manager",
		"deployTranche",
		Some(&[
			Token::Uint(Uint::from(POOL_A)),
			Token::FixedBytes(FixedBytes::from(tranche_id)),
		]),
	)
	.unwrap();

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
		"pool_manager",
		"deployTranche",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id)),
		]),
	)
	.unwrap();

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
		"pool_manager",
		"deployTranche",
		Some(&[
			Token::Uint(Uint::from(POOL_B)),
			Token::FixedBytes(FixedBytes::from(tranche_id)),
		]),
	)
	.unwrap();
}

/// Create two pools A, B and send `add_pool` message to EVM
/// * Pool A with 1 tranche
/// * Pool B with 2 tranches
pub fn setup_pools<T: Runtime>(_evm: &mut impl EvmEnv<T>) {
	crate::generic::utils::pool::create_one_tranched::<T>(
		Keyring::Admin.into(),
		POOL_A,
		LocalUSDC.id(),
	);

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
		OriginFor::<T>::signed(Keyring::Admin.into()),
		POOL_A,
		Domain::EVM(EVM_DOMAIN_CHAIN_ID)
	));

	utils::process_outbound::<T>(utils::verify_outbound_success::<T>);

	crate::generic::utils::pool::create_two_tranched::<T>(
		Keyring::Admin.into(),
		POOL_B,
		LocalUSDC.id(),
	);

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
		OriginFor::<T>::signed(Keyring::Admin.into()),
		POOL_B,
		Domain::EVM(EVM_DOMAIN_CHAIN_ID)
	));

	utils::process_outbound::<T>(utils::verify_outbound_success::<T>);
}

/// Create 3x ERC-20 currencies as Stablecoins on EVM, register them on
/// Centrifuge Chain and trigger `AddCurrency` from Centrifuge Chain to EVM
pub fn setup_currencies<T: Runtime>(evm: &mut impl EvmEnv<T>) {
	// EVM: Create currencies
	// NOTE: Called by Keyring::Admin, as admin controls all in this setup
	evm.deploy(
		"ERC20",
		"usdc",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(6))]),
	);
	evm.call(
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
	evm.call(
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
	evm.call(
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
	evm.call(
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
	evm.call(
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

	evm.deploy(
		"ERC20",
		"frax",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(18))]),
	);
	evm.call(
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
	evm.call(
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
	evm.call(
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
	evm.call(
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
	evm.call(
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

	evm.deploy(
		"ERC20",
		"dai",
		Keyring::Admin,
		Some(&[Token::Uint(Uint::from(18))]),
	);
	evm.call(
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
	evm.call(
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
	evm.call(
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
	evm.call(
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
	evm.call(
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

	// Centrifuge Chain: Register currencies and trigger `AddCurrency`
	register_currency::<T>(USDC, |meta| {
		meta.location = Some(utils::lp_asset_location::<T>(
			evm.deployed("usdc").address(),
		));
	});

	register_currency::<T>(DAI, |meta| {
		meta.location = Some(utils::lp_asset_location::<T>(evm.deployed("dai").address()));
	});

	register_currency::<T>(FRAX, |meta| {
		meta.location = Some(utils::lp_asset_location::<T>(
			evm.deployed("frax").address(),
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