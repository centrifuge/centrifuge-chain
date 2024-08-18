// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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

use super::*;
use crate::cases::lp::utils::pool_c_tranche_1_id;

pub fn setup_full<T: Runtime>() -> impl EnvEvmExtension<T> {
	setup::<T, _>(|evm| {
		setup_currencies(evm);
		setup_pools(evm);
		setup_tranches(evm);
		setup_investment_currencies(evm);
		setup_deploy_lps(evm);
		setup_investors(evm);
		setup_market_ratios::<T>();
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
		give_balance::<T>(T::Sender::get().account(), DEFAULT_BALANCE * CFG);

		// Register general local pool-currency
		register_currency::<T>(LocalUSDC, |_| {});

		// ------------------ EVM Side ----------------------- //
		setup_evm::deployer_script::<T>(evm);

		// PART: Deploy router (using the testing LocalAdapter here)
		//  * https://github.com/centrifuge/liquidity-pools/blob/e2c3ac92d1cea991e7e0d5f57be8658a46cbf1fe/script/Axelar.s.sol#L24
		//  * NEW: https://github.com/centrifuge/liquidity-pools/blob/b19bf62a3a49b8452999b9250dbd3229f60ee757/script/Axelar.s.sol#L19-L21
		evm.deploy(contracts::ADAPTER, names::ADAPTER, Keyring::Alice, None);

		setup_evm::endorse::<T>(evm);
		setup_evm::rely::<T>(evm);
		setup_evm::file::<T>(evm);
		setup_evm::local_router::<T>(evm);
		setup_evm::remove_deployer_access::<T>(evm);

		// ------------------ Substrate Side ----------------------- //
		// Create router
		let (base_fee, _) = <T as pallet_evm::Config>::FeeCalculator::min_gas_price();

		let axelar_evm_config = AxelarConfig {
			liquidity_pools_contract_address: evm.deployed(names::ADAPTER).address(),
			domain: DomainConfig::Evm(EvmConfig {
				chain_id: EVM_DOMAIN_CHAIN_ID,
				target_contract_address: evm.deployed(names::ADAPTER).address(),
				target_contract_hash: BlakeTwo256::hash_of(
					&evm.deployed(names::ADAPTER).deployed_bytecode,
				),
				fee_values: FeeValues {
					value: sp_core::U256::zero(),
					gas_limit: sp_core::U256::from(500_000),
					gas_price: sp_core::U256::from(base_fee),
				},
			}),
		};

		assert_ok!(pallet_axelar_router::Pallet::<T>::set_config(
			RawOrigin::Root.into(),
			EVM_DOMAIN_STR.as_bytes().to_vec().try_into().unwrap(),
			Box::new(axelar_evm_config)
		));

		assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::add_instance(
			RawOrigin::Root.into(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, H160(EVM_LP_INSTANCE))
		));

		assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::set_routers(
			RawOrigin::Root.into(),
			BoundedVec::try_from(vec![EVM_ROUTER_ID]).unwrap(),
		));

		assert_ok!(
			pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_hook_address(
				RawOrigin::Root.into(),
				Domain::Evm(EVM_DOMAIN_CHAIN_ID),
				LOCAL_RESTRICTION_MANAGER_ADDRESS.into(),
			)
		);

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
	utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>)
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
				contracts::LP,
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
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);

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

	// AddTranche 1 of B
	let tranche_id = {
		let tranche_id = utils::pool_b_tranche_1_id::<T>();
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_B,
			tranche_id,
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);

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

	// AddTranche 2 of B
	let tranche_id = {
		let tranche_id = utils::pool_b_tranche_2_id::<T>();
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_B,
			utils::pool_b_tranche_2_id::<T>(),
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);

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

	// AddTranche 1 of C
	let tranche_id = {
		let tranche_id = utils::pool_c_tranche_1_id::<T>();
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_tranche(
			OriginFor::<T>::signed(Keyring::Admin.into()),
			POOL_C,
			tranche_id,
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);

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
}

/// Create two pools A, B and send `add_pool` message to EVM
/// * Pool A with 1 tranche
/// * Pool B with 2 tranches
pub fn setup_pools<T: Runtime>(_evm: &mut impl EvmEnv<T>) {
	crate::utils::pool::create_one_tranched::<T>(Keyring::Admin.into(), POOL_A, LocalUSDC.id());

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
		OriginFor::<T>::signed(Keyring::Admin.into()),
		POOL_A,
		Domain::Evm(EVM_DOMAIN_CHAIN_ID)
	));

	utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);

	crate::utils::pool::create_two_tranched::<T>(Keyring::Admin.into(), POOL_B, LocalUSDC.id());

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
		OriginFor::<T>::signed(Keyring::Admin.into()),
		POOL_B,
		Domain::Evm(EVM_DOMAIN_CHAIN_ID)
	));

	crate::utils::pool::create_one_tranched::<T>(Keyring::Admin.into(), POOL_C, USDC.id());

	assert_ok!(pallet_liquidity_pools::Pallet::<T>::add_pool(
		OriginFor::<T>::signed(Keyring::Admin.into()),
		POOL_C,
		Domain::Evm(EVM_DOMAIN_CHAIN_ID)
	));

	utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
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
			Token::Address(Keyring::Alice.in_eth()),
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
			Token::Address(Keyring::Bob.in_eth()),
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
			Token::Address(Keyring::Charlie.in_eth()),
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
			Token::Address(Keyring::Alice.in_eth()),
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
			Token::Address(Keyring::Bob.in_eth()),
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
			Token::Address(Keyring::Charlie.in_eth()),
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
			Token::Address(Keyring::Alice.in_eth()),
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
			Token::Address(Keyring::Bob.in_eth()),
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
			Token::Address(Keyring::Charlie.in_eth()),
			Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
		]),
	)
	.unwrap();

	// Centrifuge Chain: Register currencies and trigger `AddAsset`
	register_currency::<T>(USDC, |meta| {
		meta.location = Some(utils::lp_asset_location::<T>(
			evm.deployed(names::USDC).address(),
		));
		meta.additional.local_representation = Some(LocalUSDC.id().try_into().unwrap())
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

	utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
}

/// Sets up investors for all tranches in Pool A and B on
/// Centrifuge Chain as well as EVM. Also mints default balance on both sides.
pub fn setup_investors<T: Runtime>(evm: &mut impl EvmEnv<T>) {
	default_investors().into_iter().for_each(|investor| {
		// POOL A - Tranche 1/1
		// Allow investor to locally invest
		crate::utils::pool::give_role::<T>(
			investor.into(),
			POOL_A,
			PoolRole::TrancheInvestor(pool_a_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		// Centrifuge Chain setup: Add permissions and dispatch LP message
		crate::utils::pool::give_role::<T>(
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()).account(),
			POOL_A,
			PoolRole::TrancheInvestor(pool_a_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			investor.as_origin(),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()),
			SECONDS_PER_YEAR,
		));

		// POOL B - Tranche 1/2
		// Allow investor to locally invest
		crate::utils::pool::give_role::<T>(
			investor.into(),
			POOL_B,
			PoolRole::TrancheInvestor(pool_b_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		crate::utils::pool::give_role::<T>(
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()).account(),
			POOL_B,
			PoolRole::TrancheInvestor(pool_b_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			investor.as_origin(),
			POOL_B,
			pool_b_tranche_1_id::<T>(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()),
			SECONDS_PER_YEAR,
		));

		// POOL B - Tranche 2/2
		// Allow investor to locally invest
		crate::utils::pool::give_role::<T>(
			investor.into(),
			POOL_B,
			PoolRole::TrancheInvestor(pool_b_tranche_2_id::<T>(), SECONDS_PER_YEAR),
		);
		crate::utils::pool::give_role::<T>(
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()).account(),
			POOL_B,
			PoolRole::TrancheInvestor(pool_b_tranche_2_id::<T>(), SECONDS_PER_YEAR),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			investor.as_origin(),
			POOL_B,
			pool_b_tranche_2_id::<T>(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()),
			SECONDS_PER_YEAR,
		));

		// POOL C - Tranche 1/1
		// Allow investor to locally invest
		crate::utils::pool::give_role::<T>(
			investor.into(),
			POOL_C,
			PoolRole::TrancheInvestor(utils::pool_c_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		crate::utils::pool::give_role::<T>(
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()).account(),
			POOL_C,
			PoolRole::TrancheInvestor(utils::pool_c_tranche_1_id::<T>(), SECONDS_PER_YEAR),
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_member(
			investor.as_origin(),
			POOL_C,
			utils::pool_c_tranche_1_id::<T>(),
			DomainAddress::Evm(EVM_DOMAIN_CHAIN_ID, investor.in_eth()),
			SECONDS_PER_YEAR,
		));

		for currency in [names::USDC, names::FRAX, names::DAI] {
			// Fund investor on EVM side
			evm.call(
				Keyring::Admin,
				Default::default(),
				currency,
				"mint",
				Some(&[
					Token::Address(investor.in_eth()),
					Token::Uint(U256::from(DEFAULT_BALANCE * DECIMALS_6)),
				]),
			)
			.unwrap();
			assert_eq!(
				DEFAULT_BALANCE * DECIMALS_6,
				Decoder::<Balance>::decode(
					&evm.view(
						investor,
						currency,
						"balanceOf",
						Some(&[Token::Address(investor.in_eth())])
					)
					.unwrap()
					.value
				)
			)
		}

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

	utils::process_gateway_message::<T>(utils::verify_gateway_message_success::<T>);
}

/// Adds bidirectional trading pairs with conversion ratio one.
pub fn enable_symmetric_trading_pair<T: Runtime>(
	currency_1: CurrencyId,
	currency_2: CurrencyId,
	pool_admin: AccountId,
	pool_id: PoolId,
) {
	assert_ok!(pallet_order_book::Pallet::<T>::set_market_feeder(
		<T as frame_system::Config>::RuntimeOrigin::root(),
		Feeder::root(),
	));
	crate::utils::oracle::update_feeders::<T>(pool_admin, pool_id, [Feeder::root()]);
	crate::utils::oracle::feed_from_root::<T>(
		OracleKey::ConversionRatio(currency_1, currency_2),
		Ratio::one(),
	);
	crate::utils::oracle::feed_from_root::<T>(
		OracleKey::ConversionRatio(currency_2, currency_1),
		Ratio::one(),
	);
}

/// Setup symmetric trading pairs and market ratios
///
/// NOTE: Necessary in order to be able to invest
pub fn setup_market_ratios<T: Runtime>() {
	for currency_id in [USDC.id(), FRAX.id(), DAI.id()] {
		enable_symmetric_trading_pair::<T>(
			pallet_foreign_investments::pool_currency_of::<T>((POOL_A, pool_a_tranche_1_id::<T>()))
				.unwrap(),
			currency_id,
			Keyring::Admin.id(),
			POOL_A,
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_token_price(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			POOL_A,
			pool_a_tranche_1_id::<T>(),
			currency_id,
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		enable_symmetric_trading_pair::<T>(
			pallet_foreign_investments::pool_currency_of::<T>((POOL_B, pool_b_tranche_1_id::<T>()))
				.unwrap(),
			currency_id,
			Keyring::Admin.id(),
			POOL_B,
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_token_price(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			POOL_B,
			pool_b_tranche_1_id::<T>(),
			currency_id,
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));
		enable_symmetric_trading_pair::<T>(
			pallet_foreign_investments::pool_currency_of::<T>((POOL_B, pool_b_tranche_2_id::<T>()))
				.unwrap(),
			currency_id,
			Keyring::Admin.id(),
			POOL_B,
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_token_price(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			POOL_B,
			pool_b_tranche_2_id::<T>(),
			currency_id,
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));

		enable_symmetric_trading_pair::<T>(
			pallet_foreign_investments::pool_currency_of::<T>((POOL_C, pool_c_tranche_1_id::<T>()))
				.unwrap(),
			currency_id,
			Keyring::Admin.id(),
			POOL_C,
		);
		assert_ok!(pallet_liquidity_pools::Pallet::<T>::update_token_price(
			OriginFor::<T>::signed(Keyring::Alice.into()),
			POOL_C,
			pool_c_tranche_1_id::<T>(),
			currency_id,
			Domain::Evm(EVM_DOMAIN_CHAIN_ID)
		));
	}
}
