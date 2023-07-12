// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
use altair_runtime::constants::currency::{AIR, MILLI_AIR};
use cfg_primitives::currency_decimals;
use cfg_types::{fee_keys::FeeKey, tokens::CurrencyId};
use cumulus_primitives_core::ParaId;
use runtime_common::account_conversion::AccountConverter;
use sc_service::{ChainType, Properties};
use sp_core::sr25519;

use super::*;

/// Specialized `ChainSpec` instances for our runtimes.
pub type AltairChainSpec = sc_service::GenericChainSpec<altair_runtime::GenesisConfig, Extensions>;

pub fn altair_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(
		&include_bytes!("../../res/genesis/altair-genesis-spec-raw.json")[..],
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
					get_from_seed::<altair_runtime::AuraId>("Alice"),
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
	AltairChainSpec::from_json_bytes(&include_bytes!("../../res/antares-spec-raw.json")[..])
		.unwrap()
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
					get_from_seed::<altair_runtime::AuraId>("Alice"),
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

pub fn algol_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../../res/algol-spec.json")[..]).unwrap()
}

pub fn charcoal_config() -> AltairChainSpec {
	AltairChainSpec::from_json_bytes(&include_bytes!("../../res/charcoal-spec-raw.json")[..])
		.unwrap()
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
					get_from_seed::<altair_runtime::AuraId>("Alice"),
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

fn altair_genesis(
	initial_authorities: Vec<(altair_runtime::AccountId, altair_runtime::AuraId)>,
	mut endowed_accounts: Vec<altair_runtime::AccountId>,
	endowed_evm_accounts: Vec<([u8; 20], Option<u64>)>,
	total_issuance: Option<altair_runtime::Balance>,
	id: ParaId,
	council_members: Vec<altair_runtime::AccountId>,
) -> altair_runtime::GenesisConfig {
	let chain_id: u32 = id.into();

	endowed_accounts.extend(endowed_evm_accounts.into_iter().map(|(addr, id)| {
		let chain_id = id.unwrap_or_else(|| chain_id.into());
		AccountConverter::<centrifuge_runtime::Runtime>::convert_evm_address(chain_id, addr)
	}));

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
			total_reward: 98_630 * MILLI_AIR * 100,
		},
		block_rewards_base: altair_runtime::BlockRewardsBaseConfig {
			currency_id: CurrencyId::Native,
			amount: altair_runtime::ExistentialDeposit::get(),
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
		interest_accrual: Default::default(),
		base_fee: Default::default(),
		evm_chain_id: development_runtime::EVMChainIdConfig {
			chain_id: chain_id.into(),
		},
		ethereum: Default::default(),
		evm: Default::default(),
	}
}
