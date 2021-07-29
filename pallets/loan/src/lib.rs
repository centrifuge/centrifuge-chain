//! # Loan pallet for runtime
//!
//! This pallet provides functionality for managing loans on Tinlake
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::sp_runtime::traits::Zero;
use sp_arithmetic::traits::CheckedAdd;
use sp_arithmetic::FixedU128;
use std::fmt::Debug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use frame_support::storage::types::OptionQuery;
pub use pallet::*;
use sp_std::convert::TryInto;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod math;

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

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_arithmetic::FixedPointNumber;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_pool::Config + pallet_timestamp::Config
	{
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// the rate type
		type Rate: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber;

		/// the amount type
		type Amount: Parameter + Member + MaybeSerializeDeserialize + FixedPointNumber;
	}

	/// Stores the pool value against the poolID.
	#[pallet::storage]
	#[pallet::getter(fn get_pool_value)]
	pub(super) type PoolValue<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolID, T::Amount, OptionQuery>;

	/// Stores the loan info for given pool and loan id
	#[pallet::storage]
	#[pallet::getter(fn get_loan_info)]
	pub(super) type Loan<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolID,
		Blake2_128Concat,
		T::LoanID,
		LoanInfo<T::Rate, T::Amount, T::Moment>,
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
		/// Sets the loan info for a given loan in a pool
		/// we update the loan details only if its not active
		#[pallet::weight(1_00_000)]
		pub fn update_loan_info(
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
	}
}

impl<T: Config> Pallet<T> {
	/// returns the pool value associated with pool id.
	pub fn pool_value(pool_id: T::PoolID) -> Option<T::Amount> {
		PoolValue::<T>::get(pool_id)
	}

	/// returns the ceiling of the given loan under a given pool.
	pub fn ceiling(pool_id: T::PoolID, loan_id: T::LoanID) -> Option<T::Amount> {
		let maybe_loan_info = Loan::<T>::get(pool_id, loan_id);
		match maybe_loan_info {
			Some(loan_info) => Some(loan_info.ceiling),
			None => None,
		}
	}

	pub fn borrow(pool_id: T::PoolID, loan_id: T::LoanID, amount: T::Amount) -> DispatchResult {
		let loan_info = Loan::<T>::get(pool_id, loan_id).ok_or(Error::<T>::ErrMissingLoan)?;

		ensure!(
			loan_info.ceiling <= amount + loan_info.borrowed_amount,
			Error::<T>::ErrLoanCeilingReached
		);

		let new_borrowed_amount = loan_info
			.borrowed_amount
			.checked_add(&amount)
			.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		let nowt = <pallet_timestamp::Pallet<T>>::get();
		let now: u64 = TryInto::<u64>::try_into(nowt).or(Err(Error::<T>::ErrEpochOverflow))?;
		let last_updated: u64 = TryInto::<u64>::try_into(loan_info.last_updated)
			.or(Err(Error::<T>::ErrEpochOverflow))?;
		let new_chi = math::calculate_cumulative_rate::<T::Rate>(
			loan_info.rate_per_sec,
			loan_info.cumulative_rate,
			now,
			last_updated,
		)
		.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		let debt =
			math::debt::<T::Amount, T::Rate>(loan_info.normalised_debt, loan_info.cumulative_rate)
				.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		let new_pie = math::calculate_normalised_debt::<T::Amount, T::Rate>(
			debt,
			math::Adjustment::Inc(amount),
			new_chi,
		)
		.ok_or(Error::<T>::ErrAddBorrowedOverflow)?;

		Loan::<T>::mutate(pool_id, loan_id, |maybe_loan_info| {
			let mut loan_info = maybe_loan_info.take().unwrap_or_default();
			loan_info.borrowed_amount = new_borrowed_amount;
			loan_info.last_updated = nowt;
			loan_info.cumulative_rate = new_chi;
			loan_info.normalised_debt = new_pie;
			*maybe_loan_info = Some(loan_info);
		});

		if loan_info.borrowed_amount == Zero::zero() {
			Self::deposit_event(Event::<T>::LoanActivated(pool_id, loan_id));
		}

		Self::deposit_event(Event::<T>::LoanAmountBorrowed(pool_id, loan_id, amount));

		Ok(())
	}
}
