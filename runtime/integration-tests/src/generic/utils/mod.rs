// Divide this utilties into files when it grows

use cfg_primitives::{AccountId, Balance, CollectionId, ItemId, PoolId, TrancheId};
use cfg_traits::investments::TrancheCurrency as _;
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role},
	tokens::{CurrencyId, TrancheCurrency},
};
use frame_system::RawOrigin;
use sp_runtime::traits::StaticLookup;
pub mod genesis;

use cfg_types::pools::TrancheMetadata;
use frame_support::BoundedVec;
use pallet_pool_system::tranches::{TrancheInput, TrancheLoc, TrancheType};
use sp_runtime::{traits::One, AccountId32, Perquintill};

use crate::generic::runtime::{Runtime, RuntimeKind};

pub fn give_nft_to<T: Runtime>(dest: AccountId, (collection_id, item_id): (CollectionId, ItemId)) {
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

pub fn give_balance_to<T: Runtime>(dest: AccountId, amount: Balance) {
	let data = pallet_balances::Account::<T>::get(dest.clone());
	pallet_balances::Pallet::<T>::set_balance(
		RawOrigin::Root.into(),
		T::Lookup::unlookup(dest),
		data.free + amount,
		data.reserved,
	)
	.unwrap();
}

pub fn give_token_to<T: Runtime>(dest: AccountId, currency_id: CurrencyId, amount: Balance) {
	let data = orml_tokens::Accounts::<T>::get(dest.clone(), currency_id);
	orml_tokens::Pallet::<T>::set_balance(
		RawOrigin::Root.into(),
		T::Lookup::unlookup(dest),
		currency_id,
		data.free + amount,
		data.reserved,
	)
	.unwrap();
}

pub fn give_pool_role<T: Runtime>(dest: AccountId, pool_id: PoolId, role: PoolRole) {
	pallet_permissions::Pallet::<T>::add(
		RawOrigin::Root.into(),
		Role::PoolRole(role),
		dest,
		PermissionScope::Pool(pool_id),
		Role::PoolRole(role),
	)
	.unwrap();
}

pub fn create_empty_pool<T: Runtime>(admin: AccountId32, pool_id: PoolId, currency_id: CurrencyId) {
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
					min_risk_buffer: Perquintill::from_percent(0),
				},
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				},
			},
		],
		currency_id,
		Balance::MAX,
		None,
		BoundedVec::default(),
	)
	.unwrap();
}

pub fn close_pool_epoch<T: Runtime>(admin: AccountId32, pool_id: PoolId) {
	pallet_pool_system::Pallet::<T>::close_epoch(RawOrigin::Signed(admin.clone()).into(), pool_id)
		.unwrap();
}

pub fn invest<T: Runtime>(
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

	pub fn default_tranche_id<T: Runtime>(pool_id: PoolId) -> TrancheId {
		pallet_pool_system::Pool::<T>::get(pool_id)
			.unwrap()
			.tranches
			.tranche_id(TrancheLoc::Index(0))
			.unwrap()
	}
}
