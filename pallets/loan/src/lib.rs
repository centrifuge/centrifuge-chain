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

//! # Loan pallet
//!
//! This pallet provides functionality for managing loans on Tinlake
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use common_traits::{PoolNAV as TPoolNav, PoolReserve};
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::Get;
use frame_support::sp_runtime::traits::{One, Zero};
use frame_support::storage::types::OptionQuery;
use frame_support::traits::tokens::nonfungibles::{Inspect, Mutate, Transfer};
use frame_support::traits::{EnsureOrigin, Time};
use frame_support::transactional;
use frame_support::{ensure, Parameter};
use frame_system::pallet_prelude::OriginFor;
use frame_system::RawOrigin;
use loan_type::LoanType;
pub use pallet::*;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{CheckedAdd, CheckedSub};
use sp_runtime::traits::{AccountIdConversion, Member};
use sp_runtime::{DispatchError, FixedPointNumber};
use sp_std::convert::TryInto;
use sp_std::{vec, vec::Vec};
#[cfg(feature = "std")]
use std::fmt::Debug;
use types::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod functions;
mod loan_type;
pub mod math;
pub mod types;

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use sp_arithmetic::FixedPointNumber;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The ClassId type
		type ClassId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ IsType<ClassIdOf<Self>>;

		/// The LoanId/InstanceId type
		type LoanId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ From<u128>
			+ IsType<InstanceIdOf<Self>>;

		/// the rate type
		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber;

		/// the amount type
		type Amount: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ Into<ReserveBalanceOf<Self>>;

		/// The NonFungible trait that can mint, transfer, and inspect assets.
		type NonFungible: Transfer<Self::AccountId> + Mutate<Self::AccountId>;

		/// A way for use to fetch the time of the current block
		type Time: frame_support::traits::Time;

		/// PalletID of this loan module
		#[pallet::constant]
		type LoanPalletId: Get<PalletId>;

		/// Origin for admin that can activate a loan
		type AdminOrigin: EnsureOrigin<Self::Origin>;

		/// Pool reserve type
		type PoolReserve: PoolReserve<Self::Origin, Self::AccountId>;
	}

	/// Stores the loan nft class ID against a given pool
	#[pallet::storage]
	#[pallet::getter(fn get_loan_nft_class)]
	pub(crate) type PoolToLoanNftClass<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, T::ClassId, OptionQuery>;

	/// Stores the poolID against ClassId as a key
	/// this is a reverse lookup used to ensure the collateral itself is not a Loan Nft
	#[pallet::storage]
	pub(crate) type LoanNftClassToPool<T: Config> =
		StorageMap<_, Blake2_128Concat, T::ClassId, PoolIdOf<T>, OptionQuery>;

	#[pallet::type_value]
	pub fn OnNextLoanIdEmpty() -> u128 {
		// always start the token ID from 1 instead of zero
		1
	}

	/// Stores the next loan tokenID to be issued
	#[pallet::storage]
	#[pallet::getter(fn get_next_loan_id)]
	pub(crate) type NextLoanId<T: Config> = StorageValue<_, u128, ValueQuery, OnNextLoanIdEmpty>;

	/// Stores the loan info for given pool and loan id
	#[pallet::storage]
	#[pallet::getter(fn get_loan_info)]
	pub(crate) type LoanInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		LoanData<T::Rate, T::Amount, AssetOf<T>>,
		OptionQuery,
	>;

	/// Stores the pool nav against poolId
	#[pallet::storage]
	#[pallet::getter(fn nav)]
	pub(crate) type PoolNAV<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, NAVDetails<T::Amount>, OptionQuery>;

	/// Stores the pool associated with the its write off groups
	#[pallet::storage]
	#[pallet::getter(fn pool_writeoff_groups)]
	pub(crate) type PoolWriteOffGroups<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, Vec<WriteOffGroup<T::Rate>>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// emits when a new loan is issued for a given
		LoanIssued(PoolIdOf<T>, T::LoanId, AssetOf<T>),

		/// emits when a loan is closed
		LoanClosed(PoolIdOf<T>, T::LoanId, AssetOf<T>),

		/// emits when the loan is activated
		LoanActivated(PoolIdOf<T>, T::LoanId),

		/// emits when some amount is borrowed
		LoanAmountBorrowed(PoolIdOf<T>, T::LoanId, T::Amount),

		/// emits when some amount is repaid
		LoanAmountRepaid(PoolIdOf<T>, T::LoanId, T::Amount),

		/// Emits when NAV is updated for a given pool
		NAVUpdated(PoolIdOf<T>, T::Amount),

		/// Emits when a write off group is added to the given pool with its index
		WriteOffGroupAdded(PoolIdOf<T>, u32),

		/// Emits when a loan is written off
		LoanWrittenOff(PoolIdOf<T>, T::LoanId, u32),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when pool is not initialised
		ErrPoolNotInitialised,

		/// Emits when pool is already initialised
		ErrPoolAlreadyInitialised,

		/// Emits when loan doesn't exist.
		ErrMissingLoan,

		/// Emits when the borrowed amount is more than ceiling
		ErrLoanCeilingReached,

		/// Emits when the addition of borrowed amount overflowed
		ErrAddAmountOverflow,

		/// Emits when principal debt calculation failed due to overflow
		ErrPrincipalDebtOverflow,

		/// Emits when tries to update an active loan
		ErrLoanIsActive,

		/// Emits when loan type given is not valid
		ErrLoanTypeInvalid,

		/// Emits when operation is done on an inactive loan
		ErrLoanNotActive,

		/// Emits when epoch time is overflowed
		ErrEpochTimeOverflow,

		/// Emits when the NFT owner is not found
		ErrNFTOwnerNotFound,

		/// Emits when nft owner doesn't match the expected owner
		ErrNotAssetOwner,

		/// Emits when the nft is not an acceptable asset
		ErrNotAValidAsset,

		/// Emits when the nft token nonce is overflowed
		ErrNftTokenNonceOverflowed,

		/// Emits when loan amount not repaid but trying to close loan
		ErrLoanNotRepaid,

		/// Emits when maturity has passed and borrower tried to borrow more
		ErrLoanMaturityDatePassed,

		/// Emits when a loan data value is invalid
		ErrLoanValueInvalid,

		/// Emits when loan accrue calculation failed
		ErrLoanAccrueFailed,

		/// Emits when loan present value calculation failed
		ErrLoanPresentValueFailed,

		/// Emits when trying to write off of a healthy loan
		ErrLoanHealthy,

		/// Emits when trying to write off loan that was written off by admin already
		ErrLoanWrittenOffByAdmin,

		/// Emits when there is no valid write off group available for unhealthy loan
		ErrNoValidWriteOffGroup,

		/// Emits when there is no valid write off groups associated with given index
		ErrInvalidWriteOffGroupIndex,

		/// Emits when new write off group is invalid
		ErrInvalidWriteOffGroup,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Initiates a new pool and maps the poolId with the loan nft classId
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn initialise_pool(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_nft_class_id: T::ClassId,
		) -> DispatchResult {
			// ensure admin is the origin
			T::AdminOrigin::ensure_origin(origin)?;

			// ensure pool is not initialised yet
			ensure!(
				!PoolToLoanNftClass::<T>::contains_key(pool_id),
				Error::<T>::ErrPoolAlreadyInitialised
			);

			PoolToLoanNftClass::<T>::insert(pool_id, loan_nft_class_id);
			LoanNftClassToPool::<T>::insert(loan_nft_class_id, pool_id);
			Ok(())
		}

		/// Issues a new loan against the asset provided
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn issue_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			asset: AssetOf<T>,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let loan_id = Self::issue(pool_id, owner, asset)?;
			Self::deposit_event(Event::<T>::LoanIssued(pool_id, loan_id, asset));
			Ok(())
		}

		/// Closes a given loan if repaid fully
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn close_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let asset = Self::close(pool_id, loan_id, owner)?;
			Self::deposit_event(Event::<T>::LoanClosed(pool_id, loan_id, asset));
			Ok(())
		}

		/// borrows some amount from an active loan
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn borrow(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Amount,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			Self::borrow_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::LoanAmountBorrowed(pool_id, loan_id, amount));
			Ok(())
		}

		/// repays some amount to an active loan
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Amount,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let repaid_amount = Self::repay_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::LoanAmountRepaid(
				pool_id,
				loan_id,
				repaid_amount,
			));
			Ok(())
		}

		/// a call to update loan specific details and activates the loan
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn activate_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			rate_per_sec: T::Rate,
			loan_type: LoanType<T::Rate, T::Amount>,
		) -> DispatchResult {
			<T as Config>::AdminOrigin::ensure_origin(origin)?;
			Self::activate(pool_id, loan_id, rate_per_sec, loan_type)?;
			Self::deposit_event(Event::<T>::LoanActivated(pool_id, loan_id));
			Ok(())
		}

		/// a call to update nav for a given pool
		/// TODO(ved): benchmarking this to get a weight would be tricky due to n loans per pool
		/// Maybe utility pallet would be a good source of inspiration?
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn update_nav(origin: OriginFor<T>, pool_id: PoolIdOf<T>) -> DispatchResult {
			// ensure signed so that caller pays for the update fees
			ensure_signed(origin)?;
			let updated_nav = Self::update_nav_of_pool(pool_id)?;
			Self::deposit_event(Event::<T>::NAVUpdated(pool_id, updated_nav));
			Ok(())
		}

		/// a call to add a new write off group for a given pool
		/// write off groups are always append only
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn add_write_off_group_to_pool(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			group: WriteOffGroup<T::Rate>,
		) -> DispatchResult {
			// ensure this is coming from an admin origin
			<T as Config>::AdminOrigin::ensure_origin(origin)?;
			let index = Self::add_write_off_group(pool_id, group)?;
			Self::deposit_event(Event::<T>::WriteOffGroupAdded(pool_id, index));
			Ok(())
		}

		/// a call to write off an unhealthy loan
		/// a valid write off group will be chosen based on the loan overdue date since maturity
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn write_off_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResult {
			// ensure this is a signed call
			ensure_signed(origin)?;

			// try to write off
			let index = Self::write_off(pool_id, loan_id, None)?;
			Self::deposit_event(Event::<T>::LoanWrittenOff(pool_id, loan_id, index));
			Ok(())
		}

		/// a admin call to write off an unhealthy loan
		/// write_off_index is overwritten to the loan and the is fixed until changes it with another call.
		#[pallet::weight(100_000)]
		#[transactional]
		pub fn admin_write_off_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			write_off_index: u32,
		) -> DispatchResult {
			// ensure this is a call from admin
			<T as Config>::AdminOrigin::ensure_origin(origin)?;

			// try to write off
			let index = Self::write_off(pool_id, loan_id, Some(write_off_index))?;
			Self::deposit_event(Event::<T>::LoanWrittenOff(pool_id, loan_id, index));
			Ok(())
		}
	}
}

impl<T: Config> TPoolNav<PoolIdOf<T>, T::Amount> for Pallet<T> {
	fn nav(pool_id: PoolIdOf<T>) -> Option<(T::Amount, u64)> {
		PoolNAV::<T>::get(pool_id)
			.and_then(|nav_details| Some((nav_details.latest_nav, nav_details.last_updated)))
	}

	fn update_nav(pool_id: PoolIdOf<T>) -> Result<T::Amount, DispatchError> {
		Self::update_nav_of_pool(pool_id)
	}
}

/// Ensure origin that allows only loan pallet account
pub struct EnsureLoanAccount<T>(sp_std::marker::PhantomData<T>);

impl<
		T: pallet::Config,
		Origin: Into<Result<RawOrigin<T::AccountId>, Origin>> + From<RawOrigin<T::AccountId>>,
	> EnsureOrigin<Origin> for EnsureLoanAccount<T>
{
	type Success = T::AccountId;

	fn try_origin(o: Origin) -> Result<Self::Success, Origin> {
		let loan_id = T::LoanPalletId::get().into_account();
		o.into().and_then(|o| match o {
			RawOrigin::Signed(who) if who == loan_id => Ok(loan_id),
			r => Err(Origin::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> Origin {
		let loan_id = T::LoanPalletId::get().into_account();
		Origin::from(RawOrigin::Signed(loan_id))
	}
}
