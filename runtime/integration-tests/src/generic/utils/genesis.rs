//! PLEASE be as much generic as possible because no domain or use cases are
//! considered at this level.

use cfg_primitives::Balance;
use cfg_types::{fixed_point::Rate, tokens::CurrencyId};
use parity_scale_codec::Encode;
use sp_core::Get;
use sp_runtime::{BuildStorage, FixedPointNumber, Storage};

use crate::{
	generic::{config::Runtime, utils::currency::CurrencyInfo},
	utils::accounts::{default_accounts, Keyring},
};

#[derive(Default)]
pub struct Genesis {
	storage: Storage,
}

impl Genesis {
	pub fn add(mut self, builder: impl BuildStorage) -> Self {
		builder.assimilate_storage(&mut self.storage).unwrap();
		self
	}

	pub fn storage(self) -> Storage {
		self.storage
	}
}

pub fn balances<T: Runtime>(balance: Balance) -> impl BuildStorage {
	pallet_balances::GenesisConfig::<T> {
		balances: default_accounts()
			.into_iter()
			.map(|keyring| (keyring.id(), balance))
			.collect(),
	}
}

pub fn tokens<T: Runtime>(values: Vec<(CurrencyId, Balance)>) -> impl BuildStorage {
	orml_tokens::GenesisConfig::<T> {
		balances: default_accounts()
			.into_iter()
			.map(|keyring| {
				values
					.clone()
					.into_iter()
					.map(|(curency_id, balance)| (keyring.id(), curency_id, balance))
					.collect::<Vec<_>>()
			})
			.flatten()
			.collect(),
	}
}

pub fn assets<T: Runtime>(currency_ids: Vec<&dyn CurrencyInfo>) -> impl BuildStorage {
	orml_asset_registry::module::GenesisConfig::<T> {
		assets: currency_ids
			.into_iter()
			.map(|currency_id| (currency_id.id(), currency_id.metadata().encode()))
			.collect(),
		last_asset_id: Default::default(), // It seems deprecated
	}
}

pub fn council_members<T: Runtime>(members: Vec<Keyring>) -> impl BuildStorage {
	pallet_collective::GenesisConfig::<T, cfg_primitives::CouncilCollective> {
		phantom: Default::default(),
		members: members.into_iter().map(|acc| acc.id()).collect(),
	}
}

pub fn invulnerables<T: Runtime>(invulnerables: Vec<Keyring>) -> impl BuildStorage {
	pallet_collator_selection::GenesisConfig::<T> {
		invulnerables: invulnerables.into_iter().map(|acc| acc.id()).collect(),
		candidacy_bond: cfg_primitives::MILLI_CFG,
		desired_candidates: T::MaxCandidates::get(),
	}
}

pub fn session_keys<T: Runtime>() -> impl BuildStorage {
	pallet_session::GenesisConfig::<T> {
		keys: default_accounts()
			.into_iter()
			.map(|acc| (acc.id(), acc.id(), T::initialize_session_keys(acc.public())))
			.collect(),
	}
}

pub fn block_rewards<T: Runtime>(collators: Vec<Keyring>) -> impl BuildStorage {
	pallet_block_rewards::GenesisConfig::<T> {
		collators: collators.into_iter().map(|acc| acc.id()).collect(),
		collator_reward: (1000 * cfg_primitives::CFG).into(),
		treasury_inflation_rate: Rate::saturating_from_rational(3, 100).into(),
		last_update: Default::default(),
	}
}
