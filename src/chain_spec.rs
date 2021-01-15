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

use cumulus_primitives::ParaId;
use hex_literal::hex;
use node_runtime::{SessionKeys, StakerStatus, constants::currency::RAD};
use sp_consensus_babe::{AuthorityId as BabeId};
use pallet_im_online::sr25519::{AuthorityId as ImOnlineId};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use node_primitives::{AccountId, Balance, Hash, Signature};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{Perbill, traits::{IdentifyAccount, Verify}};

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type ChainSpec = sc_service::GenericChainSpec<node_runtime::GenesisConfig, Extensions>;

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

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn session_keys(
    babe: BabeId,
    im_online: ImOnlineId,
    authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys { babe, im_online, authority_discovery }
}

pub fn get_chain_spec(id: ParaId) -> ChainSpec {
	ChainSpec::from_genesis(
		"Local Testnet",
		"local_testnet",
		ChainType::Local,
		move || {
			testnet_genesis(
				get_account_id_from_seed::<sr25519::Public>("Alice"),
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
				id,
			)
		},
		vec![],
		None,
		None,
		None,
		Extensions {
			relay_chain: "westend-dev".into(),
			para_id: id.into(),
		},
	)
}

pub fn staging_test_net(id: ParaId) -> ChainSpec {
	ChainSpec::from_genesis(
		"Staging Testnet",
		"staging_testnet",
		ChainType::Live,
		move || {
			testnet_genesis(
				hex!["9ed7705e3c7da027ba0583a22a3212042f7e715d3c168ba14f1424e2bc111d00"].into(),
				vec![
					hex!["9ed7705e3c7da027ba0583a22a3212042f7e715d3c168ba14f1424e2bc111d00"].into(),
				],
				id,
			)
		},
		Vec::new(),
		None,
		None,
		None,
		Extensions {
			relay_chain: "westend-dev".into(),
			para_id: id.into(),
		},
	)
}

fn testnet_genesis(
	initial_authorities: Vec<(AccountId, AccountId, BabeId, ImOnlineId, AuthorityDiscoveryId)>,
	//root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	id: ParaId,
) -> node_runtime::GenesisConfig {
    let num_endowed_accounts = endowed_accounts.len();
    const STASH: Balance = 1_000_000 * RAD;

	node_runtime::GenesisConfig {
		frame_system: Some(node_runtime::SystemConfig {
			code: node_runtime::WASM_BINARY
				.expect("WASM binary was not build, please build it!")
				.to_vec(),
			changes_trie_config: Default::default(),
		}),
		pallet_balances: Some(node_runtime::BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 1 << 60))
				.collect(),
		}),
		pallet_session: Some(node_runtime::SessionConfig {
			keys: initial_authorities.iter().map(|x| {
				(x.0.clone(), x.0.clone(), session_keys(
					x.2.clone(),
					x.3.clone(),
					x.4.clone(),
					x.5.clone(),
				))
			}).collect::<Vec<_>>(),
		}),
		pallet_staking: Some(node_runtime::StakingConfig {
			validator_count: initial_authorities.len() as u32 * 2,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities.iter().map(|x| {
				(x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator)
			}).collect(),
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			.. Default::default()
		}),
		pallet_democracy: Some(node_runtime::DemocracyConfig::default()),
		pallet_elections_phragmen: Some(node_runtime::ElectionsConfig {
			members: vec![],
		}),
		pallet_collective_Instance1: Some(node_runtime::CouncilConfig {
			members: endowed_accounts.iter()
						.take((num_endowed_accounts + 1) / 2)
						.cloned()
						.collect(),
			phantom: Default::default(),
		}),
        pallet_babe: Some(node_runtime::BabeConfig {
            authorities: vec![],
        }),
        pallet_im_online: Some(node_runtime::ImOnlineConfig {
			keys: vec![],
        }),
		pallet_indices: Some(node_runtime::IndicesConfig {
			indices: vec![],
		}),
        pallet_authority_discovery: Some(node_runtime::AuthorityDiscoveryConfig {
			keys: vec![],
		}),
		pallet_bridge: Some(node_runtime::PalletBridgeConfig{
			// Whitelist chains Ethereum - 0
			chains: vec![0],
			// Register resourceIDs
			resources: vec![
				// xRAD ResourceID to PalletBridge.transfer method (for incoming txs)
				(hex!["00000000000000000000000000000009e974040e705c10fb4de576d6cc261900"], hex!["50616c6c65744272696467652e7472616e73666572"].iter().cloned().collect())
			],
			// Dev Alice - 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
			// Fulvous Endowed1 - 5GVimUaccBq1XbjZ99Zmm8aytG6HaPCjkZGKSHC1vgrsQsLQ
			relayers: vec![
				hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into(),
				hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"].into(),
			],
			threshold: 1,
		}),
        fees: Some(node_runtime::FeesConfig {
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
        }),
		pallet_vesting: Some(Default::default()),
		parachain_info: Some(node_runtime::ParachainInfoConfig { parachain_id: id }),
	}
}
