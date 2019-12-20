use chain_spec::ChainSpecExtension;
use primitives::{Pair, Public, crypto::UncheckedInto, sr25519};
use serde::{Serialize, Deserialize};
use node_runtime::{
    AuthorityDiscoveryConfig, BabeConfig, BalancesConfig, FeesConfig, GrandpaConfig, ImOnlineConfig, IndicesConfig, SessionConfig, SessionKeys,
    StakerStatus, StakingConfig, SudoConfig, SystemConfig, WASM_BINARY,
};
use node_runtime::Block;
use sc_service;
use hex_literal::hex;
use sc_telemetry::TelemetryEndpoints;
use grandpa_primitives::{AuthorityId as GrandpaId};
use babe_primitives::{AuthorityId as BabeId};
use im_online::sr25519::{AuthorityId as ImOnlineId};
use authority_discovery_primitives::AuthorityId as AuthorityDiscoveryId;
use sp_runtime::{Perbill, traits::{Verify, IdentifyAccount}};

pub use node_primitives::{AccountId, Balance, Hash, Signature};
pub use node_runtime::GenesisConfig;

type AccountPublic = <Signature as Verify>::Signer;

const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
pub struct Extensions {
	/// Block numbers with known hashes.
	pub fork_blocks: client::ForkBlocks<Block>,
}

/// Specialized `ChainSpec`.
pub type ChainSpec = sc_service::ChainSpec<
	GenesisConfig,
	Extensions,
>;

fn session_keys(
    grandpa: GrandpaId,
    babe: BabeId,
    im_online: ImOnlineId,
    authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys { grandpa, babe, im_online, authority_discovery }
}

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
    /// Whatever the current runtime is, with just Alice as an auth.
    Development,
    /// Whatever the current runtime is, with simple Alice/Bob auths.
    LocalTestnet,
    /// Fulvous testnet with whatever the current runtime is.
    Fulvous,
    /// Flint testnet with whatever the current runtime is and persistent disks.
    Flint,
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

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

impl Alternative {
    /// Get an actual chain config from one of the alternatives.
    pub(crate) fn load(self) -> Result<ChainSpec, String> {
        Ok(match self {
            Alternative::Development => ChainSpec::from_genesis(
                "Development",
                "dev",
                || {
                    testnet_genesis(
                        vec![get_authority_keys_from_seed("Alice")],
                        get_account_id_from_seed::<sr25519::Public>("Alice"),
                        vec![
                            get_account_id_from_seed::<sr25519::Public>("Alice"),
                            get_account_id_from_seed::<sr25519::Public>("Bob"),
                            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
                            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
                        ],
                        true,
                    )
                },
                vec![],
                None,
                None,
                Some(get_default_properties("DRAD")),
                Default::default(),
            ),
            Alternative::LocalTestnet => ChainSpec::from_genesis(
                "Local Testnet",
                "local_testnet",
                || {
                    testnet_genesis(
                        vec![
                            get_authority_keys_from_seed("Alice"),
                            get_authority_keys_from_seed("Bob"),
                        ],
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
                        true,
                    )
                },
                vec![],
                None,
                None,
                None,
                Default::default(),
            ),
            // Fulvous initial spec
            Alternative::Fulvous => {
                ChainSpec::from_genesis(
                    "Fulvous Testnet",
                    "fulvous",
                    || {
                        testnet_genesis(
                        vec![
                            (
                                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].into(), // TODO replace with other AccountId
                                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].into(), // TODO replace with other AccountId
                                hex!["8f9f7766fb5f36aeeed7a05b5676c14ae7c13043e3079b8a850131784b6d15d8"].unchecked_into(),
                                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].unchecked_into(),
                                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].unchecked_into(), // TODO replace with other AccountId
                                hex!["a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"].unchecked_into(), // TODO replace with other AccountId
                            ),
                            (
                                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].into(), // TODO replace with other AccountId
                                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].into(), // TODO replace with other AccountId
                                hex!["be1ce959980b786c35e521eebece9d4fe55c41385637d117aa492211eeca7c3d"].unchecked_into(),
                                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].unchecked_into(),
                                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].unchecked_into(), // TODO replace with other AccountId
                                hex!["42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"].unchecked_into(), // TODO replace with other AccountId
                            ),
                        ],
                        hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"].into(),
                        vec![
                            hex!["c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"].into()
                        ],
                        true,
                    )
                    },
                    vec![],
                    None,
                    Some("flvs"),
                    Some(get_default_properties("TRAD")),
                    Default::default(),
                )
            }
            // Flint initial spec
            Alternative::Flint => {
                ChainSpec::from_genesis(
                    "Flint Testnet CC1",
                    "flint-cc1",
                    || {
                        testnet_genesis(
                        vec![
                            (
                                hex!["e85164fc14c1275c398301fbfb9663916f4b0847331aa8ab2097c79358cb2a3d"].into(),
                                hex!["163fd93fd76775a38ee5d12f5e6ee2c106a92e5aa725a41e427a4f278887dc4c"].into(),
                                hex!["4f5d54c1796633251f9600b51e1961dec3939ceb0f927584f357c38b5463c95e"].unchecked_into(),
                                hex!["709c81a5ada8288f8c22b9605e9f8fba5034e13110799fdd7418bff37932c130"].unchecked_into(),
                                hex!["524d4cae76a0354c7adf531c61c2e1269ecef63154cfc5d513c554bbd705fc68"].unchecked_into(),
                                hex!["145139c78b70aadb6202cfdf2220d26a466ee39110812303b119a71f40f60571"].unchecked_into(),
                            ),
                            (
                                hex!["6c8f1e49c090d4998b23cc68d52453563785df4e84f3a10024b77d8b4649d51c"].into(),
                                hex!["2eaf31854d0d09ebbb920bf0bf4ff02570fa4f01d4557b5e1753bb70e5e6f25c"].into(),
                                hex!["9eb9733ca20fa497d0b6e502a9030fc9037ad2943e2b27057816632fcc7d2237"].unchecked_into(),
                                hex!["18291e4e4ca96f95d1014935880392dfd51ee99c1e9fd01e0255302f2984ef4a"].unchecked_into(),
                                hex!["922719894768d1e78efdb286e8f2bb118165332ff6c5b4ea3beb9ed43cea2718"].unchecked_into(),
                                hex!["16cde0520759b2ac5bc63c0a5a5ca4f8b97e2757bd8e8484c25aa73fb0f93955"].unchecked_into(),
                            ),
                        ],
                        hex!["c4051f94a879bd014647993acb2d52c4059a872b6e202e70c3121212416c5842"].into(),
                        vec![
                            hex!["c4051f94a879bd014647993acb2d52c4059a872b6e202e70c3121212416c5842"].into(),
                        ],
                        true,
                    )
                    },
                    vec![],
                    Some(TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])),
                    Some("flint-cc1"),
                    Some(get_default_properties("FRAD")),
                    Default::default(),
                )
            }
        })
    }

    pub(crate) fn from(s: &str) -> Option<Self> {
        match s {
            "dev" => Some(Alternative::Development),
            "" | "local" => Some(Alternative::LocalTestnet),
            "fulvous" => Some(Alternative::Fulvous),
            "flint" => Some(Alternative::Flint),
            _ => None,
        }
    }
}

fn testnet_genesis(
    initial_authorities: Vec<(AccountId, AccountId, GrandpaId, BabeId, ImOnlineId, AuthorityDiscoveryId)>, // StashId, ControllerId, GrandpaId, BabeId, ImOnlineId, AuthorityDiscoveryId
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
) -> GenesisConfig {
    const INITIAL_SUPPLY: Balance = 300_000_000_000000000000000000; // 3% of total supply (10^9 + 18 decimals)
    const STASH: Balance =            1_000_000_000000000000000000;
    let endowment: Balance = (INITIAL_SUPPLY - STASH * (initial_authorities.len() as Balance)) /
        (endowed_accounts.len() as Balance);

    GenesisConfig {
        system: Some(SystemConfig {
            code: WASM_BINARY.to_vec(),
            changes_trie_config: Default::default(),
        }),
        balances: Some(BalancesConfig {
            balances: endowed_accounts.iter().cloned()
            .map(|k| (k, endowment))
            .chain(initial_authorities.iter().map(|x| (x.0.clone(), STASH)))
            .collect(),
            vesting: vec![],
        }),
        indices: Some(IndicesConfig {
            ids: endowed_accounts.iter().cloned()
                .chain(initial_authorities.iter().map(|x| x.0.clone()))
                .collect::<Vec<_>>(),
        }),
        session: Some(SessionConfig {
			keys: initial_authorities.iter().map(|x| {
				(x.0.clone(), session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()))
			}).collect::<Vec<_>>(),
		}),
		staking: Some(StakingConfig {
            // The current era index.
			current_era: 0,
            // The ideal number of staking participants.
			validator_count: 50,
            // Minimum number of staking participants before emergency conditions are imposed.
			minimum_validator_count: 2,
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
        sudo: Some(SudoConfig {
            key: root_key,
        }),
        babe: Some(BabeConfig {
            authorities: vec![],
        }),
        im_online: Some(ImOnlineConfig {
			keys: vec![],
        }),
        authority_discovery: Some(AuthorityDiscoveryConfig {
			keys: vec![],
		}),
        grandpa: Some(GrandpaConfig {
            authorities: vec![],
        }),
        treasury: Some(Default::default()),
        // membership_Instance1: Some(Default::default()),
        fees: Some(FeesConfig {
            initial_fees: vec![(
                // anchoring state rent fee per day. TODO Define in a more human friendly way.
                // pre-image: 0xdb4faa73ca6d2016e53c7156087c176b79b169c409b8a0063a07964f3187f9e9
                // hash   : 0x11da6d1f761ddf9bdb4c9d6e5303ebd41f61858d0a5647a1a7bfe089bf921be9
                Hash::from(&[
                    17, 218, 109, 31, 118, 29, 223, 155, 219, 76, 157, 110, 83, 3, 235, 212, 31,
                    97, 133, 141, 10, 86, 71, 161, 167, 191, 224, 137, 191, 146, 27, 233,
                ]),
                // define this based on the expected value of 1 Rad in the given testnet
                // here assuming 1 USD ~ 1 Rad => anchor cost per day = 1nRad (based on state rent sheet =0.0000000008219178082 USD)
                1,
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
