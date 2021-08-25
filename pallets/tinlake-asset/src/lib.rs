// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// SPDX-License-Identifier: Apache-2.0
//
// This file is part of the Centrifuge chain project.
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::{DispatchError, DispatchResult};
pub use pallet::*;
use tinlake::traits::{Asset as AssetTrait, Collaterale, Lockable, Owner, StaticPool};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;
	use frame_support::{
		dispatch::{DispatchError, DispatchResult},
		pallet_prelude::*,
	};
	use frame_system::pallet_prelude::*;

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching pool id
		/// TODO: We could move this one here to some overarching tinlake_system::Config pallet
		///       that also takes care of incrementing ids. Otherwise, every pallet will need this type
		type PoolId: Member + Default + AtLeast32BitUnsigned;

		/// The Ids with which assets are identified here
		type AssetId: Member + Default + AtLeast32BitUnsigned;

		/// The balance type of this pallet
		type Balance: Member + Default + AtLeast32BitUnsigned;

		/// The pool we are having here
		type pool: StaticPool<Self::PoolId, AssetId = Self::AssetId> + Owner<Self::PoolId>;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// https://substrate.dev/docs/en/knowledgebase/runtime/storage
	#[pallet::storage]
	#[pallet::getter(fn something)]
	// Learn more about declaring storage items:
	// https://substrate.dev/docs/en/knowledgebase/runtime/storage#declaring-storage-items
	pub type Something<T> = StorageValue<_, u32>;

	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn mint(origin: OriginFor<T>) -> DispatchResult {
			todo!();

			// Create actual asset type from AssetSpec
			// We do not want an ID here, as we are generating them locally
		}

		#[pallet::weight(10_000)]
		pub fn burn(origin: OriginFor<T>, id: T::NftId) -> DispatchResult {
			todo!();

			// Check if it is owner
			// Check if there are no locks
			// Burn
		}
	}
}

// TODO: define actual spec
pub struct AssetSpec {
	link: Option<Vec<u8>>,
	name: Option<Vec<u8>>,
}

pub struct Asset<AccountId> {
	owner: AccountId,
	created: bool, // TODO: maybe timestamp here?
	spec: AssetSpec,
}

impl<T: Config> Owner<T::AccountId> for Pallet<T> {
	type Of = T::AssetId;

	fn ownership(of: T::AssetId, who: T::AccountId) -> bool {
		todo!();
	}
}

impl<T: Config> Lockable<T::AssetId> for Pallet<T> {
	type Reason = LockReason;

	fn lock(id: T::AssetId, reason: LockReason) -> DispatchResult {
		todo!();
	}

	fn unlock(id: T::assetId, reason: LockReason) -> DispatchResult {
		todo!();
	}

	fn locks(id: T::AssetId) -> Result<Option<Vec<LockReasons>>, DispatchError> {
		todo!();
	}
}

impl<T: Config> Collaterale<T::AssetId, T::AccountId> for Pallet<T> {
	fn seize(what: T::AssetId, custodian: T::AccountId) {
		todo!()
	}

	fn seized(what: T::AssetId) -> bool {
		todo!()
	}
}

impl<T: Config> AssetTrait<T::AssetId> for Pallet<T> {
	type Balance = T::Balance;
	type Info = T::Asset<T::AccountId>;

	fn value(asset: T::AssetId) -> Self::Balance {
		todo!();
	}

	fn info(aasset: T::AssetId) -> Self::Info {
		todo!();
	}
}
