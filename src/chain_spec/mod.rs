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
#![allow(clippy::derive_partial_eq_without_eq)]

pub use altair::{
	algol_config, altair_config, altair_dev, altair_local, antares_config, antares_local,
	charcoal_config, charcoal_local, AltairChainSpec,
};
pub use centrifuge::{
	catalyst_config, catalyst_local, centrifuge_config, centrifuge_dev, centrifuge_local,
	CentrifugeChainSpec,
};
pub use development::{demo, development, development_local, DevelopmentChainSpec};

mod altair;
mod centrifuge;
mod development;

use hex_literal::hex;
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use serde::{Deserialize, Serialize};
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

/// Provides non-production extension for the given parachain id by defaulting
/// to "rococo-local" as relay chain.
fn development_extensions(para_id: u32) -> Extensions {
	Extensions {
		para_id,
		relay_chain: "rococo-local".into(),
	}
}

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
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Extensions> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

pub fn get_altair_session_keys(keys: altair_runtime::AuraId) -> altair_runtime::SessionKeys {
	altair_runtime::SessionKeys {
		aura: keys.clone(),
		block_rewards: keys,
	}
}

type AccountPublic = <cfg_primitives::Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> cfg_primitives::AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn endowed_accounts() -> Vec<cfg_primitives::AccountId> {
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

fn council_members_bootstrap() -> Vec<cfg_primitives::AccountId> {
	endowed_accounts().into_iter().take(4).collect()
}
