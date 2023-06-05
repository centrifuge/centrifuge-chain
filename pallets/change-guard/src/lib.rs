// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Handle changes that require an special treatment to only release them once their requirements
//! are fulfilled.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::changes::ChangeGuard;
	use cfg_types::changes::CfgChange;
	use frame_support::pallet_prelude::*;
	use sp_runtime::DispatchError;

	trait StorableChange: sp_std::any::Any + Codec + TypeInfo + MaxEncodedLen {}
	impl<T: sp_std::any::Any + Codec + TypeInfo + MaxEncodedLen> StorableChange for T {}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// A pool identification
		type PoolId: Parameter + MaxEncodedLen + Ord;

		/// A change identification
		type ChangeId: Parameter + MaxEncodedLen + Ord + Default;
	}

	/// Storage that contains the registering information
	#[pallet::storage]
	pub(crate) type Changes<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::PoolId, Blake2_128Concat, T::ChangeId, CfgChange>;

	#[pallet::storage]
	pub(crate) type Aaa<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, Box<dyn StorableChange>>;

	/// Storage that contains the data values of a collection.
	#[pallet::storage]
	pub(crate) type LastChangeId<T: Config> = StorageValue<_, T::ChangeId, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {}

	impl<T: Config> ChangeGuard for Pallet<T> {
		type Change = CfgChange;
		type ChangeId = T::ChangeId;
		type PoolId = T::PoolId;

		fn note(
			pool_id: Self::PoolId,
			change: Self::Change,
		) -> Result<Self::ChangeId, DispatchError> {
			todo!()
		}

		fn released(pool_id: Self::PoolId, change_id: Self::ChangeId) -> DispatchResult {
			todo!()
		}

		fn cancel(pool_id: Self::PoolId, change_id: Self::ChangeId) -> DispatchResult {
			todo!()
		}
	}
}
