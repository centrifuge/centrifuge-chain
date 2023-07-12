// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
use cfg_primitives::{currency_decimals, parachains, Balance, CFG, MILLI_CFG};
use cfg_types::{
	fee_keys::FeeKey,
	tokens::{AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata},
};
use cfg_utils::vec_to_fixed_array;
use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use runtime_common::account_conversion::AccountConverter;
use sc_service::{ChainType, Properties};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::UncheckedInto, sr25519, storage::Storage, Encode, Get, H160};
use sp_runtime::BuildStorage;
use xcm::{
	latest::MultiLocation,
	prelude::{GeneralIndex, GeneralKey, PalletInstance, Parachain, X2, X3},
};

use super::*;

mod axelar_v4_3_2_contracts;

pub type DevelopmentChainSpec = sc_service::GenericChainSpec<DevGenesisExt, Extensions>;

#[derive(Serialize, Deserialize)]
pub struct DevGenesisExt {
	runtime_gen: development_runtime::GenesisConfig,
	evm_code_gen: cfg_utils::evm::CodeDeployer<GetRoot>,
}

impl DevGenesisExt {
	fn new(
		runtime_gen: development_runtime::GenesisConfig,
		evm_code_gen: cfg_utils::evm::CodeDeployer<GetRoot>,
	) -> Self {
		Self {
			runtime_gen,
			evm_code_gen,
		}
	}
}

impl BuildStorage for DevGenesisExt {
	fn assimilate_storage(&self, storage: &mut Storage) -> Result<(), String> {
		self.runtime_gen.assimilate_storage(storage)?;

		sp_state_machine::BasicExternalities::execute_with_storage(storage, || {
			frame_support::traits::GenesisBuild::<development_runtime::Runtime>::build(
				&self.evm_code_gen,
			);
			Ok(())
		})
	}
}

// NOTE: Axelar Bridge contracts

#[derive(Debug)]
pub struct GetRoot;
impl Get<development_runtime::RuntimeOrigin> for GetRoot {
	fn get() -> development_runtime::RuntimeOrigin {
		frame_system::RawOrigin::Root.into()
	}
}

pub fn get_development_session_keys(
	keys: development_runtime::AuraId,
) -> development_runtime::SessionKeys {
	development_runtime::SessionKeys {
		aura: keys.clone(),
		block_rewards: keys,
	}
}

pub fn demo(para_id: ParaId) -> DevelopmentChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DEMO".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	DevelopmentChainSpec::from_genesis(
		"Demo Live",
		"demo_live",
		ChainType::Live,
		move || {
			DevGenesisExt::new(
				development_genesis(
					// kANEUrMbi9xC16AfL5vSGwfvBVRoRdfWoQ8abPiXi5etFxpdP
					hex!["e0c426785313bb7e712d66dce43ccb81a7eaef373784511fb508fff4b5df3305"].into(),
					vec![(
						// kAHJNhAragKRrAb9X8JxSNYoqPqv36TspSwdSuyMfxGKUmfdH
						hex!["068f3bd4ed27bb83da8fdebbb4deba6b3b3b83ff47c8abad11e5c48c74c20b11"]
							.into(),
						// kAKXFWse8rghi8mbAFB4RaVyZu6XZXq5i9wv7uYakZ3vQcxMR
						hex!["68d9baaa081802f8ec50d475b654810b158cdcb23e11c43815a6549f78f1b34f"]
							.unchecked_into(),
					)],
					demo_endowed_accounts(),
					vec![],
					Some(100000000 * CFG),
					para_id,
				),
				cfg_utils::evm::CodeDeployer::default(),
			)
		},
		vec![],
		None,
		None,
		None,
		Some(properties),
		development_extensions(para_id.into()),
	)
}

pub fn development(para_id: ParaId) -> DevelopmentChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DEVEL".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	let deployer_account: H160 = endowed_evm_accounts()
		.first()
		.expect("Need one evm account.")
		.0
		.into();

	DevelopmentChainSpec::from_genesis(
		"Dev Live",
		"devel_live",
		ChainType::Live,
		move || {
			DevGenesisExt::new(
				development_genesis(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					vec![(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<development_runtime::AuraId>("Alice"),
					)],
					endowed_accounts(),
					endowed_evm_accounts(),
					Some(10000000 * CFG),
					para_id,
				),
				cfg_utils::evm::CodeDeployer::new(vec![
					(
						deployer_account,
						axelar_v4_3_2_contracts::AXELAR_AUTH_WEIGHTED.to_vec(),
					),
					(
						deployer_account,
						axelar_v4_3_2_contracts::TOKEN_DEPLOYER.to_vec(),
					),
					(
						deployer_account,
						axelar_v4_3_2_contracts::AXELAR_GATEWAY.to_vec(),
					),
					(
						deployer_account,
						axelar_v4_3_2_contracts::AXELAR_GATEWAY_PROXY.to_vec(),
					),
				]),
			)
		},
		vec![],
		None,
		None,
		None,
		Some(properties),
		development_extensions(para_id.into()),
	)
}

pub fn development_local(para_id: ParaId) -> DevelopmentChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DEVEL".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	let deployer_account: H160 = endowed_evm_accounts()
		.first()
		.expect("Need one evm account.")
		.0
		.into();

	DevelopmentChainSpec::from_genesis(
		"Dev Local",
		"devel_local",
		ChainType::Local,
		move || {
			DevGenesisExt::new(
				development_genesis(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					vec![(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<development_runtime::AuraId>("Alice"),
					)],
					endowed_accounts(),
					endowed_evm_accounts(),
					Some(10000000 * CFG),
					para_id,
				),
				cfg_utils::evm::CodeDeployer::new(vec![
					(
						deployer_account,
						axelar_v4_3_2_contracts::AXELAR_AUTH_WEIGHTED.to_vec(),
					),
					(
						deployer_account,
						axelar_v4_3_2_contracts::TOKEN_DEPLOYER.to_vec(),
					),
					(
						deployer_account,
						axelar_v4_3_2_contracts::AXELAR_GATEWAY.to_vec(),
					),
					(
						deployer_account,
						axelar_v4_3_2_contracts::AXELAR_GATEWAY_PROXY.to_vec(),
					),
				]),
			)
		},
		vec![],
		None,
		None,
		None,
		Some(properties),
		development_extensions(para_id.into()),
	)
}

fn demo_endowed_accounts() -> Vec<cfg_primitives::AccountId> {
	vec![
		//kANEUrMbi9xC16AfL5vSGwfvBVRoRdfWoQ8abPiXi5etFxpdP
		hex!["e0c426785313bb7e712d66dce43ccb81a7eaef373784511fb508fff4b5df3305"].into(),
		// kAHJNhAragKRrAb9X8JxSNYoqPqv36TspSwdSuyMfxGKUmfdH
		hex!["068f3bd4ed27bb83da8fdebbb4deba6b3b3b83ff47c8abad11e5c48c74c20b11"].into(),
		// kAJ27MdBneY2U6QXvY3CmUE9btDmTvxSYfBd5qjw9U6oNZe2C
		hex!["2663e968d484dc12c488a5b74107c0c3b6bcf21a6672923b153e4b5a9170a878"].into(),
		// kAKq5N4wTcKU7qCCSzUqNcQQSMfPuN5k8tBafgoH9tpUgfVg2
		hex!["7671f8ee2c446ebd2b655ab5380b8004598d9663809cbb372f3de627a0e5eb32"].into(),
		// kAJkDfWBaUSoavcbWc7m5skLsd5APLgqfr8YfgKEcBctccxTv
		hex!["4681744964868d0f210b1161759958390a861b1733c65a6d04ac6b0ffe2f1e42"].into(),
		// kAKZvAs9YpXMbZLNqrbu4rnqWDPVDEVVsDc6ngKtemEbqmQSk
		hex!["6ae25829700ff7251861ac4a97235070b3e6e0883ce54ee53aa48400aa28d905"].into(),
		// kAMBhYMypx5LGfEwBKDg42mBmymXEvU8TRHwoDMyGhY74oMf8
		hex!["b268e5eee003859659258de82991ce0dc47db15c5b3d32bd050f8b02d350530e"].into(),
		// kANtu5pYcZ2TcutMAaeuxYgySzT1YH7y72h77rReLki24c33J
		hex!["fe110c5ece58c80fc7fb740b95776f9b640ae1c9f0842895a55d2e582e4e1076"].into(),
	]
}

/// The CurrencyId for the USDT asset on the development runtime
const DEV_USDT_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);
const DEV_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2);

fn development_genesis(
	root_key: development_runtime::AccountId,
	initial_authorities: Vec<(development_runtime::AccountId, development_runtime::AuraId)>,
	mut endowed_accounts: Vec<development_runtime::AccountId>,
	endowed_evm_accounts: Vec<([u8; 20], Option<u64>)>,
	total_issuance: Option<development_runtime::Balance>,
	id: ParaId,
) -> development_runtime::GenesisConfig {
	let chain_id: u32 = id.into();

	endowed_accounts.extend(endowed_evm_accounts.into_iter().map(|(addr, id)| {
		let chain_id = id.unwrap_or_else(|| chain_id.into());
		AccountConverter::<centrifuge_runtime::Runtime>::convert_evm_address(chain_id, addr)
	}));

	let num_endowed_accounts = endowed_accounts.len();
	let (balances, token_balances) = match total_issuance {
		Some(total_issuance) => {
			let balance_per_endowed = total_issuance
				.checked_div(num_endowed_accounts as development_runtime::Balance)
				.unwrap_or(0 as development_runtime::Balance);

			(
				// pallet_balances balances
				endowed_accounts
					.iter()
					.cloned()
					.map(|x| (x, balance_per_endowed))
					.collect(),
				// orml_tokens balances
				// bootstrap each endowed accounts with 1 million of each the foreign assets.
				endowed_accounts
					.iter()
					.cloned()
					.flat_map(|x| {
						// NOTE: We can only mint these foreign assets on development
						vec![
							// USDT is a 6-decimal asset, so 1 million + 6 zeros
							(x.clone(), DEV_USDT_CURRENCY_ID, 1_000_000_000_000),
							// AUSD is a 12-decimal asset, so 1 million + 12 zeros
							(x, DEV_AUSD_CURRENCY_ID, 1_000_000_000_000_000_000),
						]
					})
					.collect(),
			)
		}
		None => (vec![], vec![]),
	};
	let chain_id: u32 = id.into();

	development_runtime::GenesisConfig {
		system: development_runtime::SystemConfig {
			code: development_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: development_runtime::BalancesConfig { balances },
		orml_asset_registry: development_runtime::OrmlAssetRegistryConfig {
			assets: asset_registry_assets(),
			last_asset_id: Default::default(),
		},
		orml_tokens: development_runtime::OrmlTokensConfig {
			balances: token_balances,
		},
		elections: development_runtime::ElectionsConfig { members: vec![] },
		council: development_runtime::CouncilConfig {
			members: Default::default(),
			phantom: Default::default(),
		},
		fees: development_runtime::FeesConfig {
			initial_fees: vec![(
				// Anchoring state rent fee per day
				// pre-image: 0xdb4faa73ca6d2016e53c7156087c176b79b169c409b8a0063a07964f3187f9e9
				// hash   : 0x11da6d1f761ddf9bdb4c9d6e5303ebd41f61858d0a5647a1a7bfe089bf921be9
				FeeKey::AnchorsCommit,
				// Daily state rent, defined such that it will amount to 0.00259.. RAD
				// (2_590_000_000_000_040) over 3 years, which is the expected average anchor
				// duration. The other fee components for anchors amount to about 0.00041.. RAD
				// (410_000_000_000_000), such that the total anchor price for 3 years will be
				// 0.003.. RAD
				2_365_296_803_653,
			)],
		},
		vesting: Default::default(),
		sudo: development_runtime::SudoConfig {
			key: Some(root_key),
		},
		parachain_info: development_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: development_runtime::CollatorSelectionConfig {
			invulnerables: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect(),
			candidacy_bond: 1 * CFG,
			..Default::default()
		},
		collator_allowlist: Default::default(),
		session: development_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                        // account id
						acc,                                // validator id
						get_development_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		bridge: development_runtime::BridgeConfig {
			// Whitelist chains Ethereum - 0
			chains: vec![0],
			// Register resourceIDs
			resources: vec![
				// xCFG ResourceID to PalletBridge.transfer method (for incoming txs)
				(
					hex!["00000000000000000000000000000009e974040e705c10fb4de576d6cc261900"],
					hex!["50616c6c65744272696467652e7472616e73666572"].to_vec(),
				),
			],
			// Dev Alice - 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
			// Sample Endowed1 - 5GVimUaccBq1XbjZ99Zmm8aytG6HaPCjkZGKSHC1vgrsQsLQ
			relayers: vec![
				hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into(),
				hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"].into(),
			],
			threshold: 1,
		},
		aura_ext: Default::default(),
		aura: Default::default(),
		democracy: Default::default(),
		parachain_system: Default::default(),
		treasury: Default::default(),
		interest_accrual: Default::default(),
		block_rewards: development_runtime::BlockRewardsConfig {
			collators: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect(),
			collator_reward: 8_325 * MILLI_CFG,
			total_reward: 10_048 * CFG,
		},
		base_fee: Default::default(),
		evm_chain_id: development_runtime::EVMChainIdConfig {
			chain_id: chain_id.into(),
		},
		ethereum: Default::default(),
		evm: Default::default(),
		block_rewards_base: development_runtime::BlockRewardsBaseConfig {
			currency_id: CurrencyId::Native,
			amount: development_runtime::ExistentialDeposit::get(),
		},
		liquidity_rewards_base: development_runtime::LiquidityRewardsBaseConfig {
			currency_id: CurrencyId::Native,
			amount: development_runtime::ExistentialDeposit::get(),
		},
	}
}

fn asset_registry_assets() -> Vec<(CurrencyId, Vec<u8>)> {
	vec![
		(
			DEV_USDT_CURRENCY_ID,
			AssetMetadata::<Balance, CustomMetadata> {
				decimals: 6,
				name: b"Tether USD".to_vec(),
				symbol: b"USDT".to_vec(),
				existential_deposit: 0u128,
				location: Some(xcm::VersionedMultiLocation::V3(MultiLocation {
					parents: 1,
					interior: X3(
						Parachain(parachains::rococo::rocksmine::ID),
						PalletInstance(parachains::rococo::rocksmine::usdt::PALLET_INSTANCE),
						GeneralIndex(parachains::rococo::rocksmine::usdt::GENERAL_INDEX),
					),
				})),
				additional: CustomMetadata {
					mintable: false,
					permissioned: false,
					pool_currency: true,
					transferability: CrossChainTransferability::Xcm(Default::default()),
				},
			}
			.encode(),
		),
		(
			DEV_AUSD_CURRENCY_ID,
			AssetMetadata::<Balance, CustomMetadata> {
				decimals: 12,
				name: b"Acala USD".to_vec(),
				symbol: b"AUSD".to_vec(),
				existential_deposit: 0u128,
				location: Some(xcm::VersionedMultiLocation::V3(MultiLocation {
					parents: 1,
					interior: X2(
						Parachain(parachains::rococo::acala::ID),
						GeneralKey {
							length: parachains::rococo::acala::AUSD_KEY.to_vec().len() as u8,
							data: vec_to_fixed_array(parachains::rococo::acala::AUSD_KEY.to_vec()),
						},
					),
				})),
				additional: CustomMetadata {
					mintable: false,
					permissioned: false,
					pool_currency: true,
					transferability: CrossChainTransferability::Xcm(Default::default()),
				},
			}
			.encode(),
		),
	]
}
