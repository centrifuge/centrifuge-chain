//! PLEASE be as much generic as possible because no domain or use cases are
//! considered at util level and below modules. If you need utilities related to
//! your use case, add them under `cases/<my_use_case.rs>`.
//!
//! Trying to use methods that map to real extrinsic will make easy life to
//! frontend applications, having a source of the real calls they can replicate
//! to simulate some scenarios.
//!
//! Divide this utilities into files when it grows

pub mod currency;
pub mod democracy;
pub mod evm;
pub mod genesis;
pub mod pool;

use cfg_primitives::{AccountId, Balance, CollectionId, ItemId, PoolId, TrancheId};
use cfg_traits::investments::TrancheCurrency as _;
use cfg_types::{
	fixed_point::Ratio,
	oracles::OracleKey,
	tokens::{CurrencyId, TrancheCurrency},
};
use frame_system::RawOrigin;
use pallet_oracle_collection::types::CollectionInfo;
use runtime_common::oracle::Feeder;
use sp_runtime::traits::StaticLookup;

use crate::generic::config::Runtime;

pub const ESSENTIAL: &str =
	"Essential part of the test codebase failed. Assumed infallible under sane circumstances";
fn find_event<T: Runtime, E, R>(f: impl Fn(E) -> Option<R>) -> Option<R>
where
	T::RuntimeEventExt: TryInto<E>,
{
	frame_system::Pallet::<T>::events()
		.into_iter()
		.rev()
		.find_map(|record| record.event.try_into().map(|e| f(e)).ok())
		.flatten()
}

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
	pallet_balances::Pallet::<T>::force_set_balance(
		RawOrigin::Root.into(),
		T::Lookup::unlookup(dest),
		data.free + amount,
	)
	.unwrap();
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

pub fn redeem<T: Runtime>(
	investor: AccountId,
	pool_id: PoolId,
	tranche_id: TrancheId,
	amount: Balance,
) {
	pallet_investments::Pallet::<T>::update_redeem_order(
		RawOrigin::Signed(investor).into(),
		TrancheCurrency::generate(pool_id, tranche_id),
		amount,
	)
	.unwrap();
}

pub fn collect_investments<T: Runtime>(
	investor: AccountId,
	pool_id: PoolId,
	tranche_id: TrancheId,
) {
	pallet_investments::Pallet::<T>::collect_investments(
		RawOrigin::Signed(investor).into(),
		TrancheCurrency::generate(pool_id, tranche_id),
	)
	.unwrap();
}

pub fn collect_redemptions<T: Runtime>(
	investor: AccountId,
	pool_id: PoolId,
	tranche_id: TrancheId,
) {
	pallet_investments::Pallet::<T>::collect_redemptions(
		RawOrigin::Signed(investor).into(),
		TrancheCurrency::generate(pool_id, tranche_id),
	)
	.unwrap();
}

pub fn last_change_id<T: Runtime>() -> T::Hash {
	find_event::<T, _, _>(|e| match e {
		pallet_pool_system::Event::<T>::ProposedChange { change_id, .. } => Some(change_id),
		_ => None,
	})
	.unwrap()
}

pub mod oracle {
	use super::*;

	pub fn feed_from_root<T: Runtime>(key: OracleKey, value: Ratio) {
		pallet_oracle_feed::Pallet::<T>::feed(RawOrigin::Root.into(), key, value).unwrap();
	}

	pub fn update_feeders<T: Runtime>(
		admin: AccountId,
		pool_id: PoolId,
		feeders: impl IntoIterator<Item = Feeder<T::RuntimeOriginExt>>,
	) {
		pallet_oracle_collection::Pallet::<T>::propose_update_collection_info(
			RawOrigin::Signed(admin.clone()).into(),
			pool_id,
			CollectionInfo {
				feeders: pallet_oracle_collection::util::feeders_from(feeders).unwrap(),
				..Default::default()
			},
		)
		.unwrap();

		let change_id = last_change_id::<T>();

		pallet_oracle_collection::Pallet::<T>::apply_update_collection_info(
			RawOrigin::Signed(admin).into(), //or any account
			pool_id,
			change_id,
		)
		.unwrap();
	}

	pub fn update_collection<T: Runtime>(any: AccountId, pool_id: PoolId) {
		pallet_oracle_collection::Pallet::<T>::update_collection(
			RawOrigin::Signed(any).into(),
			pool_id,
		)
		.unwrap();
	}
}
