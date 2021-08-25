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

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
	use frame_system::pallet_prelude::*;
	use tinlake::traits::{Asset, Loan, Owner, StaticPool};

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching pool id
		/// TODO: We could move this one here to some overarching tinlake_system::Config pallet
		///       that also takes care of incrementing ids. Otherwise, every pallet will need this type
		type PoolId: Member + Default + AtLeast32BitUnsigned;

		/// The Ids with which assets are identified here
		type AssetId: Member + Default + AtLeast32BitUnsigned;

		/// The Id the loan is against
		type LoanId: Member + Default + AtLeast32BitUnsigned;

		/// NFT or Asset storage
		type Assets: Asset<AssetId, Balance = Self::Balance>
			+ Owner<T::AccountId, Of = Self::AssetId>;

		/// The balance type of this pallet
		type Balance: Member + Default + AtLeast32BitUnsigned;

		/// The pool we are having here
		type Pool: StaticPool<Self::PoolId, AssetId = Self::AssetId>
			+ Owner<T::AccountId, Of = Self::PoolId>;

		/// The strucutre that holds loans, and allows to repay and borrow from them
		type Lender: Loan<T::LoanId> + Owner<T::AccountId, Of = Self::LoanId>;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn something)]
	pub type Something<T> = StorageValue<_, u32>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn issue_asset(
			origin: OriginFor<T>,
			asset: T::AssetId,
			pool: T::PooldId,
		) -> DispatchResult {
			todo!();

			// Check for ownership of asset
			// check for ownership of pool
			// add_asset to pool
		}

		#[pallet::weight(10_000)]
		pub fn retract_asset(
			origin: OriginFor<T>,
			asset: T::AssetId,
			pool: T::PoolId,
		) -> DispatchResult {
			todo!();
			// Check for ownership of asset
			// check for ownership of pool
			// check for locks on asset
			// rmv_asset to pool
		}

		#[pallet::weight(10_000)]
		pub fn borrow(origin: OriginFor<T>, loan: T::LoanId, amount: T::Balance) -> DispatchResult {
			todo!();

			// check for ownership of loan
			// Call the trait funciton of loan
			// T::Loan::borrow(loan, amount)
		}

		#[pallet::weight(10_000)]
		pub fn repay(origin: OriginFor<T>, loan: T::LoanId, amount: T::Balance) -> DispatchResult {
			todo!();
			// Just call the trait function of loan
			// T::Loan::repay(loan, amount)
		}
	}
}
