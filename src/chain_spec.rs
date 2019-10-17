use babe_primitives::AuthorityId as BabeId;
use centrifuge_chain_runtime::{
    AccountId, BabeConfig, BalancesConfig, FeesConfig, GenesisConfig, GrandpaConfig, Hash,
    IndicesConfig, SudoConfig, SystemConfig, WASM_BINARY,
};
use grandpa_primitives::AuthorityId as GrandpaId;
use hex::FromHex;
use primitives::{Pair, Public};
use substrate_service;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
    /// Whatever the current runtime is, with just Alice as an auth.
    Development,
    /// Whatever the current runtime is, with simple Alice/Bob auths.
    LocalTestnet,
    /// Fulvous testnet with whatever the current runtime is and with Alice/Bob as validators.
    Fulvous,
    /// Amber testnet with whatever the current runtime is and persistent disks and with Alice/Bob as validators.
    Amber,
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

/// Helper function to obtain key from pubkey string
fn get_from_pubkey_hex<TPublic: Public>(pubkey_hex: &str) -> <TPublic::Pair as Pair>::Public {
    <TPublic::Pair as Pair>::Public::from_slice(
        Vec::from_hex(pubkey_hex)
            .expect("a static hex string is valid")
            .as_slice(),
    )
}

/// Helper function to generate stash, controller and session key from seed
pub fn get_authority_keys_from_seed(seed: &str) -> (GrandpaId, BabeId) {
    (
        get_from_seed::<GrandpaId>(seed),
        get_from_seed::<BabeId>(seed),
    )
}

/// Helper function to obtain grandpa and babe keys from pubkey strings
pub fn get_authority_keys_from_pubkey_hex(grandpa: &str, babe: &str) -> (GrandpaId, BabeId) {
    (
        get_from_pubkey_hex::<GrandpaId>(grandpa),
        get_from_pubkey_hex::<BabeId>(babe),
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
                        get_from_seed::<AccountId>("Alice"),
                        vec![
                            get_from_seed::<AccountId>("Alice"),
                            get_from_seed::<AccountId>("Bob"),
                            get_from_seed::<AccountId>("Alice//stash"),
                            get_from_seed::<AccountId>("Bob//stash"),
                        ],
                        true,
                    )
                },
                vec![],
                None,
                None,
                None,
                None,
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
                        get_from_seed::<AccountId>("Alice"),
                        vec![
                            get_from_seed::<AccountId>("Alice"),
                            get_from_seed::<AccountId>("Bob"),
                            get_from_seed::<AccountId>("Charlie"),
                            get_from_seed::<AccountId>("Dave"),
                            get_from_seed::<AccountId>("Eve"),
                            get_from_seed::<AccountId>("Ferdie"),
                            get_from_seed::<AccountId>("Alice//stash"),
                            get_from_seed::<AccountId>("Bob//stash"),
                            get_from_seed::<AccountId>("Charlie//stash"),
                            get_from_seed::<AccountId>("Dave//stash"),
                            get_from_seed::<AccountId>("Eve//stash"),
                            get_from_seed::<AccountId>("Ferdie//stash"),
                        ],
                        true,
                    )
                },
                vec![],
                None,
                None,
                None,
                None,
            ),
            // Fulvous initial spec
            Alternative::Fulvous => {
                ChainSpec::from_genesis(
                    "Fulvous Testnet",
                    "fulvous",
                    || {
                        testnet_genesis(
                        vec![
                            get_authority_keys_from_pubkey_hex("8f9f7766fb5f36aeeed7a05b5676c14ae7c13043e3079b8a850131784b6d15d8",
                                                               "a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"),
                            get_authority_keys_from_pubkey_hex("be1ce959980b786c35e521eebece9d4fe55c41385637d117aa492211eeca7c3d",
                                                               "42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"),
                        ],
                        get_from_pubkey_hex::<AccountId>("c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e"),
                        vec![
                            get_from_pubkey_hex::<AccountId>("c405224448dcd4259816b09cfedbd8df0e6796b16286ea18efa2d6343da5992e")
                        ],
                        true,
                    )
                    },
                    vec![],
                    None,
                    Some("flvs"),
                    None,
                    None,
                )
            }
            // Amber initial spec
            Alternative::Amber => {
                ChainSpec::from_genesis(
                    "Amber Testnet",
                    "amber",
                    || {
                        testnet_genesis(
                        vec![
                            get_authority_keys_from_pubkey_hex("8f9f7766fb5f36aeeed7a05b5676c14ae7c13043e3079b8a850131784b6d15d8",
                                                               "a23153e26c377a172c803e35711257c638e6944ad0c0627db9e3fc63d8503639"),
                            get_authority_keys_from_pubkey_hex("be1ce959980b786c35e521eebece9d4fe55c41385637d117aa492211eeca7c3d",
                                                               "42a6fcd852ef2fe2205de2a3d555e076353b711800c6b59aef67c7c7c1acf04d"),
                        ],
                        get_from_pubkey_hex::<AccountId>("c4051f94a879bd014647993acb2d52c4059a872b6e202e70c3121212416c5842"),
                        vec![
                            get_from_pubkey_hex::<AccountId>("c4051f94a879bd014647993acb2d52c4059a872b6e202e70c3121212416c5842"),
                            get_from_pubkey_hex::<AccountId>("c40526b6cb4c2ab991f5065b599a7313ba98ea6995786539ca05186adb30b34c"),
                            get_from_pubkey_hex::<AccountId>("f0415b8cdfcd189c5636f3c1d0b65637b97fdd926af8132a38f963361f293b0f"),
                            get_from_pubkey_hex::<AccountId>("c40524c8d2a97e347ba3f9c75395dabcad0ef7304c4804838f20ec05ef76b32a"),
                            get_from_pubkey_hex::<AccountId>("f0415a742977038943db5f619a2101d790e8a588ba33d671044a10ea332b9f7f"),
                            get_from_pubkey_hex::<AccountId>("f041601cc759ea533c386a0391344e82b6efb645c07a66355411cbc657aa8c66"),
                            get_from_pubkey_hex::<AccountId>("f04162650738ed2e19b0240419f9680ba9d3dc6b40ccf4ad8993fcbf61ca6720"),
                            get_from_pubkey_hex::<AccountId>("f0415b3730410e05516cbfcdc3eb2909d373dcaf205dc1889f4455d9dc0c7222"),
                            get_from_pubkey_hex::<AccountId>("c4052280dcd37bc6c5148307fda2ade1be9c2d555ec49f59de27c730ca43d80d"),
                            get_from_pubkey_hex::<AccountId>("f04157ad160c8e5c2847f74837b1c59ad6a927bd3feb517a16e12b59a4704c7a"),
                        ],
                        true,
                    )
                    },
                    vec![],
                    None,
                    Some("ambr"),
                    None,
                    None,
                )
            }
        })
    }

    pub(crate) fn from(s: &str) -> Option<Self> {
        match s {
            "dev" => Some(Alternative::Development),
            "" | "local" => Some(Alternative::LocalTestnet),
            "fulvous" => Some(Alternative::Fulvous),
            "amber" => Some(Alternative::Amber),
            _ => None,
        }
    }
}

fn testnet_genesis(
    initial_authorities: Vec<(GrandpaId, BabeId)>,
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
) -> GenesisConfig {
    GenesisConfig {
        system: Some(SystemConfig {
            code: WASM_BINARY.to_vec(),
            changes_trie_config: Default::default(),
        }),
        indices: Some(IndicesConfig {
            ids: endowed_accounts.clone(),
        }),
        balances: Some(BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                // endowed in nano, for 1,000,000,000 Dev (=1,000,000,000 Rad)
                .map(|k| (k, 10_000_000_000_000_000_000))
                .collect(),
            vesting: vec![],
        }),
        sudo: Some(SudoConfig { key: root_key }),
        babe: Some(BabeConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| (x.1.clone(), 1))
                .collect(),
        }),
        grandpa: Some(GrandpaConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| (x.0.clone(), 1))
                .collect(),
        }),
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
