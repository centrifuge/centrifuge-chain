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
use tinlake::traits::{InvestmentPool, Owner, Reserve, RevolvingPool, StaticPool};

use frame_support::dispatch::DispatchResult;
use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;
use frame_support::sp_std::clone::Clone;
use frame_support::sp_std::cmp::{Eq, PartialEq};
use frame_support::sp_std::fmt::Debug;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32BitUnsigned, Bounded, CheckedAdd, CheckedSub, One,
		Saturating, StaticLookup, StoredMapError, Zero,
	},
	FixedPointNumber, Perquintill, TypeId,
};
use sp_std::vec::Vec;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned, One};
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
	use frame_system::pallet_prelude::*;

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching pool id
		/// TODO: We could move this one here to some overarching tinlake_system::Config pallet
		///       that also takes care of incrementing ids. Otherwise, every pallet will need this type
		type PoolId: Member
			+ Default
			+ Copy
			+ AtLeast32BitUnsigned
			+ AccountIdConversion<T::AccountId>
			+ One;

		/// The Ids with which assets are identified here
		type AssetId: Member + Default + AtLeast32BitUnsigned;

		/// The currency Id type for multicurrency
		type CurrencyId;

		/// The balance type of this pallet
		type Balance: Member + Default + AtLeast32BitUnsigned;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::PoolId,
		PoolDetails<T::AccountId, T::CurrencyId, T::EpochId, T::Balance, T::Moment>,
	>;

	#[pallet::type_value]
	pub fn OnPoolIdEmpty() -> T::OrderId {
		One::one()
	}

	#[pallet::storage]
	#[pallet::getter(fn order_id)]
	pub type PoolId<T> = StorageValue<_, T::PoolId, ValueQuery, OnPoolIdEmpty>;

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::metadata(T::PoolId = "PoolId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Pool Created. [id, who]
		PoolCreated(T::PoolId, T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// A pool with this ID is already in use
		InUse,
		/// A parameter is invalid
		Invalid,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create_pool(
			origin: OriginFor<T>,
			id: T::PoolId,
			tranches: Vec<Tranch>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;

			// TODO: Ensure owner is authorized to create a pool

			// A single pool ID can only be used by one owner.
			ensure!(!Pool::<T>::contains_key(id), Error::<T>::InUse);

			// At least one tranch must exist, and the last
			// tranche must have an interest rate of 0,
			// indicating that it recieves all remaining
			// equity
			ensure!(tranches.last() == Some(&(0, 0)), Error::<T>::Invalid);

			let tranches = tranches
				.into_iter()
				.map(|(interest, sub_percent)| {
					const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
					let interest_per_sec =
						Perquintill::from_percent(interest.into()) / SECS_PER_YEAR;
					Tranche {
						interest_per_sec,
						min_subordination_ratio: Perquintill::from_percent(sub_percent.into()),
						epoch_supply: Zero::zero(),
						epoch_redeem: Zero::zero(),
						is_closing: false,
					}
				})
				.collect();
			Pool::<T>::insert(
				id,
				PoolDetails {
					owner: owner.clone(),
					currency,
					tranches,
					current_epoch: One::one(),
					last_epoch_closed: Default::default(),
					last_epoch_executed: Zero::zero(),
					closing_epoch: None,
					max_reserve,
					available_reserve: Zero::zero(),
				},
			);
			Self::deposit_event(Event::PoolCreated(id, owner));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		fn adjust_max_reserve(id: T::PoolId, new_reserve: T::Balance) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		fn dissolve_pool(id: T::PooldId) -> DispatchResultWithPostInfo {
			todo!()
		}

		/// Do not allow anymore investements or redemptions
		#[pallet::weight(10_000)]
		fn freeze(id: T::PooldId) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		fn unfreeze(id: PooldId) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		fn force_dissolve(id: PooldId) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		fn force_change_owner(id: PooldId) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		fn force_freeze(id: PooldId) -> DispatchResultWithPostInfo {
			todo!()
		}

		#[pallet::weight(10_000)]
		fn force_unfreeze(id: PooldId) -> DispatchResultWithPostInfo {
			todo!()
		}
	}
}

// Todo: Maybe contain its own id?
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct PoolDetails<AccountId, CurrencyId, EpochId, Balance, Timestamp> {
	pub owner: AccountId,
	pub currency: CurrencyId,
	pub tranches: Vec<Tranche<Balance>>,
	pub current_epoch: EpochId,
	pub last_epoch_closed: Timestamp,
	pub last_epoch_executed: EpochId,
	pub closing_epoch: Option<EpochId>,
	pub max_reserve: Balance,
	pub available_reserve: Balance,
}

// TODO: This should maybe go to tinlake..
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct Tranche<Balance> {
	pub interest_per_sec: Perquintill,
	pub min_subordination_ratio: Perquintill,
	pub epoch_supply: Balance,
	pub epoch_redeem: Balance,
	pub is_closing: bool,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default)]
pub struct EpochDetails<BalanceRatio> {
	pub supply_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
	pub token_price: BalanceRatio,
}

impl<T: Config> StaticPool<T::PoolId> for Pallet<T> {
	type AssetId = T::AssetId;
	type TrancheId = T::TrancheId;
	type Tranche = Tranche<T::Balance>; // TODO: The tranche type -> need to go into tinlake libs
	type Investor = (); // TODO: The investor type -> need to go into tinlake libs

	fn assets(pool: T::PoolId) -> Vec<Self::AssetId> {
		todo!()
	}

	fn add_asset(pool: T::PoolId, asset: Self::AssetId) -> DispatchResult {
		todo!()
	}

	fn remove_asset(pool: T::PoolId, asset: Self::AssetId) -> DispatchResult {
		todo!()
	}

	fn tranches(pool: T::PoolId) -> Vec<Self::Tranche> {
		todo!()
	}

	fn investors(pool: _, _: Option<Self::TrancheId>) -> Vec<Self::Investor> {
		todo!()
	}
}

impl<T: Config> RevolvingPool<T::PoolId, T::BlockNumber> for Pallet<T> {
	fn last_epoch(pool: T::PoolId) -> T::BlockNumber {
		todo!()
	}

	fn min_epoch(pool: T::PoolId) -> T::BlockNumber {
		todo!()
	}

	fn closeable(pool: T::PoolId) -> bool {
		todo!()
	}

	fn close_epoch(pool: T::PoolId) -> DispatchResult {
		todo!()
	}
}

impl<T: Config> InvestmentPool<T::PoolId> for Pallet<T> {
	type Order = tinlake::Order; // TODO: This should be in tinlake in order to not create circualr deps

	fn order(pool: T::PoolId, orders: Vec<Self::Order>) -> DispatchResult {
		todo!()
		// Those can only be stored once close_epoch has been called.
	}
}

impl<T: Config> Reserve<T::AccountId> for Pallet<T> {
	type Balance = T::Balance;

	fn deposit(from: T::AccountId, to: T::AccountId, amount: Self::Balance) -> DispatchResult {
		todo!()
	}

	fn payout(from: T::AccountId, to: T::AccountId, amount: Self::Balance) -> DispatchResult {
		todo!()
	}

	fn max_reserve(account: T::AccountId) -> Self::Balance {
		todo!()
	}

	fn avail_reserve(account: T::AccountId) -> Self::Balance {
		todo!()
	}
}

impl<T: Config> Owner<T::PoolId> for Pallet<T> {
	type Of = T::PoolId;

	fn ownership(of: Self::Of, who: T::AccountId) -> bool {
		Pool::<T>::contains_key(of.clone()) && Pool::<T>::get(of).owner == who
	}
}
