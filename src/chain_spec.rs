use sp_core::{Pair, Public, crypto::UncheckedInto, sr25519};
use node_runtime::{
	AuthorityDiscoveryConfig, BabeConfig, BalancesConfig, CouncilConfig, DemocracyConfig,
	FeesConfig, GrandpaConfig, ImOnlineConfig, SessionConfig, SessionKeys, StakerStatus, StakingConfig,
	IndicesConfig, SystemConfig, WASM_BINARY,
};
use node_runtime::constants::currency::*;
use sc_service;
use hex_literal::hex;
use sp_finality_grandpa::{AuthorityId as GrandpaId};
use sp_consensus_babe::{AuthorityId as BabeId};
use pallet_im_online::sr25519::{AuthorityId as ImOnlineId};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_runtime::{Perbill, traits::{Verify, IdentifyAccount}};

pub use node_primitives::{AccountId, Balance, Hash, Signature};
pub use node_runtime::GenesisConfig;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::ChainSpec<GenesisConfig>;

/// The chain specification option.
#[derive(Clone, Debug)]
pub enum Alternative {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// Whatever the current runtime is, with simple Alice/Bob auths.
	LocalTestnet,
	/// The Fulvous testnet.
	Fulvous,
	/// The Flint testnet.
	Flint,
	/// The Amber testnet.
	Amber,
	/// Mainnet.
	Mainnet,
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(seed: &str) -> (
    AccountId,
    AccountId,
    GrandpaId,
    BabeId,
    ImOnlineId,
    AuthorityDiscoveryId,
) {
    (
        get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
		get_account_id_from_seed::<sr25519::Public>(seed),
        get_from_seed::<GrandpaId>(seed),
        get_from_seed::<BabeId>(seed),
        get_from_seed::<ImOnlineId>(seed),
        get_from_seed::<AuthorityDiscoveryId>(seed),
    )
}

/// Get a chain config from a spec setting.
impl Alternative {
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		Ok(match self {
			Alternative::Development => development_config(),
			Alternative::LocalTestnet => local_testnet_config(),
			Alternative::Fulvous => fulvous_config(),
			Alternative::Flint => flint_config()?,
			Alternative::Amber => amber_config()?,
			Alternative::Mainnet => panic!("Mainnet spec is not yet available"),
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::Development),
			"local" => Some(Alternative::LocalTestnet),
			"fulvous" => Some(Alternative::Fulvous),
			"flint" => Some(Alternative::Flint),
			"amber" => Some(Alternative::Amber),
			"" | "mainnet" => Some(Alternative::Mainnet),
			_ => None,
		}
	}
}

/// Flint testnet generator
pub fn flint_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../res/flint-cc2-spec.json")[..])
}

/// Amber testnet generator
pub fn amber_config() -> Result<ChainSpec, String> {
	ChainSpec::from_json_bytes(&include_bytes!("../res/amber-spec.json")[..])
}

fn session_keys(
    grandpa: GrandpaId,
    babe: BabeId,
    im_online: ImOnlineId,
    authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys { grandpa, babe, im_online, authority_discovery }
}

/// Helper function to create GenesisConfig for testing
pub fn testnet_genesis(
	// StashId, ControllerId, GrandpaId, BabeId, ImOnlineId, AuthorityDiscoveryId
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId, ImOnlineId, AuthorityDiscoveryId)>,
    endowed_accounts: Option<Vec<AccountId>>,
) -> GenesisConfig {
    let endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(|| {
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
    });
    let num_endowed_accounts = endowed_accounts.len();

    const INITIAL_SUPPLY: Balance = 300_000_000 * RAD; // 3% of total supply
    const STASH: Balance = 1_000_000 * RAD;
    let endowment: Balance = (INITIAL_SUPPLY - STASH * (initial_authorities.len() as Balance)) /
        (num_endowed_accounts as Balance);

    GenesisConfig {
        frame_system: Some(SystemConfig {
            code: WASM_BINARY.to_vec(),
            changes_trie_config: Default::default(),
        }),
        pallet_balances: Some(BalancesConfig {
            balances: endowed_accounts.iter().cloned()
                .map(|k| (k, endowment))
                .chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
                .collect(),
        }),
        pallet_indices: Some(IndicesConfig {
            indices: vec![],
        }),
        pallet_session: Some(SessionConfig {
			keys: initial_authorities.iter().map(|x| {
				(x.0.clone(), x.0.clone(), session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()))
			}).collect::<Vec<_>>(),
		}),
		pallet_staking: Some(StakingConfig {
            // The current era index.
			current_era: 0,
            // The ideal number of staking participants.
			validator_count: initial_authorities.len() as u32 * 2,
			minimum_validator_count: initial_authorities.len() as u32,
			stakers: initial_authorities.iter().map(|x| {
				(x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator)
			}).collect(),
            // Any validators that may never be slashed or forcibly kicked. It's a Vec since they're
            // easy to initialize and the performance hit is minimal (we expect no more than four
            // invulnerables) and restricted to testnets.
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
            // The percentage of the slash that is distributed to reporters.
		    // The rest of the slashed value is handled by the `Slash`.
			slash_reward_fraction: Perbill::from_percent(10),
            // True if the next session change will be a new era regardless of index.
            // force_era: NotForcing
			.. Default::default()
        }),
        pallet_democracy: Some(DemocracyConfig::default()),
		pallet_collective_Instance1: Some(CouncilConfig {
			members: endowed_accounts.iter()
						.take((num_endowed_accounts + 1) / 2)
						.cloned()
						.collect(),
			phantom: Default::default(),
		}),
        pallet_babe: Some(BabeConfig {
            authorities: vec![],
        }),
        pallet_im_online: Some(ImOnlineConfig {
			keys: vec![],
        }),
        pallet_authority_discovery: Some(AuthorityDiscoveryConfig {
			keys: vec![],
		}),
        pallet_grandpa: Some(GrandpaConfig {
            authorities: vec![],
        }),
        pallet_treasury: Some(Default::default()),
        fees: Some(FeesConfig {
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
    }
}

fn get_default_properties(token: &str) -> sc_service::Properties {
    let data = format!("\
		{{
			\"tokenDecimals\": 18,\
			\"tokenSymbol\": \"{}\"\
		}}", token);
    serde_json::from_str(&data).unwrap()
}

fn development_config_genesis() -> GenesisConfig {
	testnet_genesis(
		vec![
			get_authority_keys_from_seed("Alice"),
		],
		None,
	)
}

/// Development config (single validator Alice)
pub fn development_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Development",
		"dev",
		development_config_genesis,
		vec![],
		None,
		None,
		Some(get_default_properties("DRAD")),
		Default::default(),
	)
}

fn local_testnet_genesis() -> GenesisConfig {
	testnet_genesis(
		vec![
			get_authority_keys_from_seed("Alice"),
			get_authority_keys_from_seed("Bob"),
		],
		None,
	)
}

/// Local testnet config (multivalidator Alice + Bob)
pub fn local_testnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Local Testnet",
		"local_testnet",
		local_testnet_genesis,
		vec![],
		None,
		None,
		Some(get_default_properties("DRAD")),
		Default::default(),
	)
}

fn fulvous_genesis() -> GenesisConfig {
	testnet_genesis(
		vec![
            (
                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].into(),
                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].into(),
                hex!["8f9f7766fb5f36aeeed7a05b5676c14ae7c13043e3079b8a850131784b6d15d8"].unchecked_into(),
                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].unchecked_into(),
                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].unchecked_into(),
                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].unchecked_into(),
            ),
            (
                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].into(),
                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].into(),
                hex!["be1ce959980b786c35e521eebece9d4fe55c41385637d117aa492211eeca7c3d"].unchecked_into(),
                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].unchecked_into(),
                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].unchecked_into(),
                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].unchecked_into(),
            ),
        ],
        Some(vec![
            hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"].into()
        ]),
	)
}

/// Local testnet config (multivalidator Alice + Bob)
pub fn fulvous_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Fulvous Testnet",
		"fulvous",
		fulvous_genesis,
		vec![],
		None,
		Some("flvs"),
		Some(get_default_properties("TRAD")),
		Default::default(),
	)
}

pub fn load_spec(id: &str) -> Result<Option<ChainSpec>, String> {
	Ok(match Alternative::from(id) {
		Some(spec) => Some(spec.load()?),
		None => None,
	})
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;
	use crate::service::{new_full, new_light};
	use sc_service_test;
	use sp_runtime::BuildStorage;

	fn local_testnet_genesis_instant_single() -> GenesisConfig {
		testnet_genesis(
			vec![
				get_authority_keys_from_seed("Alice"),
			],
			None,
		)
	}

	/// Local testnet config (single validator - Alice)
	pub fn integration_test_config_with_single_authority() -> ChainSpec {
		ChainSpec::from_genesis(
			"Integration Test",
			"test",
			local_testnet_genesis_instant_single,
			vec![],
			None,
			None,
			None,
			Default::default(),
		)
	}

	/// Local testnet config (multivalidator Alice + Bob)
	pub fn integration_test_config_with_two_authorities() -> ChainSpec {
		ChainSpec::from_genesis(
			"Integration Test",
			"test",
			local_testnet_genesis,
			vec![],
			None,
			None,
			None,
			Default::default(),
		)
	}

	#[test]
	#[ignore]
	fn test_connectivity() {
		sc_service_test::connectivity(
			integration_test_config_with_two_authorities(),
			|config| new_full(config),
			|config| new_light(config),
		);
	}

	#[test]
	fn test_create_development_chain_spec() {
		development_config().build_storage().unwrap();
	}

	#[test]
	fn test_create_local_testnet_chain_spec() {
		local_testnet_config().build_storage().unwrap();
	}
}
