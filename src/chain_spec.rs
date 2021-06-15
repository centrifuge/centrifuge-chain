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
use node_primitives::{AccountId, Hash, Signature};
use sc_service::{ChainType, Properties};
use sc_telemetry::TelemetryEndpoints;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

const POLKADOT_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec` instances for our runtimes.
pub type CharcoalChainSpec = sc_service::GenericChainSpec<charcoal_runtime::GenesisConfig>;
pub type AltairChainSpec = sc_service::GenericChainSpec<altair_runtime::GenesisConfig>;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

pub fn charcoal_local_network(para_id: ParaId) -> CharcoalChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CCFG".into());
	properties.insert("tokenDecimals".into(), 18.into());

	CharcoalChainSpec::from_genesis(
		"Charcoal Local Testnet",
		"charcoal_local_testnet",
		ChainType::Local,
		move || {
			charcoal_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					get_from_seed::<charcoal_runtime::AuraId>("Alice"),
					get_from_seed::<charcoal_runtime::AuraId>("Bob"),
				],
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
				],
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

pub fn charcoal_staging_network(para_id: ParaId) -> CharcoalChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CCFG".into());
	properties.insert("tokenDecimals".into(), 18.into());

	CharcoalChainSpec::from_genesis(
		"Charcoal Testnet",
		"charcoal_testnet",
		ChainType::Live,
		move || {
			charcoal_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					get_from_seed::<charcoal_runtime::AuraId>("Alice"),
					get_from_seed::<charcoal_runtime::AuraId>("Bob"),
				],
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
				],
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
				vec![
					get_from_seed::<altair_runtime::AuraId>("Alice"),
					get_from_seed::<altair_runtime::AuraId>("Bob"),
				],
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
				],
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

// Todo: Replace with Cyclone spec
pub fn cyclone_config() -> CharcoalChainSpec {
	CharcoalChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..])
		.unwrap()
}

// TODO: Replace with Altair spec
pub fn altair_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..]).unwrap()
}

pub fn rumba_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/rumba-spec-raw.json")[..]).unwrap()
}

pub fn charcoal_config() -> CharcoalChainSpec {
	CharcoalChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..])
		.unwrap()
}

fn charcoal_genesis(
	root_key: AccountId,
	initial_authorities: Vec<charcoal_runtime::AuraId>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> charcoal_runtime::GenesisConfig {
	let num_endowed_accounts = endowed_accounts.len();

	charcoal_runtime::GenesisConfig {
		frame_system: charcoal_runtime::SystemConfig {
			code: charcoal_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_balances: charcoal_runtime::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1 << 60))
				.collect(),
		},
		pallet_democracy: charcoal_runtime::DemocracyConfig::default(),
		pallet_elections_phragmen: charcoal_runtime::ElectionsConfig { members: vec![] },
		pallet_collective_Instance1: charcoal_runtime::CouncilConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		// pallet_bridge: Some(charcoal_runtime::PalletBridgeConfig{
		// 	// Whitelist chains Ethereum - 0
		// 	chains: vec![0],
		// 	// Register resourceIDs
		// 	resources: vec![
		// 		// xRAD ResourceID to PalletBridge.transfer method (for incoming txs)
		// 		(hex!["00000000000000000000000000000009e974040e705c10fb4de576d6cc261900"], hex!["50616c6c65744272696467652e7472616e73666572"].iter().cloned().collect())
		// 	],
		// 	// Dev Alice - 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
		// 	// Fulvous Endowed1 - 5GVimUaccBq1XbjZ99Zmm8aytG6HaPCjkZGKSHC1vgrsQsLQ
		// 	relayers: vec![
		// 		hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into(),
		// 		hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"].into(),
		// 	],
		// 	threshold: 1,
		// }),
		pallet_fees: charcoal_runtime::FeesConfig {
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
		pallet_vesting: Default::default(),
		pallet_sudo: charcoal_runtime::SudoConfig { key: root_key },
		parachain_info: charcoal_runtime::ParachainInfoConfig { parachain_id: id },
		cumulus_pallet_aura_ext: Default::default(),
		pallet_aura: charcoal_runtime::AuraConfig {
			authorities: initial_authorities,
		},
		pallet_anchors: Default::default(),
	}
}

fn altair_genesis(
	root_key: AccountId,
	initial_authorities: Vec<altair_runtime::AuraId>,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> altair_runtime::GenesisConfig {
	let num_endowed_accounts = endowed_accounts.len();

	altair_runtime::GenesisConfig {
		frame_system: altair_runtime::SystemConfig {
			code: altair_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
			changes_trie_config: Default::default(),
		},
		pallet_balances: altair_runtime::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1 << 60))
				.collect(),
		},
		pallet_democracy: altair_runtime::DemocracyConfig::default(),
		pallet_elections_phragmen: altair_runtime::ElectionsConfig { members: vec![] },
		pallet_collective_Instance1: altair_runtime::CouncilConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		// pallet_bridge: Some(altair_runtime::PalletBridgeConfig{
		// 	// Whitelist chains Ethereum - 0
		// 	chains: vec![0],
		// 	// Register resourceIDs
		// 	resources: vec![
		// 		// xRAD ResourceID to PalletBridge.transfer method (for incoming txs)
		// 		(hex!["00000000000000000000000000000009e974040e705c10fb4de576d6cc261900"], hex!["50616c6c65744272696467652e7472616e73666572"].iter().cloned().collect())
		// 	],
		// 	// Dev Alice - 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
		// 	// Fulvous Endowed1 - 5GVimUaccBq1XbjZ99Zmm8aytG6HaPCjkZGKSHC1vgrsQsLQ
		// 	relayers: vec![
		// 		hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into(),
		// 		hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"].into(),
		// 	],
		// 	threshold: 1,
		// }),
		pallet_fees: altair_runtime::FeesConfig {
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
		pallet_vesting: Default::default(),
		pallet_sudo: altair_runtime::SudoConfig { key: root_key },
		parachain_info: altair_runtime::ParachainInfoConfig { parachain_id: id },
		cumulus_pallet_aura_ext: Default::default(),
		pallet_aura: altair_runtime::AuraConfig {
			authorities: initial_authorities,
		},
		pallet_anchors: Default::default(),
	}
}
