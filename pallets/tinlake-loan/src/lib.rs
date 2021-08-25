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

use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, Zero};
pub use pallet::*;
use std::fmt::Debug;
use tinlake::traits::{Asset, Collaterale, Loan as LoanTrait, Owner, Reserve};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned};
	use frame_support::{
		dispatch::{DispatchError, DispatchResult},
		pallet_prelude::*,
	};
	use frame_system::pallet_prelude::*;
	use sp_arithmetic::FixedPointNumber;

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching pool id
		/// TODO: We could move this one here to some overarching tinlake_system::Config pallet
		///       that also takes care of incrementing ids. Otherwise, every pallet will need this type
		type PoolId: Member + Default + AtLeast32BitUnsigned + AccountIdConversion<T::AccountId>;

		/// The Ids with which assets are identified here
		type CollateralId: Member + Default + AtLeast32BitUnsigned;

		/// The balance type of this pallet
		type Balance: Member + Default + AtLeast32BitUnsigned;

		/// the rate type
		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber;

		/// The pool we are having here
		type Reserve: Reserve<Self::AccountId> + Owner<Self::PoolId>;

		/// A possible defi integration
		type Maker: Reserve<Self::AccountId>;

		/// This would be the asset/nft pallet
		type Collateral: Collaterale<Self::CollateralId, Self::AccountId>
			+ Asset<Self::CollateralId, Balance = Self::Balance>;

		/// The account, or accounts that will be made the owner of seized collaterals
		type Custodian: Member + Default;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Stores the loan info for given pool and loan id
	#[pallet::storage]
	#[pallet::getter(fn get_loan_info)]
	pub(super) type Loan<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::PoolID,
		Twox64Concat,
		T::LoanID,
		LoanInfo<T::Rate, T::Balance, T::Moment>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// emits when the loan info is updated.
		LoanInfoUpdate(T::PoolID, T::LoanID),

		/// emits when the loan is activated
		LoanActivated(T::PoolID, T::LoanID),

		/// emits when some amount is borrowed again
		LoanAmountBorrowed(T::PoolID, T::LoanID, T::Amount),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when loan doesn't exist.
		ErrMissingLoan,

		/// Emits when the borrowed amount is more than ceiling
		ErrLoanCeilingReached,

		/// Emits when the addition of borrowed amount overflowed
		ErrAddBorrowedOverflow,

		/// Emits when the subtraction of ceiling amount under flowed
		ErrSubCeilingUnderflow,

		/// Emits when tries to update an active loan
		ErrLoanIsActive,

		/// Emits when epoch time is overflowed
		ErrEpochOverflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create_loan(
			origin: OriginFor<T>,
			pool: T::PoolId,
			collateral: T::CollateralId,
			amount: T::Balance,
			repayment: RepaymentStyle,
		) -> DispatchResult {
			todo!()
			// Check if origin is owner of pool
			// check if is owner of collateral
			// check if collateral value is enough for the given amount
			// check if repyment if allowed or something
			// check if the reserve (i.e. the current pools reserve) has enough balance to create the
			// pool.
			// transfer the loan-amount to the actual loan from the given reserve via
			// T::Reserve::payout()
			//
			// FUTURE:
			// if pool has not enough balance, but pool has defi integration allowed
			// get the rest of the balance from the defi integration (as it also impleemts Reserve)
		}

		/// TODO: Why would we need this? Shouldn't this be determined at create loan, and if
		/// Sets the loan info for a given loan in a pool
		/// we update the loan details only if its not active
		#[pallet::weight(1_00_000)]
		pub fn update_loan(
			origin: OriginFor<T>,
			pool_id: T::PoolID,
			loan_id: T::LoanID,
			rate: T::Rate,
			principal: T::Amount,
		) -> DispatchResult {
			// TODO(dev): get the origin from the config
			ensure_signed(origin)?;

			// check if the pool exists
			pallet_pool::Pallet::<T>::check_pool(pool_id)?;

			// check if the loan is active
			let loan_info = Loan::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;
			ensure!(!loan_info.is_loan_active(), Error::<T>::ErrLoanIsActive);

			// update the loan info
			Loan::<T>::mutate(pool_id, loan_id, |maybe_loan_info| {
				let mut loan_info = maybe_loan_info.take().unwrap_or_default();
				loan_info.rate_per_sec = rate;
				loan_info.ceiling = principal;
				*maybe_loan_info = Some(loan_info);
			});

			Self::deposit_event(Event::<T>::LoanInfoUpdate(pool_id, loan_id));
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn inspect_loan(origin: OriginFor<T>, loan: LoanId) -> DispatchResult {
			todo!()
			// Allows to insoect a loan from external
			// this would typically allow to seize an underlying asset if the
			// loan is overdue
			// We want somebody to pay for this instead of iterrating over loans all the time upon
			// on-initilaze
			// Maybe we give out some reward for "found" loans that are overdue
		}
	}
}

pub enum RepaymentStyle {
	Bullet,
	Amortizing,
	Interest,
	Annuity,
	Free,
}

/// The data structure for storing loan info
#[derive(Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct LoanInfo<Rate, Amount, Moment> {
	ceiling: Amount,
	borrowed_amount: Amount,
	rate_per_sec: Rate,
	cumulative_rate: Rate,
	normalised_debt: Amount,
	last_updated: Moment,
}

impl<Rate, Amount, Moment> LoanInfo<Rate, Amount, Moment>
where
	Amount: PartialOrd + sp_arithmetic::traits::Zero,
{
	/// returns true if the loan is active
	fn is_loan_active(&self) -> bool {
		self.borrowed_amount > Zero::zero()
	}
}

impl<T: Config> Pallet<T> {
	/// returns the ceiling of the given loan under a given pool.
	pub fn ceiling(pool_id: T::PoolID, loan_id: T::LoanID) -> Option<T::Amount> {
		let maybe_loan_info = Loan::<T>::get(pool_id, loan_id);
		match maybe_loan_info {
			Some(loan_info) => Some(loan_info.ceiling),
			None => None,
		}
	}
}

impl<T: Config> Owner<T::AccountId> for Pallet<T> {
	type Of = T::LoanId;

	fn ownership(of: T::LoanId, who: T::AccountId) -> bool {
		todo!()
	}
}

impl<T: Config> LoanTrait<T::PoolId, T::LoanId> for Pallet<T> {
	type Balance = T::Balance;

	fn borrow(pool: T::PoolId, loan: T::LoanId, amount: Self::Balance) -> DispatchResult {
		todo!()
		// NOTE: Probably the best is to move funds
	}

	fn repay(pool: T::PoolId, loan: T::LoanId, amount: Self::Balance) -> DispatchResult {
		todo!()
		// repay to the actual reserve we were taking the money from in the first place
		// If repayment is to late -> Seize the collateral via the Collateral trait
		// This allows us to seize an underlying asset lazy
	}
}
