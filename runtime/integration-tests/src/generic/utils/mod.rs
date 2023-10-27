// Divide this utilties into files when it grows

use cfg_primitives::{AccountId, Balance, CollectionId, ItemId, PoolId, TrancheId};
use cfg_traits::{investments::TrancheCurrency as _, Seconds};
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role},
	tokens::{CurrencyId, TrancheCurrency},
};
use frame_system::RawOrigin;
use sp_runtime::traits::StaticLookup;

pub mod genesis;

use cfg_types::pools::TrancheMetadata;
use frame_support::{traits::fungible::Mutate, BoundedVec};
use pallet_pool_system::tranches::{TrancheInput, TrancheType};
use sp_runtime::{traits::One, Perquintill};

use crate::generic::runtime::{Runtime, RuntimeKind};

pub const POOL_MIN_EPOCH_TIME: Seconds = 24;

pub fn give_nft<T: Runtime>(dest: AccountId, (collection_id, item_id): (CollectionId, ItemId)) {
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

pub fn give_balance<T: Runtime>(dest: AccountId, amount: Balance) {
	let data = pallet_balances::Account::<T>::get(dest.clone());
	pallet_balances::Pallet::<T>::set_balance(&dest, data.free + amount);
}

pub fn give_tokens<T: Runtime>(dest: AccountId, currency_id: CurrencyId, amount: Balance) {
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

pub fn create_empty_pool<T: Runtime>(admin: AccountId, pool_id: PoolId, currency_id: CurrencyId) {
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

	// In order to later close the epoch fastly,
	// we mofify here that requirement to significalty reduce the testing time.
	// The only way to do it is breaking the integration tests rules mutating
	// this state directly.
	pallet_pool_system::Pool::<T>::mutate(pool_id, |pool| {
		pool.as_mut().unwrap().parameters.min_epoch_time = POOL_MIN_EPOCH_TIME;
	});
}

pub fn close_pool_epoch<T: Runtime>(admin: AccountId, pool_id: PoolId) {
	pallet_pool_system::Pallet::<T>::close_epoch(RawOrigin::Signed(admin.clone()).into(), pool_id)
		.unwrap();
}

pub fn invest<T: Runtime>(
	investor: AccountId,
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
