use cfg_primitives::{Balance, CollectionId, ItemId, Moment, PoolId, TrancheId};
use cfg_traits::investments::TrancheCurrency as TrancheCurrencyT;
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role},
	pools::TrancheMetadata,
	tokens::{AssetMetadata, CurrencyId, CustomMetadata, TrancheCurrency},
};
use frame_support::{
	traits::{GenesisBuild, Get},
	BoundedVec,
};
use frame_system::RawOrigin;
use pallet_pool_system::tranches::{TrancheInput, TrancheLoc, TrancheType};
use sp_core::{sr25519::Pair, Pair as _};
use sp_runtime::{
	traits::{One, StaticLookup},
	AccountId32, Perquintill,
};

use crate::{Config, RuntimeKind};

pub const MUSD_DECIMALS: u32 = 6;
pub const MUSD_UNIT: Balance = 10u128.pow(MUSD_DECIMALS);
pub const MUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(23);
pub const POOL_FUNDS: Balance = 100_000_000 * MUSD_UNIT;
pub const MAX_FUNDED_ACCOUNTS: u8 = 20;

pub const fn account(value: u8) -> AccountId32 {
	AccountId32::new([value; 32])
}

/// This genesis basically do:
/// - ED for any account available
/// - Creates MUSD currency
pub fn genesis<T: Config>() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<T>()
		.unwrap();

	pallet_balances::GenesisConfig::<T> {
		balances: (0..MAX_FUNDED_ACCOUNTS)
			.into_iter()
			.map(|i| (account(i), T::ExistentialDeposit::get()))
			.collect(),
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	orml_tokens::GenesisConfig::<T> {
		balances: (0..MAX_FUNDED_ACCOUNTS)
			.into_iter()
			.map(|i| (account(i), MUSD_CURRENCY_ID, T::ExistentialDeposit::get()))
			.collect(),
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| {
		orml_asset_registry::Pallet::<T>::register_asset(
			RawOrigin::Root.into(),
			AssetMetadata {
				decimals: MUSD_DECIMALS,
				name: "Mock USD".as_bytes().to_vec(),
				symbol: "MUSD".as_bytes().to_vec(),
				existential_deposit: 0,
				location: None,
				additional: CustomMetadata {
					pool_currency: true,
					..Default::default()
				},
			},
			Some(MUSD_CURRENCY_ID),
		)
		.unwrap();
	});
	ext
}

pub fn give_asset_to<T: Config>(
	dest: AccountId32,
	(collection_id, item_id): (CollectionId, ItemId),
) {
	pallet_uniques::Pallet::<T>::force_create(
		RawOrigin::Root.into(),
		collection_id,
		T::Lookup::unlookup(dest.clone()),
		true,
	)
	.unwrap();

	pallet_uniques::Pallet::<T>::mint(
		RawOrigin::Signed(dest.clone()).into(),
		collection_id,
		item_id,
		T::Lookup::unlookup(dest),
	)
	.unwrap()
}

pub fn give_balance_to<T: Config>(dest: AccountId32, amount: Balance) {
	let data = pallet_balances::Account::<T>::get(dest.clone());
	pallet_balances::Pallet::<T>::set_balance(
		RawOrigin::Root.into(),
		T::Lookup::unlookup(dest),
		data.free + amount,
		data.reserved,
	)
	.unwrap();
}

pub fn give_musd_to<T: Config>(dest: AccountId32, amount: Balance) {
	let data = orml_tokens::Accounts::<T>::get(dest.clone(), MUSD_CURRENCY_ID);
	orml_tokens::Pallet::<T>::set_balance(
		RawOrigin::Root.into(),
		T::Lookup::unlookup(dest),
		MUSD_CURRENCY_ID,
		data.free + amount,
		data.reserved,
	)
	.unwrap();
}

pub fn give_investor_role<T: Config>(
	investor: AccountId32,
	pool_id: PoolId,
	tranche_id: TrancheId,
) {
	let role = Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Moment::MAX));
	pallet_permissions::Pallet::<T>::add(
		RawOrigin::Root.into(),
		role,
		investor,
		PermissionScope::Pool(pool_id),
		role,
	)
	.unwrap();
}

pub fn give_borrower_role<T: Config>(borrower: AccountId32, pool_id: PoolId) {
	let role = Role::PoolRole(PoolRole::Borrower);
	pallet_permissions::Pallet::<T>::add(
		RawOrigin::Root.into(),
		role,
		borrower,
		PermissionScope::Pool(pool_id),
		role,
	)
	.unwrap();
}

pub fn create_pool<T: Config>(admin: AccountId32, pool_id: PoolId) {
	pallet_pool_registry::Pallet::<T>::register(
		match T::KIND {
			RuntimeKind::Development => RawOrigin::Signed(admin.clone()).into(),
			_ => RawOrigin::Root.into(),
		},
		admin,
		pool_id,
		vec![
			TrancheInput {
				tranche_type: TrancheType::Residual,
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				},
			},
			TrancheInput {
				tranche_type: TrancheType::NonResidual {
					interest_rate_per_sec: One::one(),
					min_risk_buffer: Perquintill::from_percent(10),
				},
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				},
			},
		],
		MUSD_CURRENCY_ID,
		POOL_FUNDS,
		None,
		BoundedVec::default(),
	)
	.unwrap();
}

pub fn invest<T: Config>(
	investor: AccountId32,
	pool_id: PoolId,
	tranche_id: TrancheId,
	amount: Balance,
) {
	pallet_investments::Pallet::<T>::update_invest_order(
		RawOrigin::Signed(investor).into(),
		TrancheCurrency::generate(pool_id, tranche_id),
		amount,
	)
	.unwrap();
}

// Utilities that does not modify the state
pub mod get {
	use super::*;

	pub fn default_tranche_id<T: Config>(pool_id: PoolId) -> TrancheId {
		pallet_pool_system::Pool::<T>::get(pool_id)
			.unwrap()
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.unwrap()
	}
}
