// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// Divide this utilities into files when it grows

pub mod accounts;
pub mod currency;
pub mod evm;
pub mod genesis;
pub mod logs;
pub mod pool;
pub mod tokens;
pub mod xcm;

use cfg_primitives::{AccountId, Balance, CollectionId, ItemId, PoolId, TrancheId};
use cfg_traits::{Seconds, TimeAsSecs};
use cfg_types::{fixed_point::Ratio, oracles::OracleKey, tokens::CurrencyId};
use frame_system::RawOrigin;
use pallet_oracle_collection::types::CollectionInfo;
use runtime_common::oracle::Feeder;
use sp_runtime::traits::StaticLookup;

use crate::{
	config::Runtime,
	utils::{accounts::Keyring, pool::close_epoch},
};

pub mod orml_asset_registry {
	// orml_asset_registry has remove the reexport of all pallet stuff,
	// we reexport it again here
	pub use orml_asset_registry::module::*;
}

pub mod approx {
	use std::fmt;

	use cfg_primitives::Balance;

	#[derive(Clone)]
	pub struct Approximation {
		value: Balance,
		offset: Balance,
		is_positive: bool,
	}

	impl PartialEq<Approximation> for Balance {
		fn eq(&self, ap: &Approximation) -> bool {
			match ap.is_positive {
				true => *self <= ap.value && *self + ap.offset >= ap.value,
				false => *self >= ap.value && *self - ap.offset <= ap.value,
			}
		}
	}

	impl fmt::Debug for Approximation {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			let (from, to) = match self.is_positive {
				true => (self.value - self.offset, self.value),
				false => (self.value, self.value + self.offset),
			};

			write!(f, "Approximation: [{}, {}]", from, to)
		}
	}

	/// Allow to compare `Balance` values with approximated values:
	pub trait Approximate {
		fn approx(&self, variation: f64) -> Approximation;
	}

	impl Approximate for Balance {
		fn approx(&self, variation: f64) -> Approximation {
			let offset = match variation >= 0.0 {
				true => (*self as f64 * variation) as Balance,
				false => (*self as f64 * -variation) as Balance,
			};

			Approximation {
				value: *self,
				offset,
				is_positive: variation >= 0.0,
			}
		}
	}

	#[test]
	fn approximations() {
		assert_eq!(1000u128, 996.approx(-0.01));
		assert_eq!(1000u128, 1004.approx(0.01));
		assert_eq!(1000u128, 1500.approx(0.5));

		assert_ne!(1000u128, 996.approx(0.01));
		assert_ne!(1000u128, 1004.approx(-0.01));
		assert_ne!(1000u128, 1500.approx(0.1));
	}
}

pub const ESSENTIAL: &str =
	"Essential part of the test codebase failed. Assumed infallible under sane circumstances";

pub fn now_secs<T: Runtime>() -> Seconds {
	<pallet_timestamp::Pallet<T> as TimeAsSecs>::now()
}

pub fn find_event<T: Runtime, E, R>(f: impl Fn(E) -> Option<R>) -> Option<R>
where
	T::RuntimeEventExt: TryInto<E>,
{
	frame_system::Pallet::<T>::events()
		.into_iter()
		.rev()
		.find_map(|record| record.event.try_into().map(|e| f(e)).ok())
		.flatten()
}

pub fn last_event<T: Runtime, E>() -> E
where
	T::RuntimeEventExt: TryInto<E>,
{
	frame_system::Pallet::<T>::events()
		.pop()
		.unwrap()
		.event
		.try_into()
		.ok()
		.unwrap()
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
		(pool_id, tranche_id),
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
		(pool_id, tranche_id),
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
		(pool_id, tranche_id),
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
		(pool_id, tranche_id),
	)
	.unwrap();
}

pub fn invest_and_collect<T: Runtime>(
	investor: AccountId,
	admin: Keyring,
	pool_id: PoolId,
	tranche_id: TrancheId,
	amount: Balance,
) {
	invest::<T>(investor.clone(), pool_id, tranche_id, amount);
	close_epoch::<T>(admin.into(), pool_id);
	collect_investments::<T>(investor, pool_id, tranche_id);
}

pub fn last_change_id<T: Runtime>() -> T::Hash {
	find_event::<T, _, _>(|e| match e {
		pallet_pool_system::Event::<T>::ProposedChange { change_id, .. } => Some(change_id),
		_ => None,
	})
	.unwrap()
}

pub mod oracle {
	use frame_support::traits::OriginTrait;

	use super::*;

	pub fn set_order_book_feeder<T: Runtime>(origin: T::RuntimeOriginExt) {
		pallet_order_book::Pallet::<T>::set_market_feeder(
			T::RuntimeOriginExt::root(),
			Feeder(origin.into_caller()),
		)
		.unwrap()
	}

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
