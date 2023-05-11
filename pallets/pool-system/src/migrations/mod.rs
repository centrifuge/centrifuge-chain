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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{storage_alias, traits::OnRuntimeUpgrade, weights::Weight, Blake2_128Concat};

use crate::*;

pub mod v0 {
	use cfg_types::epoch::EpochState;

	pub use super::pool_types::*;
	use super::*;

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct PoolDetails<
		CurrencyId,
		TrancheCurrency,
		EpochId,
		Balance,
		Rate,
		MetaSize,
		Weight,
		TrancheId,
		PoolId,
		MaxTranches,
	> where
		MetaSize: Get<u32>,
		Rate: FixedPointNumber<Inner = Balance>,
		Balance: FixedPointOperand,
		MaxTranches: Get<u32>,
	{
		/// Currency that the pool is denominated in (immutable).
		pub currency: CurrencyId,
		/// List of tranches, ordered junior to senior.
		pub tranches:
			Tranches<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId, MaxTranches>,
		/// Details about the parameters of the pool.
		pub parameters: PoolParameters,
		/// Metadata that specifies the pool.
		pub metadata: Option<BoundedVec<u8, MetaSize>>,
		/// The status the pool is currently in.
		pub status: PoolStatus,
		/// Details about the epochs of the pool.
		pub epoch: EpochState<EpochId>,
		/// Details about the reserve (unused capital) in the pool.
		pub reserve: ReserveDetails<Balance>,
	}

	pub type PoolDetailsOf<T, MetaSize> = PoolDetails<
		<T as Config>::CurrencyId,
		<T as Config>::TrancheCurrency,
		<T as Config>::EpochId,
		<T as Config>::Balance,
		<T as Config>::Rate,
		MetaSize,
		<T as Config>::TrancheWeight,
		<T as Config>::TrancheId,
		<T as Config>::PoolId,
		<T as Config>::MaxTranches,
	>;

	#[storage_alias]
	pub type Pool<T: Config, MetaSize: Get<u32>> =
		StorageMap<Pallet<T>, Blake2_128Concat, <T as Config>::PoolId, PoolDetailsOf<T, MetaSize>>;
}

pub mod v1 {
	use super::*;

	pub struct Migration<T, M>(
		sp_std::marker::PhantomData<T>,
		sp_std::marker::PhantomData<M>,
	);

	impl<T: Config, M: Get<u32> + 'static> OnRuntimeUpgrade for Migration<T, M> {
		fn on_runtime_upgrade() -> Weight {
			let version = StorageVersion::<T>::get();
			if version != Release::V0 {
				log::warn!("Skipping interest_accrual migration: Storage is at incorrect version");
				return T::DbWeight::get().reads(1);
			}
			let mut count = 1; // start at 1 for the version read/write
			Pool::<T>::translate_values(
				|v0::PoolDetailsOf::<T, M> {
				     currency,
				     tranches,
				     parameters,
				     status,
				     epoch,
				     reserve,
				     ..
				 }: v0::PoolDetailsOf<T, M>| {
					count += 1;
					Some(PoolDetailsOf::<T> {
						currency,
						tranches,
						parameters,
						status,
						epoch,
						reserve,
					})
				},
			);
			StorageVersion::<T>::set(Release::V1);
			T::DbWeight::get().reads_writes(count, count)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			let version = StorageVersion::<T>::get();
			let old_pools: Option<Vec<_>> = if version == Release::V0 {
				Some(v0::Pool::<T, M>::iter().collect())
			} else {
				None
			};

			Ok(old_pools.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
			let old_pools =
				Option::<Vec<(<T as Config>::PoolId, v0::PoolDetailsOf<T, M>)>>::decode(
					&mut state.as_ref(),
				)
				.map_err(|_| "Error decoding pre-upgrade state")?;

			let new_pools: Vec<_> = Pool::<T>::iter().collect();
			for (key, val) in old_pools.into_iter().flatten() {
				let v0::PoolDetailsOf::<T, M> {
					currency,
					tranches,
					parameters,
					status,
					epoch,
					reserve,
					..
				} = val;
				let old_val = PoolDetailsOf::<T> {
					currency,
					tranches,
					parameters,
					status,
					epoch,
					reserve,
				};
				let (_, new_val) = new_pools
					.iter()
					.find(|(k, _)| *k == key)
					.ok_or("Could not find old value in new storage")?;
				if old_val != *new_val {
					return Err("New pool details do not match old pool details");
				}
			}
			Ok(())
		}
	}
}

#[cfg(test)]
mod test {
	use cfg_types::{epoch::EpochState, tokens::CurrencyId};
	use sp_runtime::traits::ConstU32;

	pub use super::pool_types::*;
	use super::*;
	use crate::mock::*;

	#[cfg(feature = "try-runtime")]
	#[test]
	fn migrate_to_v2() {
		type MetaSize = ConstU32<0>;
		new_test_ext().execute_with(|| {
			v0::Pool::<Runtime, MetaSize>::insert(
				1701,
				v0::PoolDetails {
					currency: CurrencyId::AUSD,
					tranches: Tranches::new(1701, Vec::new()).unwrap(),
					parameters: PoolParameters {
						min_epoch_time: 12,
						max_nav_age: 1,
					},
					metadata: Default::default(),
					status: PoolStatus::Open,
					epoch: EpochState {
						current: 3,
						last_closed: 42,
						last_executed: 2,
					},
					reserve: ReserveDetails {
						max: 47,
						total: 39,
						available: 58,
					},
				},
			);
			let state = v1::Migration::<Runtime, MetaSize>::pre_upgrade().unwrap();
			v1::Migration::<Runtime, MetaSize>::on_runtime_upgrade();
			v1::Migration::<Runtime, MetaSize>::post_upgrade(state).unwrap();
		})
	}
}
