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
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

use crate::pallet::{Config, LoanChangeOf, PoolIdOf, PriceResultOf};

const DEFAULT_PRICE_ERR: DispatchError =
	DispatchError::Other("No configured price registry for pallet-loans");

/// Type used to configure the pallet without a price registry
pub struct NoPriceRegistry<T>(PhantomData<T>);

impl<T: Config> DataRegistry<T::PriceId, PoolIdOf<T>> for NoPriceRegistry<T> {
	type Collection = NoPriceCollection<T>;
	type Data = PriceResultOf<T>;
	#[cfg(feature = "runtime-benchmarks")]
	type MaxCollectionSize = sp_runtime::traits::ConstU32<0>;

	fn get(_: &T::PriceId) -> Self::Data {
		Err(DEFAULT_PRICE_ERR)
	}

	fn collection(_: &PoolIdOf<T>) -> Self::Collection {
		NoPriceCollection(PhantomData::default())
	}

	fn register_id(_: &T::PriceId, _: &PoolIdOf<T>) -> DispatchResult {
		Err(DEFAULT_PRICE_ERR)
	}

	fn unregister_id(_: &T::PriceId, _: &PoolIdOf<T>) -> DispatchResult {
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

const DEFAULT_MODIFICATION_ERR: DispatchError =
	DispatchError::Other("No configured modification system for pallet-loans");

/// Type used to configure the pallet without modification support
pub struct NoLoanModifications<T>(PhantomData<T>);

impl<T: Config> ChangeGuard for NoLoanModifications<T> {
	type Change = LoanChangeOf<T>;
	type ChangeId = T::ChangeId;
	type PoolId = PoolIdOf<T>;

	fn note(_: PoolIdOf<T>, _: Self::Change) -> Result<T::ChangeId, DispatchError> {
		Err(DEFAULT_MODIFICATION_ERR)
	}

	fn released(_: PoolIdOf<T>, _: T::ChangeId) -> Result<Self::Change, DispatchError> {
		Err(DEFAULT_MODIFICATION_ERR)
	}
}
