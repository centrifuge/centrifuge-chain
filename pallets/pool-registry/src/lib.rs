// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
//! # Pool Registry Pallet
//!
//! The Pool Registry pallet is used for creating, updating, and setting the
//! metadata of pools.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]

use cfg_traits::{
	fee::PoolFeeBucket, Permissions, PoolMutate, PoolWriteOffPolicyMutate, UpdateState,
};
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role},
	pools::PoolFeeInfo,
};
use frame_support::{pallet_prelude::*, transactional, BoundedVec};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use parity_scale_codec::MaxEncodedLen;
use scale_info::TypeInfo;
use sp_runtime::traits::BadOrigin;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

type PoolChangesOf<T> = <<T as Config>::ModifyPool as cfg_traits::PoolMutate<
	<T as frame_system::Config>::AccountId,
	<T as Config>::PoolId,
>>::PoolChanges;

type TrancheInputOf<T> = <<T as Config>::ModifyPool as cfg_traits::PoolMutate<
	<T as frame_system::Config>::AccountId,
	<T as Config>::PoolId,
>>::TrancheInput;

type PolicyOf<T> = <<T as Config>::ModifyWriteOffPolicy as cfg_traits::PoolWriteOffPolicyMutate<
	<T as Config>::PoolId,
>>::Policy;

type PoolFeeInput<T> = (
	PoolFeeBucket,
	PoolFeeInfo<
		<T as frame_system::Config>::AccountId,
		<T as Config>::Balance,
		<T as Config>::InterestRate,
	>,
);

type MaxTranches<T> = <<T as Config>::ModifyPool as PoolMutate<
	<T as frame_system::Config>::AccountId,
	<T as Config>::PoolId,
>>::MaxTranches;

type MaxFeesPerPool<T> = <<T as Config>::ModifyPool as PoolMutate<
	<T as frame_system::Config>::AccountId,
	<T as Config>::PoolId,
>>::MaxFeesPerPool;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum PoolRegistrationStatus {
	Registered,
	Unregistered,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Balance: Parameter + MaxEncodedLen;

		type PoolId: Parameter + Copy + MaxEncodedLen;

		/// A fixed-point number which represents an interest rate.
		type InterestRate: Parameter + MaxEncodedLen;

		type ModifyPool: PoolMutate<
			Self::AccountId,
			Self::PoolId,
			CurrencyId = Self::CurrencyId,
			Balance = Self::Balance,
			PoolFeeInput = PoolFeeInput<Self>,
		>;

		type ModifyWriteOffPolicy: PoolWriteOffPolicyMutate<Self::PoolId>;

		type CurrencyId: Parameter;

		type TrancheId;

		/// The origin permitted to create pools
		type PoolCreateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Max size of Metadata
		#[pallet::constant]
		type MaxSizeMetadata: Get<u32>;

		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId>,
			Error = DispatchError,
		>;

		/// Weight Information
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn get_pool_metadata)]
	pub(crate) type PoolMetadata<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, BoundedVec<u8, T::MaxSizeMetadata>>;

	#[pallet::storage]
	#[pallet::getter(fn get_pools)]
	pub(crate) type Pools<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, PoolRegistrationStatus>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A pool was registered.
		Registered { pool_id: T::PoolId },
		/// A pool update was registered.
		UpdateRegistered { pool_id: T::PoolId },
		/// A pool update was stored for later execution.
		UpdateStored { pool_id: T::PoolId },
		/// Pool metadata was set.
		MetadataSet {
			pool_id: T::PoolId,
			metadata: BoundedVec<u8, T::MaxSizeMetadata>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// A Pool with the given ID was already registered in the past
		PoolAlreadyRegistered,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new pool
		///
		/// Initialise a new pool with the given ID and tranche
		/// configuration. Tranche 0 is the equity tranche, and must
		/// have zero interest and a zero risk buffer.
		///
		/// The minimum epoch length, and maximum NAV age will be
		/// set to chain-wide defaults. They can be updated
		/// with a call to `update`.
		///
		/// The caller will be given the `PoolAdmin` role for
		/// the created pool. Additional administrators can be
		/// added with the Permissions pallet.
		///
		/// Returns an error if the requested pool ID is already in
		/// use, or if the tranche configuration cannot be used.
		#[allow(clippy::too_many_arguments)]
		#[pallet::weight(T::WeightInfo::register(
			tranche_inputs.len().try_into().unwrap_or(u32::MAX),
			pool_fees.len().try_into().unwrap_or(u32::MAX))
		)]
		#[transactional]
		#[pallet::call_index(0)]
		pub fn register(
			origin: OriginFor<T>,
			admin: T::AccountId,
			pool_id: T::PoolId,
			tranche_inputs: BoundedVec<TrancheInputOf<T>, MaxTranches<T>>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
			metadata: Option<BoundedVec<u8, T::MaxSizeMetadata>>,
			write_off_policy: PolicyOf<T>,
			pool_fees: BoundedVec<PoolFeeInput<T>, MaxFeesPerPool<T>>,
		) -> DispatchResult {
			T::PoolCreateOrigin::ensure_origin(origin.clone())?;

			// First we take a deposit.
			// If we are coming from a signed origin, we take
			// the deposit from them
			// If we are coming from some internal origin
			// (Democracy, Council, etc.) we assume that the
			// parameters are vetted somehow and rely on the
			// admin as our depositor.
			let depositor = ensure_signed(origin).unwrap_or_else(|_| admin.clone());

			if Pools::<T>::contains_key(pool_id) {
				return Err(Error::<T>::PoolAlreadyRegistered.into());
			} else {
				Pools::<T>::insert(pool_id, PoolRegistrationStatus::Registered);
			}

			// For SubQuery, pool registration event should be dispatched before MetadataSet
			// one
			T::ModifyPool::create(
				admin,
				depositor,
				pool_id,
				tranche_inputs,
				currency,
				max_reserve,
				pool_fees,
			)
			.map(|_| Self::deposit_event(Event::Registered { pool_id }))?;

			Self::do_set_metadata(pool_id, metadata.unwrap_or_default())?;

			T::ModifyWriteOffPolicy::update(pool_id, write_off_policy)
		}

		/// Update per-pool configuration settings.
		///
		/// This updates the tranches of the pool,
		/// sets the minimum epoch length, and maximum NAV age.
		///
		/// If no delay is required for updates and redemptions
		/// don't have to be fulfilled, then this is executed
		/// immediately. Otherwise, the update is scheduled
		/// to be executed later.
		///
		/// The caller must have the `PoolAdmin` role in order to
		/// invoke this extrinsic.
		#[pallet::weight(T::WeightInfo::update(MaxTranches::<T>::get()))]
		#[pallet::call_index(1)]
		pub fn update(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			changes: PoolChangesOf<T>,
		) -> DispatchResultWithPostInfo {
			// Make sure the following are true for a valid Pool Update
			// 1. Make sure Origin is signed
			// 2. Ensure the signed origin is PoolAdmin
			// 3. Either no changes in tranches/metadata or changes in both
			// 4. Ensure not in Submission Epoch Time
			// 5. If no changes, do no_op
			// 6. Minimum epoch time has to be between min and max epoch time
			// 7. MaxNavAge has to be under the upper bound NavAge
			// 8. IsValidTrancheChange?

			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who,
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				BadOrigin
			);

			let state = T::ModifyPool::update(pool_id, changes)?;
			Self::deposit_event(Event::UpdateRegistered { pool_id });

			let weight = match state {
				UpdateState::NoExecution => T::WeightInfo::update(0),
				UpdateState::Stored(num_tranches) => {
					Self::deposit_event(Event::UpdateStored { pool_id });
					T::WeightInfo::update(num_tranches)
				}
			};

			Ok(Some(weight).into())
		}

		/// Executed a scheduled update to the pool.
		///
		/// This checks if the scheduled time is in the past
		/// and, if required, if there are no outstanding
		/// redeem orders. If both apply, then the scheduled
		/// changes are applied.
		#[pallet::weight(T::WeightInfo::execute_update(MaxTranches::<T>::get()))]
		#[pallet::call_index(2)]
		pub fn execute_update(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			let num_tranches = T::ModifyPool::execute_update(pool_id)?;

			Ok(Some(T::WeightInfo::execute_update(num_tranches)).into())
		}

		/// Sets the IPFS hash for the pool metadata information.
		///
		/// The caller must have the `PoolAdmin` role in order to
		/// invoke this extrinsic.
		#[pallet::weight(T::WeightInfo::set_metadata())]
		#[pallet::call_index(3)]
		pub fn set_metadata(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			metadata: BoundedVec<u8, T::MaxSizeMetadata>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who,
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				BadOrigin,
			);

			Self::do_set_metadata(pool_id, metadata)?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn do_set_metadata(
			pool_id: T::PoolId,
			metadata: BoundedVec<u8, T::MaxSizeMetadata>,
		) -> DispatchResult {
			PoolMetadata::<T>::insert(pool_id, metadata.clone());

			Self::deposit_event(Event::MetadataSet { pool_id, metadata });

			Ok(())
		}
	}
}
