use std::marker::PhantomData;

use cfg_primitives::{Balance, CFG};
use cfg_types::tokens::{AssetMetadata, CrossChainTransferability, CurrencyId, CustomMetadata};
use codec::Encode;
use frame_support::traits::GenesisBuild;
use sp_runtime::{FixedPointNumber, Storage};

use crate::{generic::config::Runtime, utils::accounts::default_accounts};

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

pub mod currency {
	use cfg_primitives::conversion;

	use super::*;

	pub const fn cfg(amount: Balance) -> Balance {
		amount * CFG
	}

	pub trait CurrencyInfo {
		const ID: CurrencyId;
		const DECIMALS: u32;
		const UNIT: Balance = 10u128.pow(Self::DECIMALS);
		const SYMBOL: &'static str;
		const NAME: &'static str = Self::SYMBOL;
		const LOCATION: Option<xcm::VersionedMultiLocation> = None;
		const CUSTOM: CustomMetadata;
		const ED: Balance = 0;

		fn metadata() -> AssetMetadata<Balance, CustomMetadata> {
			AssetMetadata {
				decimals: Self::DECIMALS,
				name: Self::NAME.as_bytes().to_vec(),
				symbol: Self::SYMBOL.as_bytes().to_vec(),
				existential_deposit: Self::ED,
				location: None,
				additional: CustomMetadata {
					pool_currency: true,
					..Default::default()
				},
			}
		}

		fn fixed_point_as_balance<N: FixedPointNumber<Inner = Balance>>(value: N) -> Balance {
			conversion::fixed_point_to_balance(value, Self::DECIMALS as usize).unwrap()
		}
	}

	pub struct Usd6;
	impl CurrencyInfo for Usd6 {
		const CUSTOM: CustomMetadata = CustomMetadata {
			pool_currency: true,
			..CONST_DEFAULT_CUSTOM
		};
		const DECIMALS: u32 = 6;
		const ID: CurrencyId = CurrencyId::ForeignAsset(1);
		const SYMBOL: &'static str = "USD6";
	}

	pub const fn usd6(amount: Balance) -> Balance {
		amount * Usd6::UNIT
	}

	pub struct Usd12;
	impl CurrencyInfo for Usd12 {
		const CUSTOM: CustomMetadata = CustomMetadata {
			pool_currency: true,
			..CONST_DEFAULT_CUSTOM
		};
		const DECIMALS: u32 = 12;
		const ID: CurrencyId = CurrencyId::ForeignAsset(2);
		const SYMBOL: &'static str = "USD12";
	}

	pub const fn usd12(amount: Balance) -> Balance {
		amount * Usd12::UNIT
	}

	/// Matches default() but for const support
	const CONST_DEFAULT_CUSTOM: CustomMetadata = CustomMetadata {
		transferability: CrossChainTransferability::None,
		mintable: false,
		permissioned: false,
		pool_currency: false,
	};

	pub fn find_metadata(currency: CurrencyId) -> AssetMetadata<Balance, CustomMetadata> {
		match currency {
			Usd6::ID => Usd6::metadata(),
			Usd12::ID => Usd12::metadata(),
			_ => panic!("Unsupported currency {currency:?}"),
		}
	}
}
