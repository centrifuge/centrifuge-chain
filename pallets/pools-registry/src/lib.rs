// Copyright 2022 Centrifuge Foundation (centrifuge.io).
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
use cfg_traits::Permissions;
use cfg_types::{PermissionScope, PoolRole, Role};
use codec::HasCompact;
use frame_support::{pallet_prelude::*, scale_info::TypeInfo, BoundedVec};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, BadOrigin},
	FixedPointOperand,
};
use sp_std::vec::Vec;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo)]
pub struct TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	pub token_name: BoundedVec<u8, MaxTokenNameLength>,
	pub token_symbol: BoundedVec<u8, MaxTokenSymbolLength>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolMetadata<MetaSize>
where
	MetaSize: Get<u32>,
{
	metadata: BoundedVec<u8, MetaSize>,
}

type PoolMetadataOf<T> = PoolMetadata<<T as Config>::MaxSizeMetadata>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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

		type CurrencyId: Parameter + Copy;

		type Metadata: Eq
			+ PartialEq
			+ Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ core::fmt::Debug;

		type TrancheId: Member
			+ Parameter
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ TypeInfo
			+ From<[u8; 16]>;

		/// Max size of Metadata
		#[pallet::constant]
		type MaxSizeMetadata: Get<u32> + Copy + Member + scale_info::TypeInfo;

		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId, Moment>,
			Error = DispatchError,
		>;

		/// Weight Information
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn get_pool_metadata)]
	pub(super) type PoolMetadata<T: Config> =
		StorageMap<_, Blake2_256, T::PoolId, PoolMetadataOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
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
		/// Pre-requirements for a TrancheUpdate are not met
		/// for example: Tranche changed but not its metadata or vice versa
		InvalidTrancheUpdate,
		/// No metada for the given currency found
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
		#[pallet::weight(T::WeightInfo::create(tranche_inputs.len().try_into().unwrap_or(u32::MAX)))]
		#[transactional]
		pub fn create(
			origin: OriginFor<T>,
			admin: T::AccountId,
			pool_id: T::PoolId,
			tranche_inputs: Vec<
				TrancheInput<T::InterestRate, T::MaxTokenNameLength, T::MaxTokenSymbolLength>,
			>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
			metadata: Option<Vec<u8>>,
		) -> DispatchResult {
			T::PoolCreateOrigin::ensure_origin(origin.clone())?;

			// First we take a deposit.
			// If we are coming from a signed origin, we take
			// the deposit from them
			// If we are coming from some internal origin
			// (Democracy, Council, etc.) we assume that the
			// parameters are vetted somehow and rely on the
			// admin as our depositor.
			let depositor = ensure_signed(origin).unwrap_or(admin.clone());
			Self::take_deposit(depositor, pool_id)?;

			// A single pool ID can only be used by one owner.
			ensure!(!Pool::<T>::contains_key(pool_id), Error::<T>::PoolInUse);

			ensure!(
				T::PoolCurrency::contains(&currency),
				Error::<T>::InvalidCurrency
			);

			Self::is_valid_tranche_change(
				None,
				&tranche_inputs
					.iter()
					.map(|t| TrancheUpdate {
						tranche_type: t.tranche_type,
						seniority: t.seniority,
					})
					.collect(),
			)?;

			let now = Self::now();

			let tranches = Tranches::from_input::<
				T::TrancheToken,
				T::MaxTokenNameLength,
				T::MaxTokenSymbolLength,
			>(pool_id, tranche_inputs.clone(), now)?;

			let checked_metadata: Option<BoundedVec<u8, T::MaxSizeMetadata>> = match metadata {
				Some(metadata_value) => {
					let checked: BoundedVec<u8, T::MaxSizeMetadata> = metadata_value
						.try_into()
						.map_err(|_| Error::<T>::BadMetadata)?;

					Some(checked)
				}
				None => None,
			};

			for (tranche, tranche_input) in tranches.tranches.iter().zip(&tranche_inputs) {
				let token_name: BoundedVec<u8, T::MaxTokenNameLength> =
					tranche_input.clone().metadata.token_name.clone();

				let token_symbol: BoundedVec<u8, T::MaxTokenSymbolLength> =
					tranche_input.metadata.token_symbol.clone();

				let decimals = match T::AssetRegistry::metadata(&currency) {
					Some(metadata) => metadata.decimals,
					None => return Err(Error::<T>::MetadataForCurrencyNotFound.into()),
				};

				let parachain_id = T::ParachainId::get();

				let metadata = tranche.create_asset_metadata(
					decimals,
					parachain_id,
					token_name.to_vec(),
					token_symbol.to_vec(),
				);

				T::AssetRegistry::register_asset(Some(tranche.currency), metadata)
					.map_err(|_| Error::<T>::FailedToRegisterTrancheMetadata)?;
			}

			Pool::<T>::insert(
				pool_id,
				PoolDetails {
					currency,
					tranches,
					status: PoolStatus::Open,
					epoch: EpochState {
						current: One::one(),
						last_closed: now,
						last_executed: Zero::zero(),
					},
					parameters: PoolParameters {
						min_epoch_time: sp_std::cmp::min(
							sp_std::cmp::max(
								T::DefaultMinEpochTime::get(),
								T::MinEpochTimeLowerBound::get(),
							),
							T::MinEpochTimeUpperBound::get(),
						),
						max_nav_age: sp_std::cmp::min(
							T::DefaultMaxNAVAge::get(),
							T::MaxNAVAgeUpperBound::get(),
						),
					},
					reserve: ReserveDetails {
						max: max_reserve,
						available: Zero::zero(),
						total: Zero::zero(),
					},
					metadata: checked_metadata,
				},
			);

			T::Permission::add(
				PermissionScope::Pool(pool_id),
				admin.clone(),
				Role::PoolRole(PoolRole::PoolAdmin),
			)?;

			Self::deposit_event(Event::Created { pool_id, admin });
			Ok(())
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
		#[pallet::weight(T::WeightInfo::update_no_execution(T::MaxTranches::get())
		.max(T::WeightInfo::update_and_execute(T::MaxTranches::get())))]
		pub fn update(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			changes: PoolChangesOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who,
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				BadOrigin
			);

			ensure!(
				EpochExecution::<T>::try_get(pool_id).is_err(),
				Error::<T>::InSubmissionPeriod
			);

			// Both changes.tranches and changes.tranche_metadata
			// have to be NoChange or Change, we don't allow to change either or
			// ^ = XOR, !^ = negated XOR
			ensure!(
				!((changes.tranches == Change::NoChange)
					^ (changes.tranche_metadata == Change::NoChange)),
				Error::<T>::InvalidTrancheUpdate
			);

			if changes.min_epoch_time == Change::NoChange
				&& changes.max_nav_age == Change::NoChange
				&& changes.tranches == Change::NoChange
			{
				// If there's an existing update, we remove it
				// If not, this transaction is a no-op
				if ScheduledUpdate::<T>::contains_key(pool_id) {
					ScheduledUpdate::<T>::remove(pool_id);
				}

				return Ok(Some(T::WeightInfo::update_no_execution(0)).into());
			}

			if let Change::NewValue(min_epoch_time) = changes.min_epoch_time {
				ensure!(
					min_epoch_time >= T::MinEpochTimeLowerBound::get()
						&& min_epoch_time <= T::MinEpochTimeUpperBound::get(),
					Error::<T>::PoolParameterBoundViolated
				);
			}

			if let Change::NewValue(max_nav_age) = changes.max_nav_age {
				ensure!(
					max_nav_age <= T::MaxNAVAgeUpperBound::get(),
					Error::<T>::PoolParameterBoundViolated
				);
			}

			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;

			if let Change::NewValue(tranches) = &changes.tranches {
				Self::is_valid_tranche_change(Some(&pool.tranches), tranches)?;
			}

			let now = Self::now();

			let update = ScheduledUpdateDetails {
				changes: changes.clone(),
				scheduled_time: now.saturating_add(T::MinUpdateDelay::get()),
			};

			let num_tranches = pool.tranches.num_tranches().try_into().unwrap();
			if T::MinUpdateDelay::get() == 0 && T::UpdateGuard::released(&pool, &update, now) {
				Self::do_update_pool(&pool_id, &changes)?;

				Ok(Some(T::WeightInfo::update_and_execute(num_tranches)).into())
			} else {
				// If an update was already stored, this will override it
				ScheduledUpdate::<T>::insert(pool_id, update);

				Ok(Some(T::WeightInfo::update_no_execution(num_tranches)).into())
			}
		}

		/// Sets the IPFS hash for the pool metadata information.
		///
		/// The caller must have the `PoolAdmin` role in order to
		/// invoke this extrinsic.
		#[pallet::weight(T::WeightInfo::set_metadata(metadata.len().try_into().unwrap_or(u32::MAX)))]
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

			let checked_metadata: BoundedVec<u8, T::MaxSizeMetadata> =
				metadata.try_into().map_err(|_| Error::<T>::BadMetadata)?;

			PoolMetadata::<T>::insert(
				pool_id,
				PoolMetadataOf::<T> {
					metadata: checked_metadata.clone(),
				},
			);

			Self::deposit_event(Event::MetadataSet {
				pool_id,
				metadata: checked_metadata,
			});

			Ok(())
		}
	}
}
