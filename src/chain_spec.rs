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
use sc_service::{ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

use altair_runtime::constants::currency::AIR;
use runtime_common::CFG;

const POLKADOT_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec` instances for our runtimes.
pub type AltairChainSpec = sc_service::GenericChainSpec<altair_runtime::GenesisConfig>;
pub type CentrifugeChainSpec = sc_service::GenericChainSpec<centrifuge_runtime::GenesisConfig>;
pub type DevelopmentChainSpec = sc_service::GenericChainSpec<development_runtime::GenesisConfig>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

pub fn get_altair_session_keys(keys: altair_runtime::AuraId) -> altair_runtime::SessionKeys {
	altair_runtime::SessionKeys { aura: keys }
}

pub fn get_development_session_keys(
	keys: development_runtime::AuraId,
) -> development_runtime::SessionKeys {
	development_runtime::SessionKeys { aura: keys }
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

pub fn charcoal_local_network(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CAIR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	AltairChainSpec::from_genesis(
		"Charcoal Local Testnet",
		"charcoal_local_testnet",
		ChainType::Local,
		move || {
			altair_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<altair_runtime::AuraId>("Alice"),
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

pub fn altair_staging_network(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "AIR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	AltairChainSpec::from_genesis(
		"Altair",
		"altair",
		ChainType::Live,
		move || {
			altair_genesis(
				hex!["66d97d3816f5906c8a9821fac25afbb76291b12eb51c5a559e44aaafe4e42206"].into(),
				vec![
					(
						//
						hex!["b24fb587438bbe05034606dac98162d80be1d21ac6dd6edc989887fa53a8d503"]
							.into(),
						hex!["c475e1ba26aa503601f26568ce6989502fc316b41c6d788b58e4cba4ec967a73"]
							.unchecked_into(),
					),
					(
						//
						hex!["d46783c911c4d8fb42f8239eb8925857e27ee3bdd121feb43e450241891a5f1e"]
							.into(),
						hex!["4ab9526ff43c29426a6288621d85e3cbd45bcb279eab1cf250079b02d2a40e2f"]
							.unchecked_into(),
					),
					(
						//
						hex!["f02099f295f6ccd935646f50c6280f4054b7d1f9b126471668f4ac6175677c26"]
							.into(),
						hex!["2652d9800f7dcca7592c83857ecc674f34a51f7661d6dc06281565557e5ee217"]
							.unchecked_into(),
					),
				],
				vec![],
				None,
				para_id,
			)
		},
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(POLKADOT_TELEMETRY_URL.to_string(), 0)])
				.expect("Polkadot telemetry url is valid; qed"),
		),
		Some("altair"),
		Some(properties),
		Default::default(),
	)
}

pub fn charcoal_staging_network(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CAIR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	AltairChainSpec::from_genesis(
		"Charcoal Testnet",
		"charcoal_testnet",
		ChainType::Live,
		move || {
			altair_genesis(
				// kAJSPJQGb1w5Cn4ZTFPokiStQ6sNkYHApjzPBeNPdVwbyLGjs
				hex!["38e779a7cc9cc462e19ae0c8e76d6135caba7fee745645dbf9b4a1b9f53dbd6e"].into(),
				vec![
					(
						// kALpizfCQweMJjhMpDhfozAtLXrLfbkE7iMFWVt92xXrdcoZg
						hex!["a269a32274ddc7cb7f3a42ffb305c17011a67fbb97c9667a9f8ceb3141b6cb24"]
							.into(),
						hex!["f09f14e7b7bf0538793b1ff512fbe88c6f1d0ee08015dba416d27e6950803b21"]
							.unchecked_into(),
					),
					(
						// kAHvxmKFqevc6uJ3o7VoMZU78HTLZtoh9A4nrWrf3WLhwy76e
						hex!["2276c356c435f6bcbf7793b6419d1e12f8f270a6a53c28ce02737a9b5c65554d"]
							.into(),
						hex!["2211f2a23e278e9f9b8eba37033797c103b6453201369c3a951cf32d6a6e6b59"]
							.unchecked_into(),
					),
					(
						// kAKFBeQp4fZyYumtDNDu2xapHjoBFr6pzcVpXkEAoohC9JF7k
						hex!["5c98c66394608ea47747ce7a935fd94a70b508047383e8a6e9bbf3c620531c22"]
							.into(),
						hex!["4e5e5a7d116fe3528b9f015ff2f36af8460da4b38eb14a3f1659f278ff888709"]
							.unchecked_into(),
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

pub fn rumba_staging_network(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "RCFG".into());
	properties.insert("tokenDecimals".into(), 18.into());

	AltairChainSpec::from_genesis(
		"Rumba Testnet",
		"rumba_testnet",
		ChainType::Live,
		move || {
			altair_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<altair_runtime::AuraId>("Alice"),
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

pub fn cyclone_config() -> CentrifugeChainSpec {
	CentrifugeChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..])
		.unwrap()
}

pub fn altair_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/altair-spec-raw.json")[..]).unwrap()
}

pub fn rumba_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/rumba-spec-raw.json")[..]).unwrap()
}

pub fn charcoal_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..]).unwrap()
}

pub fn altair_dev(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DAIR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	AltairChainSpec::from_genesis(
		"Altair Dev",
		"altair_dev",
		ChainType::Local,
		move || {
			altair_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<altair_runtime::AuraId>("Alice"),
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

pub fn devel_local(para_id: ParaId) -> DevelopmentChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DAIR".into());
	properties.insert("tokenDecimals".into(), 18.into());

	DevelopmentChainSpec::from_genesis(
		"Dev Local",
		"devel_local",
		ChainType::Local,
		move || {
			development_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<development_runtime::AuraId>("Alice"),
				)],
				endowed_accounts(),
				Some(10000000 * CFG),
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

fn altair_genesis(
	root_key: AccountId,
	initial_authorities: Vec<(altair_runtime::AccountId, altair_runtime::AuraId)>,
	endowed_accounts: Vec<altair_runtime::AccountId>,
	total_issuance: Option<altair_runtime::Balance>,
	id: ParaId,
) -> altair_runtime::GenesisConfig {
	let num_endowed_accounts = endowed_accounts.len();
	let balances = match total_issuance {
		Some(total_issuance) => {
			let balance_per_endowed = total_issuance
				.checked_div(num_endowed_accounts as altair_runtime::Balance)
				.unwrap_or(0 as altair_runtime::Balance);
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
			changes_trie_config: Default::default(),
		},
		balances: altair_runtime::BalancesConfig { balances },
		elections: altair_runtime::ElectionsConfig { members: vec![] },
		council: altair_runtime::CouncilConfig {
			members: Default::default(),
			phantom: Default::default(),
		},
		fees: altair_runtime::FeesConfig {
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
		sudo: altair_runtime::SudoConfig { key: root_key },
		parachain_info: altair_runtime::ParachainInfoConfig { parachain_id: id },
		session: altair_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                   // account id
						acc.clone(),                   // validator id
						get_altair_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		aura_ext: Default::default(),
		aura: Default::default(),
		democracy: Default::default(),
		parachain_system: Default::default(),
	}
}

fn development_genesis(
	root_key: AccountId,
	initial_authorities: Vec<(development_runtime::AccountId, development_runtime::AuraId)>,
	endowed_accounts: Vec<development_runtime::AccountId>,
	total_issuance: Option<development_runtime::Balance>,
	id: ParaId,
) -> development_runtime::GenesisConfig {
	let num_endowed_accounts = endowed_accounts.len();
	let balances = match total_issuance {
		Some(total_issuance) => {
			let balance_per_endowed = total_issuance
				.checked_div(num_endowed_accounts as development_runtime::Balance)
				.unwrap_or(0 as development_runtime::Balance);
			endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, balance_per_endowed))
				.collect()
		}
		None => vec![],
	};

	development_runtime::GenesisConfig {
		system: development_runtime::SystemConfig {
			code: development_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
			changes_trie_config: Default::default(),
		},
		balances: development_runtime::BalancesConfig { balances },
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
		sudo: development_runtime::SudoConfig { key: root_key },
		parachain_info: development_runtime::ParachainInfoConfig { parachain_id: id },
		session: development_runtime::SessionConfig {
			keys: initial_authorities
				.iter()
				.cloned()
				.map(|(acc, aura)| {
					(
						acc.clone(),                        // account id
						acc.clone(),                        // validator id
						get_development_session_keys(aura), // session keys
					)
				})
				.collect(),
		},
		aura_ext: Default::default(),
		aura: Default::default(),
		democracy: Default::default(),
		parachain_system: Default::default(),
	}
}
