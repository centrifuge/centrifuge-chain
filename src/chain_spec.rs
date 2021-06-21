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

use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use node_primitives::{AccountId, Hash, Signature};
use node_runtime::constants::currency::*;
use node_runtime::{AuraId, Balance};
use sc_service::{ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

const POLKADOT_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<node_runtime::GenesisConfig>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

pub fn get_session_keys(keys: AuraId) -> node_runtime::SessionKeys {
	node_runtime::SessionKeys { aura: keys }
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

pub fn charcoal_local_network(para_id: ParaId) -> ChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CAIR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	ChainSpec::from_genesis(
		"Charcoal Local Testnet",
		"charcoal_local_testnet",
		ChainType::Local,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				Some(10000000 * AIR),
				para_id,
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Default::default(),
	)
}

pub fn charcoal_staging_network(para_id: ParaId) -> ChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CAIR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	ChainSpec::from_genesis(
		"Charcoal Testnet",
		"charcoal_testnet",
		ChainType::Live,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<AuraId>("Bob"),
					),
				],
				endowed_accounts(),
				Some(10000000 * AIR),
				para_id,
			)
		},
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(POLKADOT_TELEMETRY_URL.to_string(), 0)])
				.expect("Polkadot telemetry url is valid; qed"),
		),
		Some("charcoal"),
		Some(properties),
		Default::default(),
	)
}

pub fn rumba_staging_network(para_id: ParaId) -> ChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "RCFG".into());
	properties.insert("tokenDecimals".into(), 18.into());

	ChainSpec::from_genesis(
		"Rumba Testnet",
		"rumba_testnet",
		ChainType::Live,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
				)],
				endowed_accounts(),
				Some(10000000 * AIR),
				para_id,
			)
		},
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(POLKADOT_TELEMETRY_URL.to_string(), 0)])
				.expect("Polkadot telemetry url is valid; qed"),
		),
		Some("rumba"),
		Some(properties),
		Default::default(),
	)
}

// TODO: Replace with Cyclone spec
pub fn cyclone_config() -> ChainSpec {
	ChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..]).unwrap()
}

// TODO: Replace with Altair spec
pub fn altair_config() -> ChainSpec {
	ChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..]).unwrap()
}

pub fn rumba_config() -> ChainSpec {
	ChainSpec::from_json_bytes(&include_bytes!("../res/rumba-spec-raw.json")[..]).unwrap()
}

pub fn charcoal_config() -> ChainSpec {
	ChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..]).unwrap()
}

pub fn altair_dev(para_id: ParaId) -> ChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DAIR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	ChainSpec::from_genesis(
		"Altair Dev",
		"altair_dev",
		ChainType::Local,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<AuraId>("Alice"),
					//hex!("8cf7ef0821d2502301f64fe0a7e729d88dfa0cef81773d246add643668edd833").into(),
					// hex!("8cf7ef0821d2502301f64fe0a7e729d88dfa0cef81773d246add643668edd833")
					// 	.unchecked_into(),
				)],
				endowed_accounts(),
				None,
				para_id,
			)
		},
		vec![],
		None,
		None,
		Some(properties),
		Default::default(),
	)
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

fn testnet_genesis(
	root_key: AccountId,
	initial_authorities: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<AccountId>,
	total_issuance: Option<Balance>,
	id: ParaId,
) -> node_runtime::GenesisConfig {
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

	node_runtime::GenesisConfig {
		system: node_runtime::SystemConfig {
			code: node_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
			changes_trie_config: Default::default(),
		},
		balances: node_runtime::BalancesConfig { balances },
		elections: node_runtime::ElectionsConfig { members: vec![] },
		council: node_runtime::CouncilConfig {
			members: Default::default(),
			phantom: Default::default(),
		},
		fees: node_runtime::FeesConfig {
			initial_fees: vec![(
				// Anchoring state rent fee per day
				// pre-image: 0xdb4faa73ca6d2016e53c7156087c176b79b169c409b8a0063a07964f3187f9e9
				// hash   : 0x11da6d1f761ddf9bdb4c9d6e5303ebd41f61858d0a5647a1a7bfe089bf921be9
				Hash::from(&[
					17, 218, 109, 31, 118, 29, 223, 155, 219, 76, 157, 110, 83, 3, 235, 212, 31,
					97, 133, 141, 10, 86, 71, 161, 167, 191, 224, 137, 191, 146, 27, 233,
				]),
				// Daily state rent, defined such that it will amount to 0.00259.. RAD (2_590_000_000_000_040) over
				// 3 years, which is the expected average anchor duration. The other fee components for anchors amount
				// to about 0.00041.. RAD (410_000_000_000_000), such that the total anchor price for 3 years will be
				// 0.003.. RAD
				2_365_296_803_653,
			)],
		},
		vesting: Default::default(),
		sudo: node_runtime::SudoConfig { key: root_key },
		parachain_info: node_runtime::ParachainInfoConfig { parachain_id: id },
		session: node_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),            // account id
						acc.clone(),            // validator id
						get_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		aura_ext: Default::default(),
		aura: Default::default(),
		anchor: Default::default(),
		democracy: Default::default(),
		parachain_system: Default::default(),
	}
}
