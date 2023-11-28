// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod types;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		changes::ChangeGuard, investments::TrancheCurrency, Permissions, PoolInspect,
	};
	use cfg_types::{
		permissions::{PermissionScope, PoolRole, Role},
		pools::FeeBucket,
	};
	use codec::HasCompact;
	use frame_support::{
		pallet_prelude::*,
		traits::fungibles::{Inspect, Mutate},
		weights::Weight,
	};
	use frame_system::pallet_prelude::*;
	use sp_arithmetic::traits::AtLeast32BitUnsigned;

	use super::*;
	use crate::types::{Change, PoolFee};

	pub type PoolFeeOf<T> =
		PoolFee<<T as frame_system::Config>::AccountId, <T as Config>::BalanceRatio>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The source of truth for the balance of accounts
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The currency type of transferrable tokens
		type CurrencyId: Parameter + Member + Copy + TypeInfo + MaxEncodedLen;

		/// The pool id type required for the investment identifier
		type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;

		/// The tranche id type required for the investment identifier
		type TrancheId: Member + Parameter + Default + Copy + MaxEncodedLen + TypeInfo;

		/// The investment identifying type required for the investment type
		type InvestmentId: TrancheCurrency<Self::PoolId, Self::TrancheId>
			+ Clone
			+ Member
			+ Parameter
			+ Copy
			+ MaxEncodedLen;

		/// Type for price ratio for cost of incoming currency relative to
		/// outgoing
		type BalanceRatio: Parameter
			+ Member
			+ sp_runtime::FixedPointNumber
			+ sp_runtime::traits::EnsureMul
			+ sp_runtime::traits::EnsureDiv
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// The type for handling transfers, burning and minting of
		/// multi-assets.
		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>;

		/// The source of truth for runtime changes.
		type RuntimeChange: From<Change<Self::AccountId, Self::BalanceRatio>>
			+ TryInto<Change<Self::AccountId, Self::BalanceRatio>>;

		/// Used to notify the runtime about changes that require special
		/// treatment.
		type ChangeGuard: ChangeGuard<
			PoolId = Self::PoolId,
			ChangeId = Self::Hash,
			Change = Self::RuntimeChange,
		>;

		/// The source of truth for pool inspection operations such as its
		/// existence, the corresponding tranche token or the investment
		/// currency.
		type PoolInspect: PoolInspect<
			Self::AccountId,
			Self::CurrencyId,
			PoolId = Self::PoolId,
			TrancheId = Self::TrancheId,
		>;

		/// The source of truth for investment permissions.
		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId>,
			Error = DispatchError,
		>;

		// TODO: Some Pool types such as PoolInspect

		// TODO: Type for fungibles::Hold

		//

		type MaxFeesPerPoolBucket: Get<u32>;

		// TODO: Enable after creating benchmarks
		// type WeightInfo: WeightInfo;
	}

	/// Maps a pool to their corresponding fees with [FeeBucket] granularity.
	///
	/// The lifetime of this storage is expected to be forever as it directly
	/// linked to a liquidity pool.
	///
	/// In general, epoch executions happen at different times for different
	/// pools. Thus, there should be no need to iterate over this storage at any
	/// time.
	#[pallet::storage]
	pub type FeeStructure<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		FeeBucket,
		BoundedVec<PoolFeeOf<T>, T::MaxFeesPerPoolBucket>,
		ValueQuery,
	>;

	/// Represents accrued and charged fees of a particular pool [FeeBucket].
	///
	/// The second key corresponds to the index of respective fee in the (pool
	/// id, bucket) list entry of [crate::pallet::FeeStructure].
	///
	/// This storage is updated whenever either
	/// 	* A fee editor charges a fee within an epoch; or
	/// 	* The reserve of the pool is insufficient to pay all fees during epoch
	///    execution.
	///
	/// Therefore, the lifetime of this storage is at least one epoch and it is
	/// killed if a pool has sufficient reserve to pay all fees during epoch
	/// execution.

	#[pallet::storage]
	pub type AccruedFees<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		(T::PoolId, FeeBucket),
		Blake2_128Concat,
		u32,
		// TODO: Might require to be changed to BoundedVec if we cannot simply increment
		PoolFeeOf<T>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		/// A pool could not be found.
		PoolNotFound,
		/// Only the PoolAdmin can execute a given operation.
		NotPoolAdmin,
		/// The pool bucket has reached the maximum fees size.
		MaxFeesPerPoolBucket,
		/// The change id does not belong to a pool fees change.
		ChangeIdNotPoolFees,
		/// The change id belongs to a pool fees change but was called in the
		/// wrong context.
		ChangeIdUnrelated,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Propose to append a new fee to the given (pool, bucket) pair.
		///
		/// Origin must be by pool admin.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn propose_new_fee(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			bucket: FeeBucket,
			fee: PoolFeeOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				Error::<T>::NotPoolAdmin
			);

			T::ChangeGuard::note(
				pool_id,
				Change::AppendFee(bucket.clone(), fee.clone()).into(),
			)?;

			Ok(())
		}

		/// Execute a successful fee append proposal for the given (pool,
		/// bucket) pair.
		///
		/// Origin unrestriced due to proposal gate.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn apply_new_fee(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			change_id: T::Hash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			let (bucket, fee) = match Self::get_released_change(pool_id, change_id)? {
				Change::AppendFee(bucket, fee) => Ok((bucket, fee)),
				_ => Err(Error::<T>::ChangeIdUnrelated),
			}?;

			FeeStructure::<T>::mutate(pool_id, bucket, |list| list.try_push(fee))
				.map_err(|_| Error::<T>::MaxFeesPerPoolBucket)?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn get_released_change(
			pool_id: T::PoolId,
			change_id: T::Hash,
		) -> Result<Change<T::AccountId, T::BalanceRatio>, DispatchError> {
			T::ChangeGuard::released(pool_id, change_id)?
				.try_into()
				.map_err(|_| Error::<T>::ChangeIdNotPoolFees.into())
		}
	}
}

// TODO impl PoolFees
// TODO: Use PoolFees::pay when executing epoch (could be E
