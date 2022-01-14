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
use common_traits::Permissions as PermissionsT;
use common_traits::{PoolInspect, PoolNAV as TPoolNav, PoolReserve};
pub use common_types::PoolRole;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::Get;
use frame_support::sp_runtime::traits::{One, Zero};
use frame_support::storage::types::OptionQuery;
use frame_support::traits::tokens::nonfungibles::{Inspect, Mutate, Transfer};
use frame_support::traits::UnixTime;
use frame_support::transactional;
use frame_support::{ensure, Parameter};
use frame_system::pallet_prelude::OriginFor;
use loan_type::LoanType;
pub use pallet::*;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{CheckedAdd, CheckedSub};
use sp_runtime::traits::{AccountIdConversion, Member};
use sp_runtime::{DispatchError, FixedPointNumber};
use sp_std::{vec, vec::Vec};
#[cfg(feature = "std")]
use std::fmt::Debug;
use types::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(any(test, feature = "runtime-benchmarks"))]
pub(crate) mod test_utils;

pub mod functions;
mod loan_type;
pub mod math;
pub mod types;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_arithmetic::FixedPointNumber;
	use sp_runtime::traits::BadOrigin;

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
			+ Default
			+ TypeInfo
			+ IsType<ClassIdOf<Self>>;

		/// The LoanId/InstanceId type
		type LoanId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ TypeInfo
			+ From<u128>
			+ IsType<InstanceIdOf<Self>>;

		/// the rate type
		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber + TypeInfo;

		/// the amount type
		type Amount: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ TypeInfo
			+ Into<ReserveBalanceOf<Self>>;

		/// The NonFungible trait that can mint, transfer, and inspect assets.
		type NonFungible: Transfer<Self::AccountId> + Mutate<Self::AccountId>;

		/// A way for use to fetch the time of the current block
		type Time: UnixTime;

		/// PalletID of this loan module
		#[pallet::constant]
		type LoanPalletId: Get<PalletId>;

		/// Pool reserve type
		type Pool: PoolReserve<Self::AccountId>;

		/// Permission type that verifies permissions of users
		type Permission: PermissionsT<
			Self::AccountId,
			Location = PoolIdOf<Self>,
			Role = PoolRole,
			Error = DispatchError,
		>;

		/// Weight info trait for extrinsics
		type WeightInfo: WeightInfo;

		/// This is a soft limit for maximum loans we can expect in a pool.
		/// this is mainly used to calculate estimated weight for NAV calculation.
		type MaxLoansPerPool: Get<u64>;
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
		/// Emits when a pool is initiated
		PoolInitiated(PoolIdOf<T>),

		/// emits when a new loan is issued for a given
		LoanIssued(PoolIdOf<T>, T::LoanId, AssetOf<T>),

		/// emits when a loan is closed
		LoanClosed(PoolIdOf<T>, T::LoanId, AssetOf<T>),

		/// emits when the loan is activated
		LoanPriceSet(PoolIdOf<T>, T::LoanId),

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
		/// Emits when pool doesn't exist
		ErrPoolMissing,

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

		// Emits when borrow and repay happens in the same block
		ErrRepayTooEarly,

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
		/// Initiates a new pool
		///
		/// `initialise_pool` checks if pool is not initialised yet and then adds the loan nft class id.
		/// All the Loan NFTs will be created into this Class. So loan account *should* be able to mint new NFTs into the class.
		/// Adding LoanAccount as admin to the NFT class will be enough to mint new NFTs.
		/// The origin must be an Admin origin
		#[pallet::weight(<T as Config>::WeightInfo::initialise_pool())]
		pub fn initialise_pool(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_nft_class_id: T::ClassId,
		) -> DispatchResult {
			// ensure the sender has the pool admin role
			ensure_role!(pool_id, origin, PoolRole::PoolAdmin);

			// ensure pool exists
			ensure!(T::Pool::pool_exists(pool_id), Error::<T>::ErrPoolMissing);

			// ensure pool is not initialised yet
			ensure!(
				!PoolToLoanNftClass::<T>::contains_key(pool_id),
				Error::<T>::ErrPoolAlreadyInitialised
			);

			PoolToLoanNftClass::<T>::insert(pool_id, loan_nft_class_id);
			LoanNftClassToPool::<T>::insert(loan_nft_class_id, pool_id);
			let now = Self::time_now();
			PoolNAV::<T>::insert(
				pool_id,
				NAVDetails {
					latest_nav: Default::default(),
					last_updated: now,
				},
			);
			Self::deposit_event(Event::<T>::PoolInitiated(pool_id));
			Ok(())
		}

		/// Issues a new loan against the asset provided
		///
		/// `issue_loan` transfers the asset(collateral) from the owner to self and issues a new loan nft to the owner
		/// caller *must* be the owner of the asset.
		/// LoanStatus is set to issued and needs to be activated by an admin origin to start borrowing.
		/// Loan cannot be closed until the status has changed to Active.
		/// Asset NFT class cannot be another Loan NFT class. Means, you cannot collateralise a Loan.
		#[pallet::weight(<T as Config>::WeightInfo::issue_loan())]
		#[transactional]
		pub fn issue_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			asset: AssetOf<T>,
		) -> DispatchResult {
			// ensure borrower is whitelisted.
			let owner = ensure_role!(pool_id, origin, PoolRole::Borrower);
			let loan_id = Self::issue(pool_id, owner, asset)?;
			Self::deposit_event(Event::<T>::LoanIssued(pool_id, loan_id, asset));
			Ok(())
		}

		/// Closes a given loan
		///
		/// Loan can be closed on two scenarios
		/// 1. When the outstanding is fully paid off
		/// 2. When loan is written off 100%
		/// Loan status is moved to Closed
		/// Asset/Collateral is transferred back to the loan owner.
		/// LoanNFT is transferred back to LoanAccount.
		#[pallet::weight(
			<T as Config>::WeightInfo::repay_and_close().max(
				<T as Config>::WeightInfo::write_off_and_close()
			)
		)]
		#[transactional]
		pub fn close_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;
			let ClosedLoan { asset, written_off } = Self::close(pool_id, loan_id, owner)?;
			Self::deposit_event(Event::<T>::LoanClosed(pool_id, loan_id, asset));
			match written_off {
				true => Ok(Some(T::WeightInfo::write_off_and_close()).into()),
				false => Ok(Some(T::WeightInfo::repay_and_close()).into()),
			}
		}

		/// Transfers borrow amount to the loan owner.
		///
		/// LoanStatus must be active.
		/// Total Borrowed amount(Previously borrowed + requested) should not exceed ceiling set for the loan.
		/// Loan should still be healthy. If loan type supports maturity, then maturity date should not have passed.
		/// Loan should not be written off.
		/// Rate accumulation will start after the first borrow
		/// Loan is accrued upto the current time.
		/// Pool NAV is updated to reflect new present value of the loan.
		/// Amount of tokens of an Asset will be transferred from pool reserve to loan owner.
		#[pallet::weight(
			<T as Config>::WeightInfo::initial_borrow().max(
				<T as Config>::WeightInfo::further_borrows()
			)
		)]
		#[transactional]
		pub fn borrow(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Amount,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;
			let first_borrow = Self::borrow_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::LoanAmountBorrowed(pool_id, loan_id, amount));
			match first_borrow {
				true => Ok(Some(T::WeightInfo::initial_borrow()).into()),
				false => Ok(Some(T::WeightInfo::further_borrows()).into()),
			}
		}

		/// Transfers amount borrowed to the pool reserve.
		///
		/// LoanStatus must be Active.
		/// Loan is accrued before transferring the amount to reserve.
		/// If the repaying amount is more than current debt, only current debt is transferred.
		/// Amount of token will be transferred from owner to Pool reserve.
		#[pallet::weight(<T as Config>::WeightInfo::repay())]
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

		/// Set pricing for the loan with loan specific details like Rate, Loan type
		///
		/// LoanStatus must be in Issued state.
		/// Once activated, loan owner can start loan related functions like Borrow, Repay, Close
		#[pallet::weight(<T as Config>::WeightInfo::price_loan())]
		pub fn price_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			rate_per_sec: T::Rate,
			loan_type: LoanType<T::Rate, T::Amount>,
		) -> DispatchResult {
			// ensure sender has the pricing admin role in the pool
			ensure_role!(pool_id, origin, PoolRole::PricingAdmin);
			Self::price(pool_id, loan_id, rate_per_sec, loan_type)?;
			Self::deposit_event(Event::<T>::LoanPriceSet(pool_id, loan_id));
			Ok(())
		}

		/// Updates the NAV for a given pool
		///
		/// Iterate through each loan and calculate the present value of each active loan.
		/// The loan is accrued and updated.
		///
		/// Weight for the update nav is not straightforward since there could n loans in a pool
		/// So instead, we calculate weight for one loan. We assume a maximum of 200 loans and deposit that weight
		/// Once the NAV calculation is done, we check how many loans we have updated and return the actual weight so that
		/// transaction payment can return the deposit.
		#[pallet::weight(T::WeightInfo::nav_update_single_loan().saturating_mul(T::MaxLoansPerPool::get()))]
		pub fn update_nav(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
		) -> DispatchResultWithPostInfo {
			// ensure signed so that caller pays for the update fees
			ensure_signed(origin)?;
			let (updated_nav, updated_loans) = Self::update_nav_of_pool(pool_id)?;
			Self::deposit_event(Event::<T>::NAVUpdated(pool_id, updated_nav));

			// if the total loans updated are more than max loans, we are charging lower txn fees for this pool nav calculation.
			// there is nothing we can do right now. return Ok
			if updated_loans > T::MaxLoansPerPool::get() {
				return Ok(().into());
			}

			// calculate actual weight of the updating nav based on number of loan processed.
			let total_weight =
				T::WeightInfo::nav_update_single_loan().saturating_mul(updated_loans);
			Ok(Some(total_weight).into())
		}

		/// Appends a new write off group to the Pool
		///
		/// Since written off loans keep written off group index,
		/// we only allow adding new write off groups.
		/// Overdue days doesn't need to be in the sorted order.
		#[pallet::weight(<T as Config>::WeightInfo::add_write_off_group_to_pool())]
		pub fn add_write_off_group_to_pool(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			group: WriteOffGroup<T::Rate>,
		) -> DispatchResult {
			// ensure sender has the risk admin role in the pool
			ensure_role!(pool_id, origin, PoolRole::RiskAdmin);
			let index = Self::add_write_off_group(pool_id, group)?;
			Self::deposit_event(Event::<T>::WriteOffGroupAdded(pool_id, index));
			Ok(())
		}

		/// Write off an unhealthy loan
		///
		/// `write_off_loan` will find the best write off group available based on the overdue days since maturity.
		/// Loan is accrued, NAV is update accordingly, and updates the LoanInfo with new write off index.
		/// Cannot update a loan that was written off by admin.
		/// Cannot write off a healthy loan or loan type that do not have maturity date.
		#[pallet::weight(<T as Config>::WeightInfo::write_off_loan())]
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

		/// Write off an loan from admin origin
		///
		/// `admin_write_off_loan` will write off a loan with write off group associated with index passed.
		/// Loan is accrued, NAV is update accordingly, and updates the LoanInfo with new write off index.
		/// AdminOrigin can write off a healthy loan as well.
		/// Once admin writes off a loan, permission less `write_off_loan` wont be allowed after.
		/// Admin can write off loan with any index potentially going up the index or down.
		#[pallet::weight(<T as Config>::WeightInfo::admin_write_off_loan())]
		pub fn admin_write_off_loan(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			write_off_index: u32,
		) -> DispatchResult {
			// ensure this is a call from risk admin
			ensure_role!(pool_id, origin, PoolRole::RiskAdmin);

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
		let (updated_nav, ..) = Self::update_nav_of_pool(pool_id)?;
		Ok(updated_nav)
	}
}

#[macro_export]
macro_rules! ensure_role {
	( $pool_id:expr, $origin:expr, $role:expr $(,)? ) => {{
		let sender = ensure_signed($origin)?;
		ensure!(
			T::Permission::has_permission($pool_id, sender.clone(), $role),
			BadOrigin
		);
		sender
	}};
}
