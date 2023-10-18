use std::{collections::BTreeMap, marker::PhantomData};

use cfg_primitives::Balance;
use cfg_types::tokens::{AssetMetadata, CurrencyId, CustomMetadata};
use codec::Encode;
use frame_support::traits::GenesisBuild;
use sp_runtime::Storage;

use crate::{generic::runtime::Runtime, utils::accounts::default_accounts};

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

pub const USD6_DECIMALS: u32 = 6;
pub const USD6_UNIT: Balance = 10u128.pow(USD6_DECIMALS);
pub const USD6_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

pub const USD12_DECIMALS: u32 = 12;
pub const USD12_UNIT: Balance = 10u128.pow(USD12_DECIMALS);
pub const USD12_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2);

pub fn assets<T: Runtime>(currency_ids: Vec<CurrencyId>) -> impl GenesisBuild<T> {
	let assets = BTreeMap::from([
		(
			USD6_CURRENCY_ID,
			AssetMetadata {
				decimals: USD6_DECIMALS,
				name: "Mock Dollar with 6 decimals".as_bytes().to_vec(),
				symbol: "USD6".as_bytes().to_vec(),
				existential_deposit: 0 as Balance,
				location: None,
				additional: CustomMetadata {
					pool_currency: true,
					..Default::default()
				},
			}
			.encode(),
		),
		(
			USD12_CURRENCY_ID,
			AssetMetadata {
				decimals: USD12_DECIMALS,
				name: "Mock Dollar 12 with decimals".as_bytes().to_vec(),
				symbol: "USD12".as_bytes().to_vec(),
				existential_deposit: 0 as Balance,
				location: None,
				additional: CustomMetadata {
					pool_currency: true,
					..Default::default()
				},
			}
			.encode(),
		),
		// Add new currencies here
	]);

	orml_asset_registry::GenesisConfig::<T> {
		assets: dbg!(currency_ids
			.into_iter()
			.map(|id| (id, assets.get(&id).unwrap().clone()))
			.collect()),
		last_asset_id: Default::default(), // It seems deprecated
	}
}
