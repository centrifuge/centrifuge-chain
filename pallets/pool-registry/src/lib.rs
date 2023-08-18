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
#![cfg_attr(not(feature = "std"), no_std)]

use cfg_primitives::Moment;
use cfg_traits::{
	investments::TrancheCurrency, Permissions, PoolMutate, PoolWriteOffPolicyMutate, UpdateState,
};
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role},
	pools::{PoolMetadata, PoolRegistrationStatus},
	tokens::CustomMetadata,
};
use codec::{HasCompact, MaxEncodedLen};
use frame_support::{pallet_prelude::*, scale_info::TypeInfo, transactional, BoundedVec};
use frame_system::pallet_prelude::*;
use orml_traits::asset_registry::{Inspect, Mutate};
pub use pallet::*;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, BadOrigin},
	FixedPointNumber, FixedPointOperand,
};
use sp_std::vec::Vec;
pub use weights::WeightInfo;
use xcm::VersionedMultiLocation;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

type PoolMetadataOf<T> = PoolMetadata<<T as Config>::MaxSizeMetadata>;

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

#[frame_support::pallet]
pub mod pallet {

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ From<u128>
			+ TypeInfo
			+ TryInto<u64>;

		type PoolId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ core::fmt::Debug;

		type Rate: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ TypeInfo
			+ MaxEncodedLen;

		/// A fixed-point number which represents an
		/// interest rate.
		type InterestRate: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>;

		type ModifyPool: PoolMutate<
			Self::AccountId,
			Self::PoolId,
			CurrencyId = Self::CurrencyId,
			Balance = Self::Balance,
		>;

		type ModifyWriteOffPolicy: PoolWriteOffPolicyMutate<Self::PoolId>;

		/// The currency type of investments.
		type TrancheCurrency: TrancheCurrency<Self::PoolId, Self::TrancheId>
			+ Into<Self::CurrencyId>;

		type CurrencyId: Parameter + Copy;

		type TrancheId: Member
			+ Parameter
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ TypeInfo
			+ From<[u8; 16]>;

		/// The origin permitted to create pools
		type PoolCreateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Max length for a tranche token name
		#[pallet::constant]
		type MaxTokenNameLength: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max length for a tranche token symbol
		#[pallet::constant]
		type MaxTokenSymbolLength: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max number of Tranches
		#[pallet::constant]
		type MaxTranches: Get<u32> + Member + scale_info::TypeInfo;

		/// Max size of Metadata
		#[pallet::constant]
		type MaxSizeMetadata: Get<u32> + Copy + Member + scale_info::TypeInfo;

		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId, Moment>,
			Error = DispatchError,
		>;

		/// The registry type used for retrieving and updating tranche metadata
		/// as part of the `PoolMetadata` trait implementation
		type AssetRegistry: Mutate<
				AssetId = Self::CurrencyId,
				Balance = Self::Balance,
				CustomMetadata = CustomMetadata,
			> + Inspect<
				AssetId = Self::CurrencyId,
				Balance = Self::Balance,
				CustomMetadata = CustomMetadata,
			>;

		/// Weight Information
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn get_pool_metadata)]
	pub(crate) type PoolMetadata<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, PoolMetadataOf<T>>;

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
		/// A pool update was executed.
		UpdateExecuted { pool_id: T::PoolId },
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
		/// Invalid metadata passed
		BadMetadata,
		/// A Pool with the given ID was already registered in the past
		PoolAlreadyRegistered,
		/// Pre-requirements for a TrancheUpdate are not met
		/// for example: Tranche changed but not its metadata or vice versa
		InvalidTrancheUpdate,
		/// No metadata for the given currency found
		MetadataForCurrencyNotFound,
		/// No Metadata found for the given PoolId
		NoSuchPoolMetadata,
		/// The given tranche token name exceeds the length limit
		TrancheTokenNameTooLong,
		/// The given tranche symbol name exceeds the length limit
		TrancheSymbolNameTooLong,
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
		#[pallet::weight(T::WeightInfo::register(tranche_inputs.len().try_into().unwrap_or(u32::MAX)))]
		#[transactional]
		#[pallet::call_index(0)]
		pub fn register(
			origin: OriginFor<T>,
			admin: T::AccountId,
			pool_id: T::PoolId,
			tranche_inputs: Vec<TrancheInputOf<T>>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
			metadata: Option<Vec<u8>>,
			write_off_policy: PolicyOf<T>,
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

			if let Some(m) = metadata.clone() {
				let checked_metadata: BoundedVec<u8, T::MaxSizeMetadata> =
					m.try_into().map_err(|_| Error::<T>::BadMetadata)?;

				PoolMetadata::<T>::insert(
					pool_id,
					PoolMetadataOf::<T> {
						metadata: checked_metadata,
					},
				);
			}

			T::ModifyPool::create(
				admin,
				depositor,
				pool_id,
				tranche_inputs,
				currency,
				max_reserve,
			)
			.map(|_| Self::deposit_event(Event::Registered { pool_id }))?;

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
		#[pallet::weight(T::WeightInfo::update_no_execution(T::MaxTranches::get()).max(T::WeightInfo::update_and_execute(T::MaxTranches::get())))]
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
				UpdateState::NoExecution => T::WeightInfo::update_no_execution(0),
				UpdateState::Executed(num_tranches) => {
					Self::deposit_event(Event::UpdateExecuted { pool_id });
					T::WeightInfo::update_and_execute(num_tranches)
				}
				UpdateState::Stored(num_tranches) => {
					Self::deposit_event(Event::UpdateStored { pool_id });
					T::WeightInfo::update_no_execution(num_tranches)
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
		#[pallet::weight(T::WeightInfo::execute_update(T::MaxTranches::get()))]
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
		#[pallet::weight(T::WeightInfo::set_metadata(metadata.len().try_into().unwrap_or(u32::MAX)))]
		#[pallet::call_index(3)]
		pub fn set_metadata(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			metadata: Vec<u8>,
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

			let checked_metadata = Self::do_set_metadata(pool_id, metadata)?;

			Self::deposit_event(Event::MetadataSet {
				pool_id,
				metadata: checked_metadata,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn do_set_metadata(
			pool_id: T::PoolId,
			metadata: Vec<u8>,
		) -> Result<BoundedVec<u8, T::MaxSizeMetadata>, DispatchError> {
			let checked_metadata: BoundedVec<u8, T::MaxSizeMetadata> =
				metadata.try_into().map_err(|_| Error::<T>::BadMetadata)?;

			PoolMetadata::<T>::insert(
				pool_id,
				PoolMetadataOf::<T> {
					metadata: checked_metadata.clone(),
				},
			);

			Ok(checked_metadata)
		}
	}

	impl<T: Config> cfg_traits::PoolMetadata<T::Balance, VersionedMultiLocation> for Pallet<T> {
		type AssetMetadata = orml_asset_registry::AssetMetadata<T::Balance, Self::CustomMetadata>;
		type CustomMetadata = CustomMetadata;
		type PoolId = T::PoolId;
		type PoolMetadata = PoolMetadataOf<T>;
		type TrancheId = T::TrancheId;

		fn get_pool_metadata(pool_id: Self::PoolId) -> Result<Self::PoolMetadata, DispatchError> {
			PoolMetadata::<T>::get(pool_id).ok_or(Error::<T>::NoSuchPoolMetadata.into())
		}

		fn set_pool_metadata(pool_id: Self::PoolId, metadata: Vec<u8>) -> DispatchResult {
			Self::do_set_metadata(pool_id, metadata).map(|_| ())
		}

		fn get_tranche_token_metadata(
			pool_id: Self::PoolId,
			tranche_id: Self::TrancheId,
		) -> Result<Self::AssetMetadata, DispatchError> {
			let currency_id = T::TrancheCurrency::generate(pool_id, tranche_id).into();
			T::AssetRegistry::metadata(&currency_id)
				.ok_or(Error::<T>::MetadataForCurrencyNotFound.into())
		}

		fn create_tranche_token_metadata(
			pool_id: Self::PoolId,
			tranche: Self::TrancheId,
			metadata: Self::AssetMetadata,
		) -> DispatchResult {
			let currency_id = T::TrancheCurrency::generate(pool_id, tranche).into();
			T::AssetRegistry::register_asset(Some(currency_id), metadata)
		}

		fn update_tranche_token_metadata(
			pool_id: Self::PoolId,
			tranche: Self::TrancheId,
			decimals: Option<u32>,
			name: Option<Vec<u8>>,
			symbol: Option<Vec<u8>>,
			existential_deposit: Option<T::Balance>,
			location: Option<Option<VersionedMultiLocation>>,
			additional: Option<Self::CustomMetadata>,
		) -> DispatchResult {
			let currency_id = T::TrancheCurrency::generate(pool_id, tranche).into();
			T::AssetRegistry::update_asset(
				currency_id,
				decimals,
				name,
				symbol,
				existential_deposit,
				location,
				additional,
			)
		}
	}
}
