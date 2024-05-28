// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! # Interest Accrual Pallet
//!
//! A pallet for calculating interest accrual on debt.
//! It keeps track of different buckets of interest rates and is optimized
//! for many loans per interest bucket. This implementation is inspired
//! by [jug.sol](https://github.com/makerdao/dss/blob/master/src/jug.sol)
//! from Multi Collateral Dai.
//!
//! It works by defining debt = normalized debt * accumulated rate.
//! When the first loan for an interest rate is created, the accumulated
//! rate is set to 1.0. The normalized debt is then calculated, which is
//! the debt at t=0, using debt / accumulated rate.
//!
//! Over time, the accumulated rate grows based on the interest rate per second.
//! Any time the accumulated rate is updated for an interest rate group,
//! this indirectly updates the debt of all loans outstanding using this
//! interest rate.
//!
//! ```text
//!                    ar = accumulated rate
//!                    nd = normalized debt
//!
//!            │
//!        2.0 │                             ****
//!            │                         ****
//!            │                     ****
//!            │                 ****
//!   ar   1.5 │             ****
//!            │         ****
//!            │      ****
//!            │   ****
//!        1.0 │ **
//!            └──────────────────────────────────
//!            │              │
//!
//!            borrow 10      borrow 20
//!            ar   = 1.0     ar   = 1.5
//!            nd   = 10      nd   = 10 + (20 / 1.5) = 23.33
//!            debt = 10      debt = 35
//! ```
//!
//! ## Basics of shared rate accrual and "normalized debts"
//!
//! When we want to compute the interest accrued on some value, the
//! high-level equation is:
//!
//! ```text
//! rate_per_second.pow(debt_age) * debt_base_value
//! ```
//!
//! Computing that pow for everything is expensive, so we want to only
//! do it once for any given `rate_per_second` and share that result
//! across multiple debts. Because these debts might not have been
//! created at the same time as each other (or the rate), we must
//! include a correction factor to the shared interest rate accrual:
//!
//! ```text
//! correction_factor = ???;
//! rate_per_second.pow(rate_age) * debt_base_value / correction_factor
//! ```
//!
//! This correction factor is just the accumulated interest at the
//! time the debt was created:
//!
//! ```text
//! correction_factor = rate_per_second.pow(rate_age_at_time_of_debt_creation);
//! rate_per_second.pow(rate_age) * debt_base_value / correction_factor
//! // Equivalent to:
//! rate_per_second.pow(rate_age - rate_ag_at_time_of_debt_creation) * debt_base_value
//! ```
//!
//! And in the classic trade-off of space vs time complexity, we
//! precompute the correction factor applied to the base debt as the
//! normalized debt
//!
//! ```text
//! normalized_debt = debt_base_value / rate_per_second.pow(rate_age_at_time_of_debt_creation);
//! ```
//!
//! In the actual code, `rate_per_second.pow(...)` will be precomputed
//! for us at block initialize and is just queried as the "accrued
//! rate".
//!
//! The case of `rate_age_at_time_of_debt_creation == 0` creates a
//! correction factor of 1, since no debt has yet accumulated on that
//! rate. This leads to the behavior of `normalize` apparently doing
//! nothing. The debt in that case is "synced" to the interest rate,
//! and doesn't need any correction.
//!
//! ## Renormalization
//!
//! Renormalization is the operation of saying "from now one, I want
//! to accrue this debt at a new rate". Implicit in that is that all
//! previous debt has been accounted for. We are essentially "starting
//! over" with a new base debt - our accrued debt from the old rate -
//! and a new interest rate.
//!
//! ```text
//! current_debt = normalized_debt * accrued_rate(old_interest_rate);
//! normalized_debt = current_debt / accrued_rate(new_interest_rate);
//! ```
//!
//! Two things to note here:
//! * If `old_interest_rate` and `new_interest_rate` are identical, this is a
//!   no-op.
//! * If `new_interest_rate` is newly created (and thus its age is `0`), the
//!   correction factor is `1` just as for any other rate.  See the note above
//!   regarding zero-age rates.

#![cfg_attr(not(feature = "std"), no_std)]

use cfg_traits::{
	interest::{Interest, InterestAccrual, InterestModel, InterestRate},
	time::{Seconds, TimeAsSecs},
};
use cfg_types::adjustments::Adjustment;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::traits::{checked_pow, One, Zero};
use sp_runtime::{
	traits::{
		AtLeast32BitUnsigned, CheckedAdd, CheckedSub, EnsureAdd, EnsureAddAssign, EnsureDiv,
		EnsureInto, EnsureMul, EnsureSub, Saturating,
	},
	DispatchError, FixedPointNumber, FixedPointOperand,
};

pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;

	use super::*;
	use crate::weights::WeightInfo;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(3);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

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

		/// A fixed-point number which represents an interest rate.
		type Rate: Member
			+ Parameter
			+ Default
			+ core::fmt::Debug
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>
			+ MaxEncodedLen;

		type Time: TimeAsSecs;

		type Weights: WeightInfo;
	}

	#[pallet::event]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when the debt calculation failed
		DebtCalculationFailed,
		/// Emits when a rate is not within the valid range
		InvalidRate,
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn calculate_rate(
			interest_rate_per_year: T::Rate,
			accumulated_rate: T::Rate,
			when: Seconds,
		) -> Result<Interest<T::Rate>, DispatchError> {
			todo!()
		}

		pub(crate) fn calculate_debt(
			interest_rate_per_year: T::Rate,
			debt: T::Balance,
			when: Seconds,
		) -> Result<T::Rate, DispatchError> {
			todo!()
		}

		pub(crate) fn validate_interest_rate(
			interest_rate_per_year: &InterestRate<T::Rate>,
		) -> DispatchResult {
			match interest_rate_per_year {
				InterestRate::Fixed { rate_per_year, .. } => Ok(()),
			}
		}
	}
}

impl<T: Config> InterestAccrual<T::Rate, T::Balance, Adjustment<T::Balance>> for Pallet<T> {
	fn calculate_debt(
		interest_rate_per_year: &InterestModel<T::Rate>,
		debt: T::Balance,
		when: Seconds,
	) -> Result<Interest<T::Balance>, DispatchError> {
		Pallet::<T>::calculate_debt(interest_rate_per_year, debt, when)
	}

	fn accumulate_rate(
		interest_rate_per_year: &InterestModel<T::Rate>,
		acc_rate: T::Rate,
		when: Seconds,
	) -> Result<T::Rate, DispatchError> {
		Pallet::<T>::calculate_rate(interest_rate_per_year, acc_rate, when)
	}
}
