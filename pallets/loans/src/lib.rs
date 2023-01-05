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
//!
//! Check the next table as an overview of the possible actions you can do for managing loans:
//!
//! |      Action     |   From  |    To   |      Role     | Collateral Owner |
//! |-----------------|---------|---------|---------------|------------------|
//! |      create     |         | Created |    Borrower   |        Yes       |
//! |      price      | Created |  Active | PricingAdmin |                  |
//! |      borrow     |  Active |  Active |               |        Yes       |
//! |      repay      |  Active |  Active |               |        Yes       |
//! |    write_off    |  Active |  Active |               |                  |
//! | admin_write_off |  Active |  Active |   LoanAdmin   |                  |
//! |      close      |  Active |  Closed  |               |        Yes       |

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use std::fmt::Debug;

use cfg_primitives::Moment;
use cfg_traits::{
	InterestAccrual as InterestAccrualT, Permissions as PermissionsT, PoolInspect,
	PoolNAV as TPoolNav, PoolReserve,
};
pub use cfg_types::{
	adjustments::Adjustment,
	permissions::{PermissionScope, PoolRole, Role},
};
use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::Get,
	sp_runtime::traits::{One, Zero},
	traits::{
		tokens::nonfungibles::{Inspect, Mutate, Transfer},
		UnixTime,
	},
	transactional,
};
use frame_system::pallet_prelude::OriginFor;
use loan_type::LoanType;
pub use pallet::*;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{BaseArithmetic, CheckedAdd, CheckedSub};
use sp_runtime::{
	traits::{AccountIdConversion, AtLeast32BitUnsigned, BlockNumberProvider},
	DispatchError, FixedPointNumber, FixedPointOperand,
};
use sp_std::vec;
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
	use frame_support::{pallet_prelude::*, PalletId};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_arithmetic::FixedPointNumber;

	use super::*;
	use crate::weights::WeightInfo;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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
		type Pool: PoolReserve<Self::AccountId, Self::CurrencyId, Balance = Self::Balance>;

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
		#[pallet::constant]
		type MaxActiveLoansPerPool: Get<u32>;

		/// Max number of write-off groups per pool.
		#[pallet::constant]
		type MaxWriteOffGroups: Get<u32>;

		/// Source of the current block number
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;
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
	pub type Loan<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		LoanDetailsOf<T>,
		OptionQuery,
	>;

	#[pallet::storage]
	pub(crate) type ActiveLoans<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		BoundedVec<PricedLoanDetailsOf<T>, T::MaxActiveLoansPerPool>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub(crate) type ClosedLoans<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		Blake2_128Concat,
		T::LoanId,
		PricedLoanDetailsOf<T>,
		OptionQuery,
	>;

	/// Stores the pool nav against poolId
	#[pallet::storage]
	#[pallet::getter(fn nav)]
	pub type PoolNAV<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolIdOf<T>, NAVDetails<T::Balance>, OptionQuery>;

	/// Stores the pool associated with the its write off groups
	#[pallet::storage]
	#[pallet::getter(fn pool_writeoff_groups)]
	pub(crate) type PoolWriteOffGroups<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		PoolIdOf<T>,
		BoundedVec<WriteOffGroup<T::Rate>, T::MaxWriteOffGroups>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A pool was initialised.
		PoolInitialised { pool_id: PoolIdOf<T> },
		/// A loan was created.
		Created {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			collateral: AssetOf<T>,
		},
		/// A loan was closed.
		Closed {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			collateral: AssetOf<T>,
		},
		/// A loan was priced.
		Priced {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			interest_rate_per_sec: T::Rate,
			loan_type: LoanType<T::Rate, T::Balance>,
		},
		/// An amount was borrowed for a loan.
		Borrowed {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		},
		/// An amount was repaid for a loan.
		Repaid {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		},
		/// The NAV for a pool was updated.
		NAVUpdated {
			pool_id: PoolIdOf<T>,
			nav: T::Balance,
			update_type: NAVUpdateType,
		},
		/// A write-off group was added to a pool.
		WriteOffGroupAdded {
			pool_id: PoolIdOf<T>,
			write_off_group_index: u32,
		},
		/// A loan was written off. [pool, loan, percentage, penalty_interest_rate_per_sec, write_off_group_index]
		WrittenOff {
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			percentage: T::Rate,
			penalty_interest_rate_per_sec: T::Rate,
			write_off_group_index: Option<u32>,
		},
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
		/// Emits when tries to price a closed loan
		LoanIsClosed,
		/// Emits when loan type given is not valid
		LoanTypeInvalid,
		/// Emits when operation is done on an inactive loan
		LoanNotActive,
		/// Emits when borrow and repay happens in the same block
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
			Self::ensure_role(pool_id, ensure_signed(origin)?, PoolRole::PoolAdmin)?;

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
			Self::deposit_event(Event::<T>::PoolInitialised { pool_id });
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
			let owner = ensure_signed(origin)?;
			Self::ensure_role(pool_id, owner.clone(), PoolRole::Borrower)?;
			let loan_id = Self::create_loan(pool_id, owner, collateral)?;
			Self::deposit_event(Event::<T>::Created {
				pool_id,
				loan_id,
				collateral,
			});
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
			<T as Config>::WeightInfo::repay_and_close(T::MaxActiveLoansPerPool::get()).max(
				<T as Config>::WeightInfo::write_off_and_close(T::MaxActiveLoansPerPool::get())
			)
		)]
		#[transactional]
		pub fn close(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;
			let (
				active_count,
				ClosedLoan {
					collateral,
					written_off,
				},
			) = Self::close_loan(pool_id, loan_id, owner)?;
			Self::deposit_event(Event::<T>::Closed {
				pool_id,
				loan_id,
				collateral,
			});

			let weight = if written_off {
				T::WeightInfo::write_off_and_close(active_count)
			} else {
				T::WeightInfo::repay_and_close(active_count)
			};
			Ok(Some(weight).into())
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
			<T as Config>::WeightInfo::initial_borrow(T::MaxActiveLoansPerPool::get()).max(
				<T as Config>::WeightInfo::further_borrows(T::MaxActiveLoansPerPool::get())
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
			let (active_count, first_borrow) =
				Self::borrow_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::Borrowed {
				pool_id,
				loan_id,
				amount,
			});

			let weight = if first_borrow {
				T::WeightInfo::initial_borrow(active_count)
			} else {
				T::WeightInfo::further_borrows(active_count)
			};
			Ok(Some(weight).into())
		}

		/// Transfers amount borrowed to the pool reserve.
		///
		/// LoanStatus must be Active.
		/// Loan is accrued before transferring the amount to reserve.
		/// If the repaying amount is more than current debt, only current debt is transferred.
		/// Amount of token will be transferred from owner to Pool reserve.
		#[pallet::weight(<T as Config>::WeightInfo::repay(T::MaxActiveLoansPerPool::get()))]
		#[transactional]
		pub fn repay(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;
			let (active_count, total_repaid) = Self::repay_amount(pool_id, loan_id, owner, amount)?;
			Self::deposit_event(Event::<T>::Repaid {
				pool_id,
				loan_id,
				amount: total_repaid,
			});
			Ok(Some(T::WeightInfo::repay(active_count)).into())
		}

		/// Set pricing for the loan with loan specific details like Rate, Loan type
		///
		/// LoanStatus must be in Created or Active state.
		/// Once activated, loan owner can start loan related functions like Borrow, Repay, Close
		/// `interset_rate_per_year` is the anual interest rate, in the form 0.XXXX,
		///     such that an APR of XX.YY% becomes 0.XXYY. Valid values are 0.0001
		///     through 0.9999, with no more than four significant figures.
		#[pallet::weight(<T as Config>::WeightInfo::price(T::MaxActiveLoansPerPool::get()))]
		pub fn price(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			interest_rate_per_year: T::Rate,
			loan_type: LoanType<T::Rate, T::Balance>,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;

			let (active_count, interest_rate_per_sec) = Loan::<T>::try_mutate(
				pool_id,
				loan_id,
				|loan| -> Result<(u32, T::Rate), DispatchError> {
					let loan = loan.as_mut().ok_or(Error::<T>::MissingLoan)?;

					match loan.status {
						LoanStatus::Created => {
							Self::ensure_role(pool_id, owner, PoolRole::PricingAdmin)?;
							let res = Self::price_created_loan(
								pool_id,
								loan_id,
								interest_rate_per_year,
								loan_type,
							);

							loan.status = LoanStatus::Active;
							res
						}
						LoanStatus::Active => {
							Self::ensure_role(pool_id, owner, PoolRole::LoanAdmin)?;
							Self::price_active_loan(
								pool_id,
								loan_id,
								interest_rate_per_year,
								loan_type,
							)
						}
						LoanStatus::Closed { .. } => Err(Error::<T>::LoanIsClosed)?,
					}
				},
			)?;

			Self::deposit_event(Event::<T>::Priced {
				pool_id,
				loan_id,
				interest_rate_per_sec,
				loan_type,
			});

			Ok(Some(T::WeightInfo::price(active_count)).into())
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
		#[pallet::weight(T::WeightInfo::update_nav(T::MaxActiveLoansPerPool::get()))]
		pub fn update_nav(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
		) -> DispatchResultWithPostInfo {
			// ensure signed so that caller pays for the update fees
			ensure_signed(origin)?;
			let (active_count, nav) = Self::update_nav_of_pool(pool_id)?;
			Self::deposit_event(Event::<T>::NAVUpdated {
				pool_id,
				nav,
				update_type: NAVUpdateType::Exact,
			});

			Ok(Some(T::WeightInfo::update_nav(active_count)).into())
		}

		/// Appends a new write off group to the Pool
		///
		/// `group.penalty_interest_rate_per_year` is a yearly
		/// rate, in the same format as used for pricing
		/// loans.
		///
		/// Since written off loans keep written off group index,
		/// we only allow adding new write off groups.
		/// Overdue days doesn't need to be in the sorted order.
		#[pallet::weight(<T as Config>::WeightInfo::add_write_off_group())]
		pub fn add_write_off_group(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			group: WriteOffGroupInput<T::Rate>,
		) -> DispatchResult {
			// ensure sender has the risk admin role in the pool
			Self::ensure_role(pool_id, ensure_signed(origin)?, PoolRole::LoanAdmin)?;

			// Convert percentage from a yearly rate to a per-second rate.
			let WriteOffGroupInput {
				percentage,
				overdue_days,
				penalty_interest_rate_per_year,
			} = group;
			let penalty_interest_rate_per_sec =
				T::InterestAccrual::convert_additive_rate_to_per_sec(
					penalty_interest_rate_per_year,
				)?;
			let group = WriteOffGroup {
				percentage,
				overdue_days,
				penalty_interest_rate_per_sec,
			};

			let write_off_group_index = Self::add_write_off_group_to_pool(pool_id, group)?;
			Self::deposit_event(Event::<T>::WriteOffGroupAdded {
				pool_id,
				write_off_group_index,
			});
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
		#[pallet::weight(<T as Config>::WeightInfo::write_off(T::MaxActiveLoansPerPool::get(), T::MaxWriteOffGroups::get()))]
		pub fn write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
		) -> DispatchResultWithPostInfo {
			// ensure this is a signed call
			ensure_signed(origin)?;

			// try to write off
			let (active_count, (write_off_group_index, percentage, penalty_interest_rate_per_sec)) =
				Self::write_off_loan(pool_id, loan_id, WriteOffAction::WriteOffToCurrentGroup)?;
			Self::deposit_event(Event::<T>::WrittenOff {
				pool_id,
				loan_id,
				percentage,
				penalty_interest_rate_per_sec,
				write_off_group_index,
			});

			// since the write off group index is picked in loop sequentially,
			// total loops = index+1. This cannot overflow since it is
			// capped by `MaxWriteOffGroups`
			let count = write_off_group_index
				.expect("non-admin write off always returns an index. qed")
				+ 1;
			Ok(Some(T::WeightInfo::write_off(active_count, count)).into())
		}

		/// Write off an loan from admin origin
		///
		/// `admin_write_off_loan` will write off a loan with write off group associated with index passed.
		/// Loan is accrued, NAV is update accordingly, and updates the Loan with new write off index.
		/// AdminOrigin can write off a healthy loan as well.
		/// Once admin writes off a loan, permission less `write_off_loan` wont be allowed after.
		/// Admin can write off loan with any index potentially going up the index or down.
		///
		/// `penalty_interest_rate_per_year` is specified in the same format as used for pricing loans.
		#[pallet::weight(<T as Config>::WeightInfo::admin_write_off(T::MaxActiveLoansPerPool::get()))]
		pub fn admin_write_off(
			origin: OriginFor<T>,
			pool_id: PoolIdOf<T>,
			loan_id: T::LoanId,
			percentage: T::Rate,
			penalty_interest_rate_per_year: T::Rate,
		) -> DispatchResultWithPostInfo {
			// ensure this is a call from risk admin
			Self::ensure_role(pool_id, ensure_signed(origin)?, PoolRole::LoanAdmin)?;
			let penalty_interest_rate_per_sec =
				T::InterestAccrual::convert_additive_rate_to_per_sec(
					penalty_interest_rate_per_year,
				)?;

			// try to write off
			let (active_count, (.., percentage, penalty_interest_rate_per_sec)) =
				Self::write_off_loan(
					pool_id,
					loan_id,
					WriteOffAction::WriteOffAsAdmin {
						percentage,
						penalty_interest_rate_per_sec,
					},
				)?;
			Self::deposit_event(Event::<T>::WrittenOff {
				pool_id,
				loan_id,
				percentage,
				penalty_interest_rate_per_sec,
				write_off_group_index: None,
			});
			Ok(Some(T::WeightInfo::admin_write_off(active_count)).into())
		}
	}
}

impl<T: Config> TPoolNav<PoolIdOf<T>, T::Balance> for Pallet<T> {
	type ClassId = T::ClassId;
	type RuntimeOrigin = T::RuntimeOrigin;

	fn nav(pool_id: PoolIdOf<T>) -> Option<(T::Balance, Moment)> {
		PoolNAV::<T>::get(pool_id).map(|nav_details| (nav_details.latest, nav_details.last_updated))
	}

	fn update_nav(pool_id: PoolIdOf<T>) -> Result<T::Balance, DispatchError> {
		let (_, updated_nav) = Self::update_nav_of_pool(pool_id)?;
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
