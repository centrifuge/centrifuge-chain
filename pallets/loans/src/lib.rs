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
use common_traits::{InterestAccrual as InterestAccrualT, Permissions as PermissionsT};
use common_traits::{PoolInspect, PoolNAV as TPoolNav, PoolReserve};
pub use common_types::{Adjustment, Moment, PermissionScope, PoolRole, Role};
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
use sp_arithmetic::traits::{BaseArithmetic, CheckedAdd, CheckedSub};
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned, Member};
use sp_runtime::{DispatchError, FixedPointNumber, FixedPointOperand};
use sp_std::{convert::TryInto, vec, vec::Vec};
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
pub mod loan_type;
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
	#[pallet::without_storage_info]
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

		/// The NonFungible trait that can mint, transfer, and inspect assets.
		type NonFungible: Transfer<Self::AccountId> + Mutate<Self::AccountId>;

		/// A way for use to fetch the time of the current block
		type Time: UnixTime;

		/// PalletID of this loan module
		#[pallet::constant]
		type LoansPalletId: Get<PalletId>;

		/// Pool reserve type
		type Pool: PoolReserve<Self::AccountId, Balance = Self::Balance>;

		type CurrencyId: Parameter + Copy;

		/// Permission type that verifies permissions of users
		type Permission: PermissionsT<
			Self::AccountId,
			Scope = PermissionScope<PoolIdOf<Self>, Self::CurrencyId>,
			Role = Role,
			Error = DispatchError,
		>;

		type InterestAccrual: InterestAccrualT<Self::Rate, Self::Balance, Adjustment<Self::Balance>>;

		/// Weight info trait for extrinsics
		type WeightInfo: WeightInfo;

		/// Max number of active loans per pool.
		type MaxActiveLoansPerPool: Get<u64>;

		/// Max number of write-off groups per pool.
		type MaxWriteOffGroups: Get<u32>;
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

	/// Stores the next loan tokenID to be created
	#[pallet::storage]
	#[pallet::getter(fn get_next_loan_id)]
	pub(crate) type NextLoanId<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, u128, ValueQuery, OnNextLoanIdEmpty>;

	/// Stores the loan info for given pool and loan id
	#[pallet::storage]
	#[pallet::getter(fn get_loan)]
	pub(crate) type Loan<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		LoanDetails<AssetOf<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub(crate) type ActiveLoans<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Vec<ActiveLoanDetails<T::LoanId, T::Rate, T::Balance, NormalizedDebtOf<T>>>,
		ValueQuery,
	>;

	/// Stores the pool nav against poolId
	#[pallet::storage]
	#[pallet::getter(fn nav)]
	pub(crate) type PoolNAV<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, NAVDetails<T::Balance>, OptionQuery>;

	/// Stores the pool associated with the its write off groups
	#[pallet::storage]
	#[pallet::getter(fn pool_writeoff_groups)]
	pub(crate) type PoolWriteOffGroups<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, Vec<WriteOffGroup<T::Rate>>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A pool was initialised. [pool]
		PoolInitialised(PoolIdOf<T>),
		/// A loan was created. [pool, loan, collateral]
		Created(PoolIdOf<T>, T::LoanId, AssetOf<T>),
		/// A loan was closed. [pool, loan, collateral]
		Closed(PoolIdOf<T>, T::LoanId, AssetOf<T>),
		/// A loan was priced. [pool, loan, interest_rate_per_sec, loan_type]
		Priced(
			PoolIdOf<T>,
			T::LoanId,
			T::Rate,
			LoanType<T::Rate, T::Balance>,
		),
		/// An amount was borrowed for a loan. [pool, loan, amount]
		Borrowed(PoolIdOf<T>, T::LoanId, T::Balance),
		/// An amount was repaid for a loan. [pool, loan, amount]
		Repaid(PoolIdOf<T>, T::LoanId, T::Balance),
		/// The NAV for a pool was updated. [pool, nav, update_type]
		NAVUpdated(PoolIdOf<T>, T::Balance, NAVUpdateType),
		/// A write-off group was added to a pool. [pool, write_off_group]
		WriteOffGroupAdded(PoolIdOf<T>, u32),
		/// A loan was written off. [pool, loan, percentage, penalty_interest_rate_per_sec]
		WrittenOff(PoolIdOf<T>, T::LoanId, percentage, penalty_interest_rate_per_sec),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when pool doesn't exist
		PoolMissing,
		/// Emits when pool is not initialised
		PoolNotInitialised,
		/// Emits when pool is already initialised
		PoolAlreadyInitialised,
		/// Emits when loan doesn't exist.
		MissingLoan,
		/// Emits when the borrowed amount is more than max_borrow_amount
		MaxBorrowAmountExceeded,
		/// Emits when an operation lead to the number overflow
		ValueOverflow,
		/// Emits when principal debt calculation failed due to overflow
		NormalizedDebtOverflow,
		/// Emits when tries to update an active loan
		LoanIsActive,
		/// Emits when loan type given is not valid
		LoanTypeInvalid,
		/// Emits when operation is done on an inactive loan
		LoanNotActive,
		// Emits when borrow and repay happens in the same block
		RepayTooEarly,
		/// Emits when the NFT owner is not found
		NFTOwnerNotFound,
		/// Emits when nft owner doesn't match the expected owner
		NotAssetOwner,
		/// Emits when the nft is not an acceptable asset
		NotAValidAsset,
		/// Emits when the nft token nonce is overflowed
		NftTokenNonceOverflowed,
		/// Emits when loan amount not repaid but trying to close loan
		LoanNotRepaid,
		/// Emits when maturity has passed and borrower tried to borrow more
		LoanMaturityDatePassed,
		/// Emits when a loan data value is invalid
		LoanValueInvalid,
		/// Emits when loan accrue calculation failed
		LoanAccrueFailed,
		/// Emits when loan present value calculation failed
		LoanPresentValueFailed,
		/// Emits when trying to write off of a healthy loan
		LoanHealthy,
		/// Emits when trying to write off loan that was written off by admin already
		WrittenOffByAdmin,
		/// Emits when there is no valid write off group available for unhealthy loan
		NoValidWriteOffGroup,
		/// Emits when there is no valid write off groups associated with given index
		InvalidWriteOffGroupIndex,
		/// Emits when new write off group is invalid
		InvalidWriteOffGroup,
		/// Emits when the max number of write off groups was reached
		TooManyWriteOffGroups,
		/// Emits when the max number of active loans was reached
		TooManyActiveLoans,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Initialises a new pool
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
			ensure!(T::Pool::pool_exists(pool_id), Error::<T>::PoolMissing);

			// ensure pool is not initialised yet
			ensure!(
				!PoolToLoanNftClass::<T>::contains_key(pool_id),
				Error::<T>::PoolAlreadyInitialised
			);

			PoolToLoanNftClass::<T>::insert(pool_id, loan_nft_class_id);
			LoanNftClassToPool::<T>::insert(loan_nft_class_id, pool_id);
			let now = Self::now();
			PoolNAV::<T>::insert(
				pool_id,
				NAVDetails {
					latest: Default::default(),
					last_updated: now,
				},
			);
			Self::deposit_event(Event::<T>::PoolInitialised(pool_id));
			Ok(())
		}

		/// Create a new loan against the collateral provided
		///
		/// `create_loan` transfers the collateral nft from the owner to self and issues a new loan nft to the owner
		/// caller *must* be the owner of the collateral.
		/// LoanStatus is set to created and needs to be priced by an admin origin to start borrowing.
		/// Loan cannot be closed until the status has changed to Priced.
		/// Collateral NFT class cannot be another Loan NFT class. Means, you cannot collateralise a Loan.
		#[pallet::weight(<T as Config>::WeightInfo::create())]
		#[transactional]
		pub fn create(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			collateral: AssetOf<T>,
		) -> DispatchResult {
			// ensure borrower is whitelisted.
			let owner = ensure_role!(pool_id, origin, PoolRole::Borrower);
			let loan_id = Self::create_loan(pool_id, owner, collateral)?;
			Self::deposit_event(Event::<T>::Created(pool_id, loan_id, collateral));
			Ok(())
		}

		/// Closes a given loan
		///
		/// Loan can be closed on two scenarios
		/// 1. When the outstanding is fully paid off
		/// 2. When loan is written off 100%
		/// Loan status is moved to Closed
		/// Collateral NFT is transferred back to the loan owner.
		/// Loan NFT is transferred back to LoanAccount.
		#[pallet::weight(
			<T as Config>::WeightInfo::repay_and_close().max(
				<T as Config>::WeightInfo::write_off_and_close()
			)
		)]
		#[transactional]
		pub fn close(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;
			let ClosedLoan {
				collateral,
				written_off,
			} = Self::close_loan(pool_id, loan_id, owner)?;
			Self::deposit_event(Event::<T>::Closed(pool_id, loan_id, collateral));
			match written_off {
				true => Ok(Some(T::WeightInfo::write_off_and_close()).into()),
				false => Ok(Some(T::WeightInfo::repay_and_close()).into()),
			}
		}

		/// Transfers borrow amount to the loan owner.
		///
		/// LoanStatus must be active.
		/// Borrow amount should not exceed max_borrow_amount set for the loan.
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
			amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;
			let first_borrow = Self::borrow_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::Borrowed(pool_id, loan_id, amount));
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
			amount: T::Balance,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let total_repaid = Self::repay_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::Repaid(pool_id, loan_id, total_repaid));
			Ok(())
		}

		/// Set pricing for the loan with loan specific details like Rate, Loan type
		///
		/// LoanStatus must be in Created state.
		/// Once activated, loan owner can start loan related functions like Borrow, Repay, Close
		#[pallet::weight(<T as Config>::WeightInfo::price())]
		pub fn price(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			interest_rate_per_sec: T::Rate,
			loan_type: LoanType<T::Rate, T::Balance>,
		) -> DispatchResult {
			// ensure sender has the pricing admin role in the pool
			ensure_role!(pool_id, origin, PoolRole::PricingAdmin);
			Self::price_loan(pool_id, loan_id, interest_rate_per_sec, loan_type)?;
			Self::deposit_event(Event::<T>::Priced(
				pool_id,
				loan_id,
				interest_rate_per_sec,
				loan_type,
			));
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
		#[pallet::weight(T::WeightInfo::nav_update_single_loan().saturating_mul(T::MaxActiveLoansPerPool::get()))]
		pub fn update_nav(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
		) -> DispatchResultWithPostInfo {
			// ensure signed so that caller pays for the update fees
			ensure_signed(origin)?;
			let (updated_nav, updated_loans) = Self::update_nav_of_pool(pool_id)?;
			Self::deposit_event(Event::<T>::NAVUpdated(
				pool_id,
				updated_nav,
				NAVUpdateType::Exact,
			));

			// if the total loans updated are more than max loans, we are charging lower txn fees for this pool nav calculation.
			// there is nothing we can do right now. return Ok
			if updated_loans > T::MaxActiveLoansPerPool::get() {
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
		#[pallet::weight(<T as Config>::WeightInfo::add_write_off_group())]
		pub fn add_write_off_group(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			group: WriteOffGroup<T::Rate>,
		) -> DispatchResult {
			// ensure sender has the risk admin role in the pool
			ensure_role!(pool_id, origin, PoolRole::RiskAdmin);
			let index = Self::add_write_off_group_to_pool(pool_id, group)?;
			Self::deposit_event(Event::<T>::WriteOffGroupAdded(pool_id, index));
			Ok(())
		}

		/// Write off an unhealthy loan
		///
		/// `write_off_loan` will find the best write off group available based on the overdue days since maturity.
		/// Loan is accrued, NAV is update accordingly, and updates the Loan with new write off index.
		/// Cannot update a loan that was written off by admin.
		/// Cannot write off a healthy loan or loan type that do not have maturity date.
		///
		///
		/// Weight is calculated for one group. Since there is no extra read or writes for groups more than 1,
		/// We need to ensure we are charging the reads and write only once but the actual compute to be equal to number of groups processed
		#[pallet::weight(Pallet::<T>::write_off_group_weight(T::MaxWriteOffGroups::get() as u64))]
		pub fn write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResultWithPostInfo {
			// ensure this is a signed call
			ensure_signed(origin)?;

			// try to write off
			let (index, percentage, penalty_interest_rate_per_sec) = Self::write_off_loan(pool_id, loan_id, WriteOffAction::WriteOffToCurrentGroup)?;
			Self::deposit_event(Event::<T>::WrittenOff(pool_id, loan_id, percentage, penalty_interest_rate_per_sec));

			// since the write off group index is picked in loop sequentially,
			// total loops = index+1
			let count = index + 1;
			Ok(Some(Self::write_off_group_weight(count as u64)).into())
		}

		/// Write off an loan from admin origin
		///
		/// `admin_write_off_loan` will write off a loan with write off group associated with index passed.
		/// Loan is accrued, NAV is update accordingly, and updates the Loan with new write off index.
		/// AdminOrigin can write off a healthy loan as well.
		/// Once admin writes off a loan, permission less `write_off_loan` wont be allowed after.
		/// Admin can write off loan with any index potentially going up the index or down.
		#[pallet::weight(<T as Config>::WeightInfo::admin_write_off())]
		pub fn admin_write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			percentage: T::Rate,
			penalty_interest_rate_per_sec: T::Rate
		) -> DispatchResult {
			// ensure this is a call from risk admin
			ensure_role!(pool_id, origin, PoolRole::RiskAdmin);
			
			// try to write off
			let index = Self::write_off_loan(pool_id, loan_id, WriteOffAction::WriteOffAsAdmin {
				percentage,
				penalty_interest_rate_per_sec,
			})?;
			Self::deposit_event(Event::<T>::WrittenOff(pool_id, loan_id, percentage, penalty_interest_rate_per_sec));
			Ok(())
		}
	}
}

impl<T: Config> TPoolNav<PoolIdOf<T>, T::Balance> for Pallet<T> {
	type ClassId = T::ClassId;
	type Origin = T::Origin;
	fn nav(pool_id: PoolIdOf<T>) -> Option<(T::Balance, Moment)> {
		PoolNAV::<T>::get(pool_id).map(|nav_details| (nav_details.latest, nav_details.last_updated))
	}

	fn update_nav(pool_id: PoolIdOf<T>) -> Result<T::Balance, DispatchError> {
		let (updated_nav, ..) = Self::update_nav_of_pool(pool_id)?;
		Ok(updated_nav)
	}

	fn initialise(
		origin: OriginFor<T>,
		pool_id: PoolIdOf<T>,
		class_id: T::ClassId,
	) -> DispatchResult {
		Self::initialise_pool(origin, pool_id, class_id)
	}
}

#[macro_export]
macro_rules! ensure_role {
	( $pool_id:expr, $origin:expr, $role:expr $(,)? ) => {{
		let sender = ensure_signed($origin)?;
		ensure!(
			T::Permission::has(
				PermissionScope::Pool($pool_id),
				sender.clone(),
				Role::PoolRole($role)
			),
			BadOrigin
		);
		sender
	}};
}
