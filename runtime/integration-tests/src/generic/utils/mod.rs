// Divide this utilties into files when they grow

use cfg_primitives::{Balance, CollectionId, ItemId, Moment, PoolId, TrancheId};
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role},
	tokens::CurrencyId,
};
use frame_system::RawOrigin;
use sp_runtime::{traits::StaticLookup, AccountId32};
pub mod genesis;

use crate::generic::runtime::Runtime;

pub fn give_nft_to<T: Runtime>(
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

pub fn give_balance_to<T: Runtime>(dest: AccountId32, amount: Balance) {
	let data = pallet_balances::Account::<T>::get(dest.clone());
	pallet_balances::Pallet::<T>::set_balance(
		RawOrigin::Root.into(),
		T::Lookup::unlookup(dest),
		data.free + amount,
		data.reserved,
	)
	.unwrap();
}

pub fn give_token_to<T: Runtime>(dest: AccountId32, currency_id: CurrencyId, amount: Balance) {
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

pub fn give_investor_role<T: Runtime>(
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

pub fn give_borrower_role<T: Runtime>(borrower: AccountId32, pool_id: PoolId) {
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
