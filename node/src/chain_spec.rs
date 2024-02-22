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

use altair_runtime::constants::currency::{AIR, MILLI_AIR};
use cfg_primitives::{
	currency_decimals, parachains, AccountId, AuraId, Balance, BlockNumber, CFG, MILLI_CFG,
	SAFE_XCM_VERSION,
};
use cfg_types::{
	fee_keys::FeeKey,
	tokens::{
		usdc::{
			lp_wrapped_usdc_metadata, CHAIN_ID_ETH_GOERLI_TESTNET, CONTRACT_ETH_GOERLI,
			CURRENCY_ID_LOCAL, CURRENCY_ID_LP_ETH_GOERLI,
		},
		AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata,
	},
};
use cfg_utils::vec_to_fixed_array;
use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use runtime_common::{account_conversion::AccountConverter, evm::precompile::H160Addresses};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::{ChainType, Properties};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::UncheckedInto, sr25519, Encode, Pair, Public, H160};
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	FixedPointNumber,
};
use xcm::{
	latest::MultiLocation,
	prelude::{GeneralIndex, GeneralKey, PalletInstance, Parachain, X2, X3},
};

/// Specialized `ChainSpec` instances for our runtimes.
pub type AltairChainSpec = sc_service::GenericChainSpec<altair_runtime::GenesisConfig, Extensions>;
pub type CentrifugeChainSpec =
	sc_service::GenericChainSpec<centrifuge_runtime::GenesisConfig, Extensions>;
pub type DevelopmentChainSpec =
	sc_service::GenericChainSpec<development_runtime::GenesisConfig, Extensions>;

use altair_runtime::AltairPrecompiles;
use centrifuge_runtime::CentrifugePrecompiles;
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

pub fn centrifuge_dev(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DCFG".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	CentrifugeChainSpec::from_genesis(
		"Centrifuge Dev",
		"centrifuge_dev",
		ChainType::Live,
		move || {
			centrifuge_genesis(
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<AuraId>("Bob"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie"),
						get_from_seed::<AuraId>("Charlie"),
					),
				],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(100000000 * CFG),
				para_id,
				council_members_bootstrap(),
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

pub fn centrifuge_local(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DCFG".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	CentrifugeChainSpec::from_genesis(
		"Centrifuge Local",
		"centrifuge_local",
		ChainType::Local,
		move || {
			centrifuge_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(100000000 * CFG),
				para_id,
				council_members_bootstrap(),
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

pub fn catalyst_config() -> CentrifugeChainSpec {
	CentrifugeChainSpec::from_json_bytes(&include_bytes!("../res/catalyst-spec-raw.json")[..])
		.unwrap()
}

pub fn catalyst_local(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "NCFG".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	CentrifugeChainSpec::from_genesis(
		"Catalyst Local",
		"catalyst_local",
		ChainType::Local,
		move || {
			centrifuge_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(10000000 * CFG),
				para_id,
				Default::default(),
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

pub fn altair_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(
		&include_bytes!("../res/genesis/altair-genesis-spec-raw.json")[..],
	)
	.unwrap()
}

pub fn altair_dev(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DAIR".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Altair Dev",
		"altair_dev",
		ChainType::Live,
		move || {
			altair_genesis(
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<AuraId>("Bob"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie"),
						get_from_seed::<AuraId>("Charlie"),
					),
				],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(100000000 * AIR),
				para_id,
				council_members_bootstrap(),
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

pub fn altair_local(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DAIR".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Altair Local",
		"altair_local",
		ChainType::Local,
		move || {
			altair_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(100000000 * AIR),
				para_id,
				council_members_bootstrap(),
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

pub fn antares_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/antares-spec-raw.json")[..]).unwrap()
}

pub fn antares_local(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "NAIR".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Antares Local",
		"antares_local",
		ChainType::Local,
		move || {
			altair_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(10000000 * AIR),
				para_id,
				Default::default(),
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

pub fn charcoal_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..]).unwrap()
}

pub fn charcoal_local(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CAIR".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Charcoal Local",
		"charcoal_local",
		ChainType::Local,
		move || {
			altair_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(10000000 * AIR),
				para_id,
				Default::default(),
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

pub fn demo(para_id: ParaId) -> DevelopmentChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DEMO".into());
	properties.insert("tokenDecimals".into(), currency_decimals::NATIVE.into());

	DevelopmentChainSpec::from_genesis(
		"Demo Live",
		"demo_live",
		ChainType::Live,
		move || {
			development_genesis(
				// kANEUrMbi9xC16AfL5vSGwfvBVRoRdfWoQ8abPiXi5etFxpdP
				hex!["e0c426785313bb7e712d66dce43ccb81a7eaef373784511fb508fff4b5df3305"].into(),
				vec![(
					// kAHJNhAragKRrAb9X8JxSNYoqPqv36TspSwdSuyMfxGKUmfdH
					hex!["068f3bd4ed27bb83da8fdebbb4deba6b3b3b83ff47c8abad11e5c48c74c20b11"].into(),
					// kAKXFWse8rghi8mbAFB4RaVyZu6XZXq5i9wv7uYakZ3vQcxMR
					hex!["68d9baaa081802f8ec50d475b654810b158cdcb23e11c43815a6549f78f1b34f"]
						.unchecked_into(),
				)],
				demo_endowed_accounts(),
				vec![],
				Some(100000000 * CFG),
				para_id,
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

	DevelopmentChainSpec::from_genesis(
		"Dev Live",
		"devel_live",
		ChainType::Live,
		move || {
			development_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(10000000 * CFG),
				para_id,
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

	DevelopmentChainSpec::from_genesis(
		"Dev Local",
		"devel_local",
		ChainType::Local,
		move || {
			development_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				endowed_evm_accounts(),
				Some(10000000 * CFG),
				para_id,
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

fn demo_endowed_accounts() -> Vec<AccountId> {
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
	endowed_accounts().into_iter().take(4).collect()
}

fn centrifuge_genesis(
	initial_authorities: Vec<(AccountId, AuraId)>,
	mut endowed_accounts: Vec<AccountId>,
	endowed_evm_accounts: Vec<([u8; 20], Option<u64>)>,
	total_issuance: Option<Balance>,
	id: ParaId,
	council_members: Vec<AccountId>,
) -> centrifuge_runtime::GenesisConfig {
	let chain_id: u32 = id.into();

	endowed_accounts.extend(endowed_evm_accounts.into_iter().map(|(addr, id)| {
		let chain_id = id.unwrap_or_else(|| chain_id.into());
		AccountConverter::<
			centrifuge_runtime::Runtime,
			centrifuge_runtime::xcm::LocationToAccountId,
		>::convert_evm_address(chain_id, addr)
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
				.collect()
		}
		None => vec![],
	};

	centrifuge_runtime::GenesisConfig {
		system: centrifuge_runtime::SystemConfig {
			code: centrifuge_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: centrifuge_runtime::BalancesConfig { balances },
		orml_asset_registry: Default::default(),
		orml_tokens: centrifuge_runtime::OrmlTokensConfig { balances: vec![] },
		elections: centrifuge_runtime::ElectionsConfig { members: vec![] },
		council: centrifuge_runtime::CouncilConfig {
			members: council_members,
			phantom: Default::default(),
		},
		fees: centrifuge_runtime::FeesConfig {
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
		parachain_info: centrifuge_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: centrifuge_runtime::CollatorSelectionConfig {
			invulnerables: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect(),
			candidacy_bond: 1 * CFG,
			..Default::default()
		},
		collator_allowlist: Default::default(),
		session: centrifuge_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                       // account id
						acc,                               // validator id
						get_centrifuge_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		aura_ext: Default::default(),
		aura: Default::default(),
		democracy: Default::default(),
		parachain_system: Default::default(),
		bridge: centrifuge_runtime::BridgeConfig {
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
		treasury: Default::default(),
		block_rewards: centrifuge_runtime::BlockRewardsConfig {
			collators: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect(),
			collator_reward: 8_325 * MILLI_CFG,
			treasury_inflation_rate: Rate::saturating_from_rational(3, 100),
			last_update: std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.expect("SystemTime before UNIX EPOCH!")
				.as_secs(),
		},
		block_rewards_base: Default::default(),
		base_fee: Default::default(),
		evm_chain_id: centrifuge_runtime::EVMChainIdConfig {
			chain_id: chain_id.into(),
		},
		ethereum: Default::default(),
		evm: centrifuge_runtime::EVMConfig {
			accounts: precompile_account_genesis::<CentrifugePrecompiles>(),
		},
		liquidity_rewards_base: Default::default(),
		polkadot_xcm: centrifuge_runtime::PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
		},
	}
}

fn altair_genesis(
	initial_authorities: Vec<(AccountId, AuraId)>,
	mut endowed_accounts: Vec<AccountId>,
	endowed_evm_accounts: Vec<([u8; 20], Option<u64>)>,
	total_issuance: Option<Balance>,
	id: ParaId,
	council_members: Vec<AccountId>,
) -> altair_runtime::GenesisConfig {
	let chain_id: u32 = id.into();

	endowed_accounts.extend(endowed_evm_accounts.into_iter().map(|(addr, id)| {
		let chain_id = id.unwrap_or_else(|| chain_id.into());
		AccountConverter::<
			altair_runtime::Runtime,
			altair_runtime::xcm::LocationToAccountId,
		>::convert_evm_address(chain_id, addr)
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
				.collect()
		}
		None => vec![],
	};

	altair_runtime::GenesisConfig {
		system: altair_runtime::SystemConfig {
			code: altair_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: altair_runtime::BalancesConfig { balances },
		orml_asset_registry: Default::default(),
		orml_tokens: altair_runtime::OrmlTokensConfig { balances: vec![] },
		elections: altair_runtime::ElectionsConfig { members: vec![] },
		council: altair_runtime::CouncilConfig {
			members: council_members,
			phantom: Default::default(),
		},

		fees: altair_runtime::FeesConfig {
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
		parachain_info: altair_runtime::ParachainInfoConfig { parachain_id: id },
		collator_selection: altair_runtime::CollatorSelectionConfig {
			invulnerables: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect(),
			candidacy_bond: 1 * AIR,
			..Default::default()
		},
		block_rewards: altair_runtime::BlockRewardsConfig {
			collators: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect(),
			collator_reward: 98_630 * MILLI_AIR,
			treasury_inflation_rate: Rate::saturating_from_rational(3, 100),
			last_update: std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.expect("SystemTime before UNIX EPOCH!")
				.as_secs(),
		},
		block_rewards_base: Default::default(),
		collator_allowlist: Default::default(),
		session: altair_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                   // account id
						acc,                           // validator id
						get_altair_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		aura_ext: Default::default(),
		aura: Default::default(),
		democracy: Default::default(),
		parachain_system: Default::default(),
		treasury: Default::default(),
		base_fee: Default::default(),
		evm_chain_id: altair_runtime::EVMChainIdConfig {
			chain_id: chain_id.into(),
		},
		ethereum: Default::default(),
		evm: centrifuge_runtime::EVMConfig {
			accounts: precompile_account_genesis::<AltairPrecompiles>(),
		},
		liquidity_rewards_base: Default::default(),
		polkadot_xcm: altair_runtime::PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
		},
	}
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
) -> development_runtime::GenesisConfig {
	let chain_id: u32 = id.into();

	endowed_accounts.extend(endowed_evm_accounts.into_iter().map(|(addr, id)| {
		let chain_id = id.unwrap_or_else(|| chain_id.into());
		AccountConverter::<
			development_runtime::Runtime,
			development_runtime::xcm::LocationToAccountId,
		>::convert_evm_address(chain_id, addr)
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
		block_rewards: development_runtime::BlockRewardsConfig {
			collators: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, _)| acc)
				.collect(),
			collator_reward: 8_325 * MILLI_CFG,
			treasury_inflation_rate: Rate::saturating_from_rational(3, 100),
			last_update: std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.expect("SystemTime before UNIX EPOCH!")
				.as_secs(),
		},
		base_fee: Default::default(),
		evm_chain_id: development_runtime::EVMChainIdConfig {
			chain_id: chain_id.into(),
		},
		ethereum: Default::default(),
		evm: centrifuge_runtime::EVMConfig {
			accounts: precompile_account_genesis::<DevelopmentPrecompiles>(),
		},
		block_rewards_base: Default::default(),
		liquidity_rewards_base: Default::default(),
		polkadot_xcm: development_runtime::PolkadotXcmConfig {
			safe_xcm_version: Some(SAFE_XCM_VERSION),
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
					local_representation: None,
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
					local_representation: None,
				},
			}
			.encode(),
		),
		(
			CURRENCY_ID_LOCAL,
			AssetMetadata::<Balance, CustomMetadata> {
				decimals: 6,
				name: b"Local USDC".to_vec(),
				symbol: b"localUSDC".to_vec(),
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
			CURRENCY_ID_LP_ETH_GOERLI,
			lp_wrapped_usdc_metadata(
				"LP Ethereum Wrapped USDC".as_bytes().to_vec(),
				"LpEthUSDC".as_bytes().to_vec(),
				development_runtime::LiquidityPoolsPalletIndex::get(),
				CHAIN_ID_ETH_GOERLI_TESTNET,
				CONTRACT_ETH_GOERLI,
				true,
			)
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
