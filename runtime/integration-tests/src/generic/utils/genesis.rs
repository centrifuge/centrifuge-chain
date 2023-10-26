use std::marker::PhantomData;

use cfg_primitives::Balance;
use cfg_types::tokens::CurrencyId;
use codec::Encode;
use frame_support::traits::GenesisBuild;
use sp_runtime::Storage;

use crate::{
	generic::{config::Runtime, utils::currency},
	utils::accounts::default_accounts,
};

pub struct Genesis<T> {
	storage: Storage,
	_config: PhantomData<T>,
}

impl<T> Default for Genesis<T> {
	fn default() -> Self {
		Self {
			storage: Default::default(),
			_config: Default::default(),
		}
	}
}

impl<T: Runtime> Genesis<T> {
	pub fn add(mut self, builder: impl GenesisBuild<T>) -> Self {
		builder.assimilate_storage(&mut self.storage).unwrap();
		self
	}

	pub fn storage(self) -> Storage {
		self.storage
	}
}

pub fn balances<T: Runtime>(balance: Balance) -> impl GenesisBuild<T> {
	pallet_balances::GenesisConfig::<T> {
		balances: default_accounts()
			.into_iter()
			.map(|keyring| (keyring.id(), balance))
			.collect(),
	}
}

pub fn tokens<T: Runtime>(values: Vec<(CurrencyId, Balance)>) -> impl GenesisBuild<T> {
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

pub fn assets<T: Runtime>(currency_ids: Vec<CurrencyId>) -> impl GenesisBuild<T> {
	orml_asset_registry::GenesisConfig::<T> {
		assets: currency_ids
			.into_iter()
			.map(|currency_id| (currency_id, currency::find_metadata(currency_id).encode()))
			.collect(),
		last_asset_id: Default::default(), // It seems deprecated
	}
}
