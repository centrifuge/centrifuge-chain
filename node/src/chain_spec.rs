// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

// This missing impl comes from the Substrate ChainSpecGroup derive macro
//
// That macro does not forward deny/allow directives to its internal
// struct, so there is no way to specifically target the output of
// that macro for an allow. Unfortunately, we need to allow this at
// module level.
#![allow(clippy::derive_partial_eq_without_eq)]

use std::collections::BTreeMap;
use std::sync::Arc;

use altair_runtime::constants::currency::{AIR, MILLI_AIR};
use cfg_primitives::{
	currency_decimals, parachains, AccountId, AuraId, Balance, BlockNumber, CFG, MILLI_CFG,
	SAFE_XCM_VERSION,
};
use cfg_types::{
	fee_keys::FeeKey,
	tokens::{usdc, AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata},
};
use cfg_utils::vec_to_fixed_array;
use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use runtime_common::{account_conversion::AccountConverter, evm::precompile::H160Addresses};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::{ChainType, Properties};
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Encode, Pair, Public, H160};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	FixedPointNumber,
};
use staging_xcm::v4::Junctions::{X2, X3};
use staging_xcm::{
	latest::{Location, NetworkId},
	prelude::{AccountKey20, GeneralIndex, GeneralKey, GlobalConsensus, PalletInstance, Parachain},
};

/// Specialized `ChainSpec` instances for our runtimes.
pub type AltairChainSpec =
	sc_service::GenericChainSpec<altair_runtime::RuntimeGenesisConfig, Extensions>;
pub type CentrifugeChainSpec =
	sc_service::GenericChainSpec<centrifuge_runtime::RuntimeGenesisConfig, Extensions>;
pub type DevelopmentChainSpec =
	sc_service::GenericChainSpec<development_runtime::RuntimeGenesisConfig, Extensions>;

use altair_runtime::AltairPrecompiles;
use centrifuge_runtime::CentrifugePrecompiles;
use cfg_types::fixed_point::Rate;
use development_runtime::DevelopmentPrecompiles;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{seed}"), None)
		.expect("static values are valid; qed")
		.public()
}

/// The extensions for the `ChainSpec`.
#[derive(
	Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension,
)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
	/// The first block which contains EVM logs
	pub first_evm_block: BlockNumber,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Extensions> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

/// Provides non-production extension for the given parachain id by defaulting
/// to "rococo-local" as relay chain.
fn development_extensions(para_id: u32) -> Extensions {
	Extensions {
		para_id,
		relay_chain: "rococo-local".into(),
		first_evm_block: 1,
	}
}

pub fn get_altair_session_keys(keys: AuraId) -> altair_runtime::SessionKeys {
	altair_runtime::SessionKeys {
		aura: keys.clone(),
		block_rewards: keys,
	}
}

pub fn get_centrifuge_session_keys(keys: AuraId) -> centrifuge_runtime::SessionKeys {
	centrifuge_runtime::SessionKeys {
		aura: keys.clone(),
		block_rewards: keys,
	}
}

pub fn get_development_session_keys(keys: AuraId) -> development_runtime::SessionKeys {
	development_runtime::SessionKeys {
		aura: keys.clone(),
		block_rewards: keys,
	}
}

type AccountPublic = <cfg_primitives::Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

pub fn centrifuge_config() -> CentrifugeChainSpec {
	CentrifugeChainSpec::from_json_bytes(
		&include_bytes!("../res/genesis/centrifuge-genesis-spec-raw.json")[..],
	)
	.unwrap()
}

pub fn centrifuge_local(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DCFG".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	CentrifugeChainSpec::builder(
		centrifuge_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
		development_extensions(para_id.into()),
	)
	.with_name("Centrifuge Local")
	.with_id("centrifuge_local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(centrifuge_genesis(
		vec![(
			get_account_id_from_seed::<sr25519::Public>("Alice"),
			get_from_seed::<AuraId>("Alice"),
		)],
		endowed_accounts(),
		endowed_evm_accounts(),
		Some(100000000 * CFG),
		para_id,
		council_members_bootstrap(),
	))
	.with_properties(properties)
	.build()
}

pub fn catalyst_config() -> CentrifugeChainSpec {
	CentrifugeChainSpec::from_json_bytes(&include_bytes!("../res/catalyst-spec-raw.json")[..])
		.unwrap()
}

pub fn altair_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(
		&include_bytes!("../res/genesis/altair-genesis-spec-raw.json")[..],
	)
	.unwrap()
}

pub fn demo_config() -> DevelopmentChainSpec {
	DevelopmentChainSpec::from_json_bytes(&include_bytes!("../res/demo-spec-raw.json")[..]).unwrap()
}

pub fn altair_local(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DAIR".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	AltairChainSpec::builder(
		altair_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
		development_extensions(para_id.into()),
	)
	.with_name("Altair Local")
	.with_id("altair_local")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(altair_genesis(
		vec![(
			get_account_id_from_seed::<sr25519::Public>("Alice"),
			get_from_seed::<AuraId>("Alice"),
		)],
		endowed_accounts(),
		endowed_evm_accounts(),
		Some(100000000 * AIR),
		para_id,
		council_members_bootstrap(),
	))
	.with_properties(properties)
	.build()
}

pub fn development(para_id: ParaId) -> DevelopmentChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DEVEL".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	DevelopmentChainSpec::builder(
		development_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
		development_extensions(para_id.into()),
	)
	.with_name("Dev Live")
	.with_id("devel_live")
	.with_chain_type(ChainType::Live)
	.with_genesis_config_patch(development_genesis(
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		vec![(
			get_account_id_from_seed::<sr25519::Public>("Alice"),
			get_from_seed::<AuraId>("Alice"),
		)],
		endowed_accounts(),
		endowed_evm_accounts(),
		Some(10000000 * CFG),
		para_id,
	))
	.with_properties(properties)
	.build()
}

fn endowed_accounts() -> Vec<AccountId> {
	vec![
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		get_account_id_from_seed::<sr25519::Public>("Bob"),
		get_account_id_from_seed::<sr25519::Public>("Charlie"),
		get_account_id_from_seed::<sr25519::Public>("Dave"),
		get_account_id_from_seed::<sr25519::Public>("Eve"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie"),
		get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
		get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
		get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
		get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
		get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
	]
}

fn endowed_evm_accounts() -> Vec<([u8; 20], Option<u64>)> {
	vec![(
		// Private key 0x4529cc809780dcc4bf85d99e55a757bc8fb3262d81fae92a759ec9056aca32b7
		hex!["7F429e2e38BDeFa7a2E797e3BEB374a3955746a4"],
		None,
	)]
}

fn council_members_bootstrap() -> Vec<AccountId> {
	endowed_accounts().into_iter().take(4).collect::<Vec<_>>()
}

fn centrifuge_genesis(
	initial_authorities: Vec<(AccountId, AuraId)>,
	mut endowed_accounts: Vec<AccountId>,
	endowed_evm_accounts: Vec<([u8; 20], Option<u64>)>,
	total_issuance: Option<Balance>,
	id: ParaId,
	council_members: Vec<AccountId>,
) -> serde_json::Value {
	let chain_id: u32 = id.into();

	endowed_accounts.extend(endowed_evm_accounts.into_iter().map(|(addr, id)| {
		let chain_id = id.unwrap_or_else(|| chain_id.into());
		AccountConverter::convert_evm_address(chain_id, addr)
	}));

	let num_endowed_accounts = endowed_accounts.len();
	let balances = match total_issuance {
		Some(total_issuance) => {
			let balance_per_endowed = total_issuance
				.checked_div(num_endowed_accounts as Balance)
				.unwrap_or(0 as Balance);
			endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, balance_per_endowed))
				.collect::<Vec<_>>()
		}
		None => vec![],
	};

	serde_json::json!({
		"balances": { "balances": balances },
		"council": {
			"members": council_members,
		},
		"fees": {
			"initialFees": vec![(
				FeeKey::AnchorsCommit,
				2_365_296_803_653u128,
			)],
		},
		"parachainInfo": {
			"parachainId": id,
		},
		"collatorSelection": {
			"invulnerables": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect::<Vec<_>>(),
			"candidacyBond": 1 * CFG,
		},
		"session": {
			"keys": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                       // account id
						acc,                               // validator id
						get_centrifuge_session_keys(aura), // session keys
					)
				})
				.collect::<Vec<_>>(),
		},
		"bridge": {
			"chains": vec![0],
			"resources": vec![
				(
					hex!["00000000000000000000000000000009e974040e705c10fb4de576d6cc261900"],
					hex!["50616c6c65744272696467652e7472616e73666572"].to_vec(),
				),
			],
			"relayers": vec![
				Into::<AccountId>::into(hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"]),
				Into::<AccountId>::into(hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"]),
			],
			"threshold": 1,
		},
		"blockRewards": {
			"collators": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect::<Vec<_>>(),
			"collatorReward": 8_325 * MILLI_CFG,
			"treasuryInflationRate": Rate::saturating_from_rational(3, 100),
		},
		"evmChainId": {
			"chainId": Into::<u32>::into(chain_id),
		},
		"evm": {
			"accounts": precompile_account_genesis::<CentrifugePrecompiles>(),
		},
		"polkadotXcm": {
			"safeXcmVersion": Some(SAFE_XCM_VERSION),
		},
	})
}

fn altair_genesis(
	initial_authorities: Vec<(AccountId, AuraId)>,
	mut endowed_accounts: Vec<AccountId>,
	endowed_evm_accounts: Vec<([u8; 20], Option<u64>)>,
	total_issuance: Option<Balance>,
	id: ParaId,
	council_members: Vec<AccountId>,
) -> serde_json::Value {
	let chain_id: u32 = id.into();

	endowed_accounts.extend(endowed_evm_accounts.into_iter().map(|(addr, id)| {
		let chain_id = id.unwrap_or_else(|| chain_id.into());
		AccountConverter::convert_evm_address(chain_id, addr)
	}));

	let num_endowed_accounts = endowed_accounts.len();
	let balances = match total_issuance {
		Some(total_issuance) => {
			let balance_per_endowed = total_issuance
				.checked_div(num_endowed_accounts as Balance)
				.unwrap_or(0 as Balance);
			endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, balance_per_endowed))
				.collect::<Vec<_>>()
		}
		None => vec![],
	};

	serde_json::json!({
		"balances": { "balances": balances },
		"council": {
			"members": council_members,
		},
		"fees": {
			"initialFees": vec![(
				// Anchoring state rent fee per day
				// pre-image: 0xdb4faa73ca6d2016e53c7156087c176b79b169c409b8a0063a07964f3187f9e9
				// hash   : 0x11da6d1f761ddf9bdb4c9d6e5303ebd41f61858d0a5647a1a7bfe089bf921be9
				FeeKey::AnchorsCommit,
				// Daily state rent, defined such that it will amount to 0.00259.. RAD
				// (2_590_000_000_000_040) over 3 years, which is the expected average anchor
				// duration. The other fee components for anchors amount to about 0.00041.. RAD
				// (410_000_000_000_000), such that the total anchor price for 3 years will be
				// 0.003.. RAD
				2_365_296_803_653u128,
			)],
		},
		"parachainInfo": {
			"parachainId": id,
		},
		"collatorSelection": {
			"invulnerables": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect::<Vec<_>>(),
			"candidacyBond": 1 * AIR,
		},
		"blockRewards": {
			"collators": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect::<Vec<_>>(),
			"collatorReward": 98_630 * MILLI_AIR,
			"treasuryInflationRate": Rate::saturating_from_rational(3, 100),
		},
		"session": {
			"keys": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                   // account id
						acc,                           // validator id
						get_altair_session_keys(aura), // session keys
					)
				})
				.collect::<Vec<_>>(),
		},
		"evmChainId": {
			"chainId": Into::<u32>::into(chain_id),
		},
		"evm": {
			"accounts": precompile_account_genesis::<AltairPrecompiles>(),
		},
		"polkadotXcm": {
			"safeXcmVersion": Some(SAFE_XCM_VERSION),
		},
	})
}

/// The CurrencyId for the USDT asset on the development runtime
const DEV_USDT_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);
const DEV_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2);

fn development_genesis(
	root_key: AccountId,
	initial_authorities: Vec<(AccountId, AuraId)>,
	mut endowed_accounts: Vec<AccountId>,
	endowed_evm_accounts: Vec<([u8; 20], Option<u64>)>,
	total_issuance: Option<Balance>,
	id: ParaId,
) -> serde_json::Value {
	let chain_id: u32 = id.into();

	endowed_accounts.extend(endowed_evm_accounts.into_iter().map(|(addr, id)| {
		let chain_id = id.unwrap_or_else(|| chain_id.into());
		AccountConverter::convert_evm_address(chain_id, addr)
	}));

	let num_endowed_accounts = endowed_accounts.len();
	let (balances, token_balances) = match total_issuance {
		Some(total_issuance) => {
			let balance_per_endowed = total_issuance
				.checked_div(num_endowed_accounts as Balance)
				.unwrap_or(0 as Balance);

			(
				// pallet_balances balances
				endowed_accounts
					.iter()
					.cloned()
					.map(|x| (x, balance_per_endowed))
					.collect::<Vec<_>>(),
				// orml_tokens balances
				// bootstrap each endowed accounts with 1 million of each the foreign assets.
				endowed_accounts
					.iter()
					.cloned()
					.flat_map(|x| {
						// NOTE: We can only mint these foreign assets on development
						vec![
							// USDT is a 6-decimal asset, so 1 million + 6 zeros
							(x.clone(), DEV_USDT_CURRENCY_ID, 1_000_000_000_000u128),
							// AUSD is a 12-decimal asset, so 1 million + 12 zeros
							(x, DEV_AUSD_CURRENCY_ID, 1_000_000_000_000_000_000u128),
						]
					})
					.collect::<Vec<_>>(),
			)
		}
		None => (vec![], vec![]),
	};
	let chain_id: u32 = id.into();

	serde_json::json!({
		"balances": { "balances": balances },
		"ormlAssetRegistry": {
			"assets": asset_registry_assets(),
		},
		"ormlTokens": {
			"balances": token_balances,
		},
		"fees": {
			"initialFees": vec![(
				// Anchoring state rent fee per day
				// pre-"image": 0xdb4faa73ca6d2016e53c7156087c176b79b169c409b8a0063a07964f3187f9e9
				// "hash   ": 0x11da6d1f761ddf9bdb4c9d6e5303ebd41f61858d0a5647a1a7bfe089bf921be9
				FeeKey::AnchorsCommit,
				// Daily state rent, defined such that it will amount to 0.00259.. RAD
				// (2_590_000_000_000_040) over 3 years, which is the expected average anchor
				// duration. The other fee components for anchors amount to about 0.00041.. RAD
				// (410_000_000_000_000), such that the total anchor price for 3 years will be
				// 0.003.. RAD
				2_365_296_803_653u128,
			)],
		},
		"sudo": {
			"key": Some(root_key),
		},
		"parachainInfo":  {
			"parachainId": id,
		},
		"collatorSelection": {
			"invulnerables": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect::<Vec<_>>(),
			"candidacyBond": 1 * CFG,
		},
		"session": {
			"keys": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                        // account id
					acc,                                // validator id
						get_development_session_keys(aura), // session keys
					)
				})
				.collect::<Vec<_>>(),
		},
		"bridge": {
			// Whitelist chains Ethereum - 0
			"chains": vec![0],
			// Register resourceIDs
			"resources": vec![
				// xCFG ResourceID to PalletBridge.transfer method (for incoming txs)
				(
					hex!["00000000000000000000000000000009e974040e705c10fb4de576d6cc261900"],
					hex!["50616c6c65744272696467652e7472616e73666572"].to_vec(),
				),
			],
			// Dev Alice - 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
			// Sample Endowed1 - 5GVimUaccBq1XbjZ99Zmm8aytG6HaPCjkZGKSHC1vgrsQsLQ
			"relayers": vec![
				Into::<AccountId>::into(hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"]),
				Into::<AccountId>::into(hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"]),
			],
			"threshold": 1,
		},
		"blockRewards": {
			"collators": initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect::<Vec<_>>(),
			"collatorReward": 8_325 * MILLI_CFG,
			"treasuryInflationRate": Rate::saturating_from_rational(3, 100),
		},
		"evmChainId": {
			"chainId": Into::<u32>::into(chain_id),
		},
		"evm": {
			"accounts": precompile_account_genesis::<DevelopmentPrecompiles>(),
		},
		"polkadotXcm": {
			"safeXcmVersion": Some(SAFE_XCM_VERSION),
		},
	})
}

fn asset_registry_assets() -> Vec<(CurrencyId, Vec<u8>)> {
	vec![
		(
			DEV_USDT_CURRENCY_ID,
			AssetMetadata {
				decimals: 6,
				name: b"Tether USD"
					.to_vec()
					.try_into()
					.expect("fit in the BoundedVec"),
				symbol: b"USDT".to_vec().try_into().expect("fit in the BoundedVec"),
				existential_deposit: 0u128,
				location: Some(staging_xcm::VersionedLocation::V4(Location {
					parents: 1,
					interior: X3(Arc::new([
						Parachain(parachains::rococo::rocksmine::ID),
						PalletInstance(parachains::rococo::rocksmine::usdt::PALLET_INSTANCE),
						GeneralIndex(parachains::rococo::rocksmine::usdt::GENERAL_INDEX),
					])),
				})),
				additional: CustomMetadata {
					mintable: false,
					permissioned: false,
					pool_currency: true,
					transferability: CrossChainTransferability::Xcm(Default::default()),
					local_representation: None,
				},
			}
			.encode(),
		),
		(
			DEV_AUSD_CURRENCY_ID,
			AssetMetadata {
				decimals: 12,
				name: b"Acala USD"
					.to_vec()
					.try_into()
					.expect("fit in the BoundedVec"),
				symbol: b"AUSD".to_vec().try_into().expect("fit in the BoundedVec"),
				existential_deposit: 0u128,
				location: Some(staging_xcm::VersionedLocation::V4(Location {
					parents: 1,
					interior: X2(Arc::new([
						Parachain(parachains::rococo::acala::ID),
						GeneralKey {
							length: parachains::rococo::acala::AUSD_KEY.to_vec().len() as u8,
							data: vec_to_fixed_array(parachains::rococo::acala::AUSD_KEY.to_vec()),
						},
					])),
				})),
				additional: CustomMetadata {
					mintable: false,
					permissioned: false,
					pool_currency: true,
					transferability: CrossChainTransferability::Xcm(Default::default()),
					local_representation: None,
				},
			}
			.encode(),
		),
		(
			usdc::CURRENCY_ID_LOCAL,
			AssetMetadata {
				decimals: 6,
				name: b"Local USDC"
					.to_vec()
					.try_into()
					.expect("fit in the BoundedVec"),
				symbol: b"localUSDC"
					.to_vec()
					.try_into()
					.expect("fit in the BoundedVec"),
				existential_deposit: 0u128,
				location: None,
				additional: CustomMetadata {
					mintable: false,
					permissioned: false,
					pool_currency: true,
					transferability: CrossChainTransferability::None,
					local_representation: None,
				},
			}
			.encode(),
		),
		(
			usdc::CURRENCY_ID_LP_ETH_GOERLI,
			AssetMetadata {
				decimals: usdc::DECIMALS,
				name: b"LP Ethereum Wrapped USDC"
					.to_vec()
					.try_into()
					.expect("fit in the BoundedVec"),
				symbol: b"LpEthUSDC"
					.to_vec()
					.try_into()
					.expect("fit in the BoundedVec"),
				existential_deposit: usdc::EXISTENTIAL_DEPOSIT,
				location: Some(staging_xcm::VersionedLocation::V4(Location {
					parents: 0,
					interior: X3(Arc::new([
						PalletInstance(development_runtime::LiquidityPoolsPalletIndex::get()),
						GlobalConsensus(NetworkId::Ethereum {
							chain_id: usdc::CHAIN_ID_ETH_GOERLI_TESTNET,
						}),
						AccountKey20 {
							network: None,
							key: usdc::CONTRACT_ETH_GOERLI,
						},
					])),
				})),
				additional: CustomMetadata {
					transferability: CrossChainTransferability::LiquidityPools,
					mintable: false,
					permissioned: false,
					pool_currency: true,
					local_representation: Some(usdc::LOCAL_ASSET_ID),
				},
			}
			.encode(),
		),
	]
}

fn precompile_account_genesis<PrecompileSet: H160Addresses>(
) -> BTreeMap<H160, fp_evm::GenesisAccount> {
	PrecompileSet::h160_addresses()
		.map(|addr| {
			(
				addr,
				fp_evm::GenesisAccount {
					nonce: Default::default(),
					balance: Default::default(),
					storage: Default::default(),
					code: runtime_common::evm::precompile::utils::REVERT_BYTECODE.to_vec(),
				},
			)
		})
		.collect()
}
