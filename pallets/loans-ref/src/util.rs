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

use cfg_traits::data::{DataCollection, DataRegistry};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

use crate::pallet::{Config, PoolIdOf, PriceResultOf};

const DEFAULT_ERR: DispatchError =
	DispatchError::Other("No configured price registry for pallet-loans");

pub struct NoPriceRegistry<T>(PhantomData<T>);

impl<T: Config> DataRegistry<T::PriceId, PoolIdOf<T>> for NoPriceRegistry<T> {
	type Collection = NoPriceCollection<T>;
	type Data = PriceResultOf<T>;
	#[cfg(feature = "runtime-benchmarks")]
	type MaxCollectionSize = sp_runtime::traits::ConstU32<0>;

	fn get(_: &T::PriceId) -> Self::Data {
		Err(DEFAULT_ERR)
	}

	fn collection(_: &PoolIdOf<T>) -> Self::Collection {
		NoPriceCollection(PhantomData::default())
	}

	fn register_id(_: &T::PriceId, _: &PoolIdOf<T>) -> DispatchResult {
		Err(DEFAULT_ERR)
	}

	fn unregister_id(_: &T::PriceId, _: &PoolIdOf<T>) -> DispatchResult {
		Err(DEFAULT_ERR)
	}
}

pub struct NoPriceCollection<T>(PhantomData<T>);

impl<T: Config> DataCollection<T::PriceId> for NoPriceCollection<T> {
	type Data = PriceResultOf<T>;

	fn get(&self, _: &T::PriceId) -> Self::Data {
		Err(DEFAULT_ERR)
	}
}
