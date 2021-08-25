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

use tinlake::traits::{Accreditation, InvestmentPool, Owner, RevolvingPool};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, One};
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
	use frame_system::pallet_prelude::*;
	use tinlake::Order;

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching pool id
		/// TODO: We could move this one here to some overarching tinlake_system::Config pallet
		///       that also takes care of incrementing ids. Otherwise, every pallet will need this type
		type PoolId: Member + Default + AtLeast32BitUnsigned;

		/// The Ids with which assets are identified here
		type OrderId: Member + Default + AtLeast32BitUnsigned + One;

		/// The balance type of this pallet
		type Balance: Member + Default + AtLeast32BitUnsigned;

		/// The pool we are having here
		type pool: RevolvingPool<Self::PoolId, T::BlockNumber>
			+ InvestmentPool<Self::PoolId>
			+ Owner<Self::PoolId>;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn orders)]
	pub type Orders<T> =
		StorageDoubleMap<_, Blake2_128concat, T::AccountId, _, Twox64Concat, T::PoolId, Vec<Order>>;

	#[pallet::storage]
	#[pallet::getter(fn orders)]
	pub type FinalOrders<T> = StorageMap<_, Twox64Concat, T::PoolId, Vec<T::OrderId>>;

	#[pallet::type_value]
	pub fn OnOrderIdEmpty() -> T::OrderId {
		One::one()
	}

	#[pallet::storage]
	#[pallet::getter(fn order_id)]
	pub type OrderId<T> = StorageValue<_, T::OrderId, ValueQuery, OnOrderIdEmpty>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn add_investor(
			origin: OriginFor<T>,
			investor: T::AccountId,
			pool: T::PoolId,
			tranche: T::TrancheId,
		) -> DispatchResult {
			todo!();
			// Check if origin is owner of pool
			// Add investor to pool-tranche-entry
		}

		#[pallet::weight(10_000)]
		pub fn rmv_investor(
			origin: OriginFor<T>,
			investor: T::AccountId,
			pool: T::PoolId,
			tranche: T::TrancheId,
		) -> DispatchResult {
			todo!();
			// Check if origin is owner of pool
			// Remove investor from pool-tranche-entry
		}

		#[pallet::weight(10_000)]
		pub fn order_invest(
			origin: OriginFor<T>,
			pool: T::PoolId,
			tranche: T::TrancheId,
			amount: T::Balance,
		) -> DispatchResult {
			todo!();
			// Check if origin is investor for pool and tranche
			// Store the order for the given pool in orders
			// Orders is equal for redeem and invest.
		}

		#[pallet::weight(10_000)]
		pub fn order_redeem(
			origin: OriginFor<T>,
			pool: T::PoolId,
			tranche: T::TrancheId,
			amount: T::Balance,
		) -> DispatchResult {
			todo!();
			// Check if origin is investor for pool and tranche
			// Store the order for the given pool in orders
			// Orders is equal for redeem and invest.
		}

		#[pallet::weight(10_000)]
		pub fn cancel_order(
			origin: OriginFor<T>,
			pool: T::PoolId,
			order: T::OrderId,
		) -> DispatchResult {
			todo!();
			// Check if origin is creator of order
			// remove from orders
			// TODO: Maybe we will not allow this...
		}

		#[pallet::weight(10_000)]
		pub fn close_orders(origin: OriginFor<T>, pool: T::PoolId) -> DispatchResult {
			todo!();
			// Anybody can close orders for a pool
			// Orders are just cloasbeale if the epoch of a pool is able to be closed
			// Otherwise we fail. We could also allow for this to store something, so that the
			// orders will be transmitted to the pool upon `on_initialize` but this would make it
			// harder to still acceppting orders up to this point.
			//
			// Flow will be something like
			// T::Pool::close_epoch(pool_id)?;
			// T::Pool::order(pool_id, orders)?;
		}

		#[pallet::weight(10_000)]
		pub fn challenge_orders(
			origin: OriginFor<T>,
			pool: T::PoolId,
			orders: Vec<Order>,
		) -> DispatchResult {
			todo!();
			// Anybody can close orders for a pool
			// Orders are just cloasbeale if the epoch of a pool is able to be closed
			// Otherwise we fail. We could also allow for this to store something, so that the
			// orders will be transmitted to the pool upon `on_initialize` but this would make it
			// harder to still acceppting orders up to this point.
			//
			// Flow will be something like
			// T::Pool::close_epoch(pool_id)?;
			// T::Pool::order(pool_id, orders)?;
		}
	}
}

impl<T: Config> Accreditation<T::PoolId, T::TrancheId, T::AccountId> for Pallet<T> {
	fn accredited(pool: PoolId, tranche: TrancheId, who: AccountId) -> bool {
		todo!();
	}
}
