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

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_primitives::Moment;
	use cfg_traits::PoolNAV;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use sp_runtime::traits::{AtLeast32BitUnsigned, Zero};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type ClassId: Parameter + Member + MaybeSerializeDeserialize + Copy + Default + TypeInfo;

		type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;

		type Balance: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ AtLeast32BitUnsigned;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type Nav<T: Config> = StorageMap<_, Blake2_128Concat, T::PoolId, (T::Balance, Moment)>;

	impl<T: Config> Pallet<T> {
		pub fn value(pool_id: T::PoolId) -> T::Balance {
			Nav::<T>::get(pool_id)
				.map(|(nav, _)| nav)
				.unwrap_or_else(T::Balance::zero)
		}

		pub fn update(pool_id: T::PoolId, balance: T::Balance, now: Moment) {
			Nav::<T>::insert(pool_id, (balance, now));
		}

		pub fn latest(pool_id: T::PoolId) -> (T::Balance, Moment) {
			Nav::<T>::get(pool_id).unwrap_or((T::Balance::zero(), 0))
		}
	}

	impl<T: Config> PoolNAV<T::PoolId, T::Balance> for Pallet<T> {
		type ClassId = T::ClassId;
		type RuntimeOrigin = T::RuntimeOrigin;

		fn nav(pool_id: T::PoolId) -> Option<(T::Balance, Moment)> {
			Some(Self::latest(pool_id))
		}

		fn update_nav(pool_id: T::PoolId) -> Result<T::Balance, DispatchError> {
			Ok(Self::value(pool_id))
		}

		fn initialise(_: Self::RuntimeOrigin, _: T::PoolId, _: Self::ClassId) -> DispatchResult {
			Ok(())
		}
	}
}
