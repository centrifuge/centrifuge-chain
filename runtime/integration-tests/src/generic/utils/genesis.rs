//! PLEASE be as much generic as possible because no domain or use cases are
//! considered at this level.

use std::marker::PhantomData;

use cfg_primitives::Balance;
use cfg_types::tokens::CurrencyId;
use frame_support::traits::GenesisBuild;
use parity_scale_codec::Encode;
use sp_core::crypto::AccountId32;
use sp_runtime::Storage;

use crate::{
	generic::{config::Runtime, utils::currency::CurrencyInfo},
	utils::accounts::{default_accounts, Keyring},
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
	pub fn add<I: 'static>(mut self, builder: impl GenesisBuild<T, I>) -> Self {
		builder.assimilate_storage(&mut self.storage).unwrap();
		self
	}

	pub fn storage(self) -> Storage {
		self.storage
	}
}

// Add GenesisBuild functions for pallet initialization.

pub fn balances<T: Runtime>(balance: Balance) -> impl GenesisBuild<T> {
	let mut accounts = Vec::new();
	accounts.extend(default_accounts().into_iter().map(|k| (k.id(), balance)));
	accounts.extend(
		default_accounts()
			.into_iter()
			.map(|k| (k.id_ecdsa::<T>(), balance)),
	);
	accounts.extend(
		default_accounts()
			.into_iter()
			.map(|k| (k.id_ed25519(), balance)),
	);

	pallet_balances::GenesisConfig::<T> { balances: accounts }
}

pub fn tokens<T: Runtime>(values: Vec<(CurrencyId, Balance)>) -> impl GenesisBuild<T> {
	let mut accounts = Vec::new();
	accounts.extend(
		default_accounts()
			.into_iter()
			.map(|keyring| {
				values
					.clone()
					.into_iter()
					.map(|(curency_id, balance)| (keyring.id(), curency_id, balance))
					.collect::<Vec<_>>()
			})
			.flatten(),
	);
	accounts.extend(
		default_accounts()
			.into_iter()
			.map(|keyring| {
				values
					.clone()
					.into_iter()
					.map(|(curency_id, balance)| (keyring.id_ecdsa::<T>(), curency_id, balance))
					.collect::<Vec<_>>()
			})
			.flatten(),
	);
	accounts.extend(
		default_accounts()
			.into_iter()
			.map(|keyring| {
				values
					.clone()
					.into_iter()
					.map(|(curency_id, balance)| (keyring.id_ed25519(), curency_id, balance))
					.collect::<Vec<_>>()
			})
			.flatten(),
	);

	orml_tokens::GenesisConfig::<T> { balances: accounts }
}

pub fn assets<T: Runtime>(currency_ids: Vec<Box<dyn CurrencyInfo>>) -> impl GenesisBuild<T> {
	orml_asset_registry::GenesisConfig::<T> {
		assets: currency_ids
			.into_iter()
			.map(|currency_id| (currency_id.id(), currency_id.metadata().encode()))
			.collect(),
		last_asset_id: Default::default(), // It seems deprecated
	}
}

pub fn council_members<T, I>(members: Vec<Keyring>) -> impl GenesisBuild<T, I>
where
	I: 'static,
	T: pallet_collective::Config<I>,
	T::AccountId: From<AccountId32>,
{
	pallet_collective::GenesisConfig::<T, I> {
		phantom: Default::default(),
		members: members.into_iter().map(|acc| acc.id().into()).collect(),
	}
}
