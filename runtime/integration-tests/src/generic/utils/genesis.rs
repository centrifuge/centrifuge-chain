//! PLEASE be as much generic as possible because no domain or use cases are
//! considered at this level.

use cfg_primitives::Balance;
use cfg_types::tokens::CurrencyId;
use parity_scale_codec::Encode;
use sp_core::crypto::AccountId32;
use sp_runtime::{BuildStorage, Storage};

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

// Add BuildStorage functions for pallet initialization.

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

pub fn assets<T: Runtime>(currency_ids: Vec<Box<dyn CurrencyInfo>>) -> impl BuildStorage {
	orml_asset_registry::GenesisConfig::<T> {
		assets: currency_ids
			.into_iter()
			.map(|currency_id| (currency_id.id(), currency_id.metadata().encode()))
			.collect(),
		last_asset_id: Default::default(), // It seems deprecated
	}
}

pub fn council_members<T, I>(members: Vec<Keyring>) -> impl BuildStorage
where
	I: 'static,
	T: pallet_collective::Config<I>,
	T::AccountId: From<AccountId32>,
{
	pallet_collective::GenesisConfig::<T, I> {
		phantom: Default::default(),
		members: members
			.into_iter()
			.map(|acc| acc.to_account_id().into())
			.collect(),
	}
}
