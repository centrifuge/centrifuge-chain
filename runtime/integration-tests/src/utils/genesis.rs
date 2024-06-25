//! PLEASE be as much generic as possible because no domain or use cases are
//! considered at this level.

use cfg_primitives::{AccountId, Balance};
use cfg_types::{
	fixed_point::Rate,
	tokens::{AssetMetadata, CurrencyId},
};
use parity_scale_codec::Encode;
use sp_core::Get;
use sp_runtime::{BuildStorage, FixedPointNumber, Storage};

use crate::{
	config::Runtime,
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

pub fn balances<T: pallet_balances::Config<AccountId = AccountId, Balance = Balance>>(
	balance: Balance,
) -> impl BuildStorage {
	pallet_balances::GenesisConfig::<T> {
		balances: default_accounts()
			.into_iter()
			.map(Keyring::id)
			.chain(default_accounts().into_iter().map(Keyring::id_ed25519))
			.map(|id| (id, balance))
			.collect(),
	}
}

pub fn tokens<T: Runtime>(
	values: impl IntoIterator<Item = (CurrencyId, Balance)> + Clone,
) -> impl BuildStorage {
	orml_tokens::GenesisConfig::<T> {
		balances: default_accounts()
			.into_iter()
			.map(Keyring::id)
			.chain(default_accounts().into_iter().map(Keyring::id_ed25519))
			.flat_map(|account_id| {
				values
					.clone()
					.into_iter()
					.map(|(curency_id, balance)| (account_id.clone(), curency_id, balance))
					.collect::<Vec<_>>()
			})
			.collect(),
	}
}

pub fn assets<'a, T: Runtime>(
	currency_ids: impl IntoIterator<Item = (CurrencyId, &'a AssetMetadata)>,
) -> impl BuildStorage {
	orml_asset_registry::module::GenesisConfig::<T> {
		assets: currency_ids
			.into_iter()
			.map(|(currency_id, metadata)| (currency_id, metadata.encode()))
			.collect(),
		last_asset_id: Default::default(), // It seems deprecated
	}
}

pub fn council_members<T: Runtime>(
	members: impl IntoIterator<Item = Keyring>,
) -> impl BuildStorage {
	pallet_collective::GenesisConfig::<T, cfg_primitives::CouncilCollective> {
		phantom: Default::default(),
		members: members.into_iter().map(|acc| acc.id().into()).collect(),
	}
}

pub fn invulnerables<T: Runtime>(
	invulnerables: impl IntoIterator<Item = Keyring>,
) -> impl BuildStorage {
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
			.map(|acc| (acc.id(), acc.id(), T::initialize_session_keys(acc.into())))
			.collect(),
	}
}

pub fn block_rewards<T: Runtime>(
	collators: impl IntoIterator<Item = Keyring>,
) -> impl BuildStorage {
	pallet_block_rewards::GenesisConfig::<T> {
		collators: collators.into_iter().map(|acc| acc.id()).collect(),
		collator_reward: (1000 * cfg_primitives::CFG).into(),
		treasury_inflation_rate: Rate::saturating_from_rational(3, 100).into(),
		last_update: Default::default(),
	}
}

pub fn parachain_id<T: Runtime>(para_id: u32) -> impl BuildStorage {
	staging_parachain_info::GenesisConfig::<T> {
		_config: Default::default(),
		parachain_id: para_id.into(),
	}
}
