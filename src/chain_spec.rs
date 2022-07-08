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
use runtime_common::{decimals, CFG};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use serde::{Deserialize, Serialize};

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

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

pub fn get_altair_session_keys(keys: altair_runtime::AuraId) -> altair_runtime::SessionKeys {
	altair_runtime::SessionKeys { aura: keys }
}

pub fn get_centrifuge_session_keys(
	keys: centrifuge_runtime::AuraId,
) -> centrifuge_runtime::SessionKeys {
	centrifuge_runtime::SessionKeys { aura: keys }
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

pub fn centrifuge_config() -> CentrifugeChainSpec {
	CentrifugeChainSpec::from_json_bytes(
		&include_bytes!("../res/genesis/centrifuge-genesis-spec-raw.json")[..],
	)
	.unwrap()
}

pub fn centrifuge_staging(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CFG".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	CentrifugeChainSpec::from_genesis(
		"Centrifuge",
		"centrifuge",
		ChainType::Live,
		move || {
			centrifuge_genesis(
				vec![
					(
						// 4dsemFj9QroJbpP1Zdd18DXVvYeyo6ymvnGTEvEvs5ikPCxF
						hex!["700a6abbcdbb6595cf48f019a4409c3670c42552d7f4b5bc317af642d91ceb09"]
							.into(),
						hex!["7e2a9759dcef70d18fa271026ba1b891391c22f1531055bf687b34fe547c3029"]
							.unchecked_into(),
					),
					(
						// 4dCqKqsy3VuQzakfQT2XTGTaMSC2jWK1jL8EaNZyvvApjjMG
						hex!["526f668def3ef79c8087552cfcecf575b89ac48a903379b5b5ec4f657ed6c67b"]
							.into(),
						hex!["087e9792a7ea8eb599d3696dbdbd0b1e957a3a29cc78405d7c84f96a6ecab725"]
							.unchecked_into(),
					),
					(
						// 4deVxTkHqXeueeNS8dF9fwKFiDJEMujCzEzrhm2aFhkELxLA
						hex!["6602949762bcfc0e52685f01d9723ea9eb92e4102fae739b7f1143cae518ce74"]
							.into(),
						hex!["96504d2fe659a6ab6b4d2ded1340de5d995d25d9aad3be37d948bc5259355512"]
							.unchecked_into(),
					),
				],
				vec![
					hex!["b03cd3fb823de75f888ac647105d7820476a6b1943a74af840996d2b28e64017"].into(),
				],
				Some(1000 * AIR),
				para_id,
				Default::default(),
			)
		},
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(POLKADOT_TELEMETRY_URL.to_string(), 0)])
				.expect("Polkadot telemetry url is valid; qed"),
		),
		Some("centrifuge"),
		None,
		Some(properties),
		Default::default(),
	)
}

pub fn centrifuge_dev(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DCFG".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	CentrifugeChainSpec::from_genesis(
		"Centrifuge Dev",
		"centrifuge_dev",
		ChainType::Live,
		move || {
			centrifuge_genesis(
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<centrifuge_runtime::AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<centrifuge_runtime::AuraId>("Bob"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie"),
						get_from_seed::<centrifuge_runtime::AuraId>("Charlie"),
					),
				],
				endowed_accounts(),
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
		Default::default(),
	)
}

pub fn centrifuge_local(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DCFG".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	CentrifugeChainSpec::from_genesis(
		"Centrifuge Local",
		"centrifuge_local",
		ChainType::Local,
		move || {
			centrifuge_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<centrifuge_runtime::AuraId>("Alice"),
				)],
				endowed_accounts(),
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
		Default::default(),
	)
}

pub fn catalyst_config() -> CentrifugeChainSpec {
	CentrifugeChainSpec::from_json_bytes(&include_bytes!("../res/catalyst-spec-raw.json")[..])
		.unwrap()
}

pub fn catalyst_staging(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "NCFG".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	CentrifugeChainSpec::from_genesis(
		"Catalyst Testnet",
		"catalyst_testnet",
		ChainType::Live,
		move || {
			centrifuge_genesis(
				vec![
					(
						//4cSqT4wpxaSUwwmJoGvz6pXX31T5iP8SRyxrQRExquaQScwP
						hex!["30e105ac915a56bdf153e3a233bd767d538a3c76ba98dd4f3eae37487a804d24"]
							.into(),
						//4dngEiVgGmMRjaxkQFu8badZrhEetEHu4nFgBmVoBqkrNYTK
						hex!["6c3f266a8b74b0f5c1d9a93b2ec943b270003fea8218e89ab7ec4e9294a2584a"]
							.unchecked_into(),
					),
					(
						//4gWsFAXX4NRAgs2nZQ68eLfSGPKdskz6psY2gSLVQvNr63H2
						hex!["e4e4fab396035fc3c64b3a4127ac93687486fb21fbc5a69e14cae5c3e6025203"]
							.into(),
						//4chcpyjgumhQV7rZvegpYf7bCf5xUVfmh6ufr1wuvWEbamsT
						hex!["3c273686697bc47f164c5e1e80d4b9c7ce4c7b8cfdfcff069cceb0d9a128b920"]
							.unchecked_into(),
					),
					(
						//4gmT8vkpH8KTpLiwt8CEdrmcF8w5nQFJmYgP56fmTzE58Fvw
						hex!["f00481d4785faf42c44b77d59bd06b7edc0ed21f7ae00e5898fb43037e383049"]
							.into(),
						//4fkLCjk2BZ2QZf21YALguTxAxNCx7KEud7LCjjknchN4cAXE
						hex!["c2edaf71a4c09ade552fb2b078d9c346e509d7eb9c28356ad3f4f85f58bebe15"]
							.unchecked_into(),
					),
				],
				vec![
					hex!["cc5615f974947b126361c494090dd621777896c3f606912d9c772bdffeda4924"].into(),
				],
				Some(10000000 * CFG),
				para_id,
				Default::default(),
			)
		},
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(POLKADOT_TELEMETRY_URL.to_string(), 0)])
				.expect("Polkadot telemetry url is valid; qed"),
		),
		Some("catalyst"),
		None,
		Some(properties),
		Default::default(),
	)
}

pub fn catalyst_local(para_id: ParaId) -> CentrifugeChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "NCFG".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	CentrifugeChainSpec::from_genesis(
		"Catalyst Local",
		"catalyst_local",
		ChainType::Local,
		move || {
			centrifuge_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<altair_runtime::AuraId>("Alice"),
				)],
				endowed_accounts(),
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
		Default::default(),
	)
}

pub fn altair_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(
		&include_bytes!("../res/genesis/altair-genesis-spec-raw.json")[..],
	)
	.unwrap()
}

pub fn altair_staging(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "AIR".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Altair",
		"altair",
		ChainType::Live,
		move || {
			altair_genesis(
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
				Default::default(),
			)
		},
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(POLKADOT_TELEMETRY_URL.to_string(), 0)])
				.expect("Polkadot telemetry url is valid; qed"),
		),
		Some("altair"),
		None,
		Some(properties),
		Default::default(),
	)
}

pub fn altair_dev(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DAIR".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Altair Dev",
		"altair_dev",
		ChainType::Live,
		move || {
			altair_genesis(
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<altair_runtime::AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<altair_runtime::AuraId>("Bob"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie"),
						get_from_seed::<altair_runtime::AuraId>("Charlie"),
					),
				],
				endowed_accounts(),
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
		Default::default(),
	)
}

pub fn altair_local(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DAIR".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Altair Local",
		"altair_local",
		ChainType::Local,
		move || {
			altair_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<altair_runtime::AuraId>("Alice"),
				)],
				endowed_accounts(),
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
		Default::default(),
	)
}

pub fn antares_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/antares-spec-raw.json")[..]).unwrap()
}

pub fn antares_staging(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "NAIR".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Antares Testnet",
		"antares_testnet",
		ChainType::Live,
		move || {
			altair_genesis(
				// kAMp8Np345RVsnznxHNrsqS3BuNDWqFn5jXNT44vegDF3xcD8
				vec![
					(
						//kAKHQhXnjqLyv1nsCLEWb7fzPNVxXce3m8befa1tQtv1vxFxn
						hex!["5e4b3571ca8b591a3a4bbe74ef98c175ded537327eb0fee804b2b4bb9e6a4d17"]
							.into(),
						//kALq9HKGio1JH3FiP1nBqobG4Uw5ZWruAdisFnYPPV3LnngUq
						hex!["a2bb652a9722f01408b586aebc14891861809931e523c12e159399b9dd01c150"]
							.unchecked_into(),
					),
					(
						//kANowgeZWL2DhEzvcK5fZn9S6zWagoS8VivqSfHkxA1UetiAq
						hex!["fa499346a1c747b839d8f125e668bdd1342dff00c0c958f790bac11cbb08b51d"]
							.into(),
						//kANbf9tdTos3tjFwTWu8pys2vjRreayx2cyRKRhGnYU8XMXTK
						hex!["f0eafc07a1b05d926c5edf842752bbc25d8fe048d7aa4847fafc7b6577a51b7f"]
							.unchecked_into(),
					),
					(
						//kAHAYwo51dEmSLPXibTGvyB6gZ94uhEvWrW6jWS2Xay4drscH
						hex!["0097c8435cd03de1e57045221de04c23fc14a36fc82b50ea35ddc0165a7f8626"]
							.into(),
						//kAMpz3UFrxHoWsW6JcadsdtZjQenT4yptu4XTMfsnQUJQzyTq
						hex!["ced887433a5c8c1e0af93bf6c5de96a39fe09be06bc3f747b76fa0cab9ef4a69"]
							.unchecked_into(),
					),
				],
				vec![
					hex!["ce3155fe53b83191a3d50da03b2368d0e596a43c09885cd9de9b0ada82782952"].into(),
				],
				Some(10000000 * AIR),
				para_id,
				Default::default(),
			)
		},
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(POLKADOT_TELEMETRY_URL.to_string(), 0)])
				.expect("Polkadot telemetry url is valid; qed"),
		),
		Some("antares"),
		None,
		Some(properties),
		Default::default(),
	)
}

pub fn antares_local(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "NAIR".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Antares Local",
		"antares_local",
		ChainType::Local,
		move || {
			altair_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<altair_runtime::AuraId>("Alice"),
				)],
				endowed_accounts(),
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
		Default::default(),
	)
}

pub fn charcoal_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../res/charcoal-spec-raw.json")[..]).unwrap()
}

pub fn charcoal_staging(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CAIR".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Charcoal Testnet",
		"charcoal_testnet",
		ChainType::Live,
		move || {
			altair_genesis(
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
				Default::default(),
			)
		},
		vec![],
		Some(
			TelemetryEndpoints::new(vec![(POLKADOT_TELEMETRY_URL.to_string(), 0)])
				.expect("Polkadot telemetry url is valid; qed"),
		),
		Some("charcoal"),
		None,
		Some(properties),
		Default::default(),
	)
}

pub fn charcoal_local(para_id: ParaId) -> AltairChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "CAIR".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	AltairChainSpec::from_genesis(
		"Charcoal Local",
		"charcoal_local",
		ChainType::Local,
		move || {
			altair_genesis(
				vec![(
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_from_seed::<altair_runtime::AuraId>("Alice"),
				)],
				endowed_accounts(),
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
		Default::default(),
	)
}

pub fn development(para_id: ParaId) -> DevelopmentChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DEVEL".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

	DevelopmentChainSpec::from_genesis(
		"Dev Live",
		"devel_live",
		ChainType::Live,
		move || {
			development_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				vec![
					(
						get_account_id_from_seed::<sr25519::Public>("Alice"),
						get_from_seed::<development_runtime::AuraId>("Alice"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Bob"),
						get_from_seed::<development_runtime::AuraId>("Bob"),
					),
					(
						get_account_id_from_seed::<sr25519::Public>("Charlie"),
						get_from_seed::<development_runtime::AuraId>("Charlie"),
					),
				],
				endowed_accounts(),
				Some(10000000 * CFG),
				para_id,
			)
		},
		vec![],
		None,
		None,
		None,
		Some(properties),
		Default::default(),
	)
}

pub fn development_local(para_id: ParaId) -> DevelopmentChainSpec {
	let mut properties = Properties::new();
	properties.insert("tokenSymbol".into(), "DEVEL".into());
	properties.insert("tokenDecimals".into(), decimals::NATIVE.into());

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

fn council_members_bootstrap() -> Vec<AccountId> {
	endowed_accounts().into_iter().take(4).collect()
}

fn centrifuge_genesis(
	initial_authorities: Vec<(centrifuge_runtime::AccountId, centrifuge_runtime::AuraId)>,
	endowed_accounts: Vec<centrifuge_runtime::AccountId>,
	total_issuance: Option<centrifuge_runtime::Balance>,
	id: ParaId,
	council_members: Vec<centrifuge_runtime::AccountId>,
) -> centrifuge_runtime::GenesisConfig {
	let num_endowed_accounts = endowed_accounts.len();
	let balances = match total_issuance {
		Some(total_issuance) => {
			let balance_per_endowed = total_issuance
				.checked_div(num_endowed_accounts as centrifuge_runtime::Balance)
				.unwrap_or(0 as centrifuge_runtime::Balance);
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
		parachain_info: centrifuge_runtime::ParachainInfoConfig { parachain_id: id },
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
					hex!["50616c6c65744272696467652e7472616e73666572"]
						.iter()
						.cloned()
						.collect(),
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
	}
}

fn altair_genesis(
	initial_authorities: Vec<(altair_runtime::AccountId, altair_runtime::AuraId)>,
	endowed_accounts: Vec<altair_runtime::AccountId>,
	total_issuance: Option<altair_runtime::Balance>,
	id: ParaId,
	council_members: Vec<altair_runtime::AccountId>,
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
		None => (vec![]),
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
	let (balances, token_balances) = match total_issuance {
		Some(total_issuance) => {
			let balance_per_endowed = total_issuance
				.checked_div(num_endowed_accounts as development_runtime::Balance)
				.unwrap_or(0 as development_runtime::Balance);
			(
				endowed_accounts
					.iter()
					.cloned()
					.map(|k| (k, balance_per_endowed))
					.collect(),
				// We can only mint kUSD in development
				endowed_accounts
					.iter()
					.cloned()
					.map(|k| {
						(
							k,
							development_runtime::CurrencyId::AUSD,
							balance_per_endowed,
						)
					})
					.collect(),
			)
		}
		None => (vec![], vec![]),
	};

	development_runtime::GenesisConfig {
		system: development_runtime::SystemConfig {
			code: development_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
		},
		balances: development_runtime::BalancesConfig { balances },
		orml_asset_registry: Default::default(),
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
					hex!["50616c6c65744272696467652e7472616e73666572"]
						.iter()
						.cloned()
						.collect(),
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
	}
}
