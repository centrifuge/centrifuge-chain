// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::{
	changes::ChangeGuard,
	data::{DataCollection, DataRegistry},
};
use orml_traits::{DataFeeder, DataProvider};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

use crate::{
	entities::changes::Change,
	pallet::{Config, PriceResultOf},
};

const DEFAULT_PRICE_ERR: DispatchError =
	DispatchError::Other("No configured price registry for pallet-loans");

/// Type used to configure the pallet without a price registry
pub struct NoPriceRegistry<T>(PhantomData<T>);

impl<T: Config> DataRegistry<T::PriceId, T::PoolId> for NoPriceRegistry<T> {
	type Collection = NoPriceCollection<T>;
	type Data = PriceResultOf<T>;
	#[cfg(feature = "runtime-benchmarks")]
	type MaxCollectionSize = sp_runtime::traits::ConstU32<0>;

	fn get(_: &T::PriceId) -> Self::Data {
		Err(DEFAULT_PRICE_ERR)
	}

	fn collection(_: &T::PoolId) -> Self::Collection {
		NoPriceCollection(PhantomData::default())
	}

	fn register_id(_: &T::PriceId, _: &T::PoolId) -> DispatchResult {
		Err(DEFAULT_PRICE_ERR)
	}

	fn unregister_id(_: &T::PriceId, _: &T::PoolId) -> DispatchResult {
		Err(DEFAULT_PRICE_ERR)
	}
}

impl<T: Config> DataProvider<T::PriceId, T::Rate> for NoPriceRegistry<T> {
	fn get(_: &T::PriceId) -> Option<T::Rate> {
		None
	}
}

impl<T: Config> DataFeeder<T::PriceId, T::Rate, T::AccountId> for NoPriceRegistry<T> {
	fn feed_value(_: T::AccountId, _: T::PriceId, _: T::Rate) -> DispatchResult {
		Err(DEFAULT_PRICE_ERR)
	}
}

pub struct NoPriceCollection<T>(PhantomData<T>);

impl<T: Config> DataCollection<T::PriceId> for NoPriceCollection<T> {
	type Data = PriceResultOf<T>;

	fn get(&self, _: &T::PriceId) -> Self::Data {
		Err(DEFAULT_PRICE_ERR)
	}
}

const DEFAULT_CHANGE_ERR: DispatchError =
	DispatchError::Other("No configured change system for pallet-loans");

/// Type used to configure the pallet without changes support
pub struct NoLoanChanges<T>(PhantomData<T>);

impl<T: Config> ChangeGuard for NoLoanChanges<T> {
	type Change = Change<T>;
	type ChangeId = T::Hash;
	type PoolId = T::PoolId;

	fn note(_: T::PoolId, _: Self::Change) -> Result<Self::ChangeId, DispatchError> {
		Err(DEFAULT_CHANGE_ERR)
	}

	fn released(_: T::PoolId, _: Self::ChangeId) -> Result<Self::Change, DispatchError> {
		Err(DEFAULT_CHANGE_ERR)
	}
}
