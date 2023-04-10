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
//! ```ignore
//! rate_per_second.pow(debt_age) * debt_base_value
//! ```
//!
//! Computing that pow for everything is expensive, so we want to only
//! do it once for any given `rate_per_second` and share that result
//! across multiple debts. Because these debts might not have been
//! created at the same time as each other (or the rate), we must
//! include a correction factor to the shared interest rate accrual:
//!
//! ```ignore
//! correction_factor = ???;
//! rate_per_second.pow(rate_age) * debt_base_value / correction_factor
//! ```
//!
//! This correction factor is just the accumulated interest at the
//! time the debt was created:
//!
//! ```ignore
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
//! ```ignore
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
//! ```ignore
//! current_debt = normalized_debt * accrued_rate(old_interest_rate);
//! normalized_debt = current_debt / accrued_rate(new_interest_rate);
//! ```
//!
//! Two things to note here:
//! * If `old_interest_rate` and `new_interest_rate` are identical,
//!   this is a no-op.
//! * If `new_interest_rate` is newly created (and thus its age is `0`),
//!   the correction factor is `1` just as for any other rate.  See the
//!   note above regarding zero-age rates.

#![cfg_attr(not(feature = "std"), no_std)]
use cfg_primitives::{Moment, SECONDS_PER_YEAR};
use cfg_traits::{
	accrual::{RateAccrual, RateCache},
	ops::{EnsureAdd, EnsureAddAssign, EnsureDiv, EnsureInto, EnsureMul, EnsureSub},
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::UnixTime, RuntimeDebug};
use scale_info::TypeInfo;
use sp_arithmetic::traits::{checked_pow, One, Zero};
use sp_runtime::{traits::Saturating, ArithmeticError, FixedPointNumber};
use sp_std::vec::Vec;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub use pallet::*;

// TODO: This "magic" number can be removed: tracking issue #1297
// For now it comes from `pallet-loans` demands:
// possible interest rates < 1 plus a penalty from [0, 1].
// Which in the worst cases could be near to 2.
const MAX_INTEREST_RATE: u32 = 2; // Which corresponds to 200%.

// Type aliases
type RateDetailsOf<T> = RateDetails<<T as Config>::SecRate>;

#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RateDetails<InterestRate> {
	pub interest_rate_per_sec: InterestRate,
	pub accumulated_rate: InterestRate,
	pub reference_count: u32,
}

#[derive(Encode, Decode, TypeInfo, PartialEq, Eq, MaxEncodedLen, RuntimeDebug)]
#[repr(u32)]
pub enum Release {
	V0,
	V1,
	V2,
}

impl Default for Release {
	fn default() -> Self {
		Self::V0
	}
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;

	use super::*;
	use crate::weights::WeightInfo;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// A fixed-point number which represents an interest rate per year.
		type YearRate: Member
			+ Parameter
			+ Default
			+ core::fmt::Debug
			+ Copy
			+ TypeInfo
			+ FixedPointNumber
			+ MaxEncodedLen
			+ TryInto<Self::SecRate>;

		/// A fixed-point number which represents an interest rate per sec.
		type SecRate: Member
			+ Parameter
			+ Default
			+ core::fmt::Debug
			+ Copy
			+ TypeInfo
			+ FixedPointNumber
			+ MaxEncodedLen;

		type Time: UnixTime;

		type MaxRateCount: Get<u32>;

		type Weights: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn rates)]
	pub(super) type Rates<T: Config> =
		StorageValue<_, BoundedVec<RateDetailsOf<T>, T::MaxRateCount>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn last_updated)]
	pub(super) type LastUpdated<T: Config> = StorageValue<_, Moment, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn storage_version)]
	pub(super) type StorageVersion<T: Config> = StorageValue<_, Release, ValueQuery>;

	#[pallet::event]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when the interest rate was not used
		NoSuchRate,
		/// Emits when a rate is not within the valid range
		InvalidRate,
		/// Emits when adding a new rate would exceed the storage limits
		TooManyRates,
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config>(core::marker::PhantomData<T>);

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self(core::marker::PhantomData)
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			StorageVersion::<T>::put(Release::V2);
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_: T::BlockNumber) -> Weight {
			let then = LastUpdated::<T>::get();
			let now = T::Time::now().as_secs();
			LastUpdated::<T>::set(now);
			let delta = now - then;
			let bits = Moment::BITS - delta.leading_zeros();

			// reads: timestamp, last updated, rates vec
			// writes: last updated, rates vec
			let mut weight = T::DbWeight::get().reads_writes(3, 2);

			let rates = Rates::<T>::get();
			let rates: Vec<_> = rates
				.into_iter()
				.filter_map(|rate| {
					weight.saturating_accrue(T::Weights::calculate_accumulated_rate(bits));

					let RateDetailsOf::<T> {
						interest_rate_per_sec,
						accumulated_rate,
						reference_count,
					} = rate;

					Self::calculate_accumulated_rate(interest_rate_per_sec, accumulated_rate, delta)
						.ok()
						.map(|accumulated_rate| RateDetailsOf::<T> {
							interest_rate_per_sec,
							accumulated_rate,
							reference_count,
						})
				})
				.collect();

			Rates::<T>::set(
				rates
					.try_into()
					.expect("We got this vec from a bounded vec to begin with"),
			);
			weight
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn calculate_accumulated_rate(
			interest_rate_per_sec: T::SecRate,
			accumulated_rate: T::SecRate,
			delta: Moment,
		) -> Result<T::SecRate, ArithmeticError> {
			// accumulated_rate * interest_rate_per_sec ^ (now - last_updated)
			checked_pow(interest_rate_per_sec, delta as usize)
				.ok_or(ArithmeticError::Overflow)? // TODO: This line can be remove once #1241 be merged
				.ensure_mul(accumulated_rate)
		}

		fn get_rate(
			interest_rate_per_year: T::YearRate,
		) -> Result<RateDetailsOf<T>, DispatchError> {
			let interest_rate_per_sec = Self::rate_conversion(interest_rate_per_year)?;
			Rates::<T>::get()
				.into_iter()
				.find(|rate| rate.interest_rate_per_sec == interest_rate_per_sec)
				.ok_or_else(|| Error::<T>::NoSuchRate.into())
		}

		pub(crate) fn rate_conversion(
			interest_rate_per_year: T::YearRate,
		) -> Result<T::SecRate, ArithmeticError> {
			interest_rate_per_year
				.ensure_into()?
				.ensure_div(T::SecRate::saturating_from_integer(SECONDS_PER_YEAR))?
				.ensure_add(One::one())
		}
	}

	impl<T: Config> RateAccrual for Pallet<T> {
		type AccRate = T::SecRate;
		type Cache = RateVec<T>;
		type Moment = Moment;
		type OuterRate = T::YearRate;

		fn accrual(interest_rate_per_year: T::YearRate) -> Result<T::SecRate, DispatchError> {
			Ok(Self::get_rate(interest_rate_per_year)?.accumulated_rate)
		}

		fn accrual_at(
			interest_rate_per_year: T::YearRate,
			when: Self::Moment,
		) -> Result<T::SecRate, DispatchError> {
			let rate = Self::get_rate(interest_rate_per_year)?;
			let delta = Self::last_updated().ensure_sub(when)?;

			let rate_adjustment = checked_pow(rate.interest_rate_per_sec, delta.ensure_into()?)
				.ok_or(ArithmeticError::Overflow)?;
			Ok(rate.accumulated_rate.ensure_div(rate_adjustment)?)
		}

		fn last_updated() -> Self::Moment {
			LastUpdated::<T>::get()
		}

		fn validate(interest_rate_per_year: T::YearRate) -> DispatchResult {
			let four_decimals = T::YearRate::saturating_from_integer(10000);
			let maximum = T::YearRate::saturating_from_integer(MAX_INTEREST_RATE);
			ensure!(
				interest_rate_per_year <= maximum
					&& interest_rate_per_year >= Zero::zero()
					&& (interest_rate_per_year.saturating_mul(four_decimals)).frac()
						== Zero::zero(),
				Error::<T>::InvalidRate
			);
			Ok(())
		}

		fn reference(interest_rate_per_year: T::YearRate) -> DispatchResult {
			let interest_rate_per_sec = Self::rate_conversion(interest_rate_per_year)?;
			Rates::<T>::try_mutate(|rates| {
				let rate = rates
					.iter_mut()
					.find(|rate| rate.interest_rate_per_sec == interest_rate_per_sec);

				match rate {
					Some(rate) => Ok(rate.reference_count.ensure_add_assign(1)?),
					None => {
						Self::validate(interest_rate_per_year)?;

						let new_rate = RateDetailsOf::<T> {
							interest_rate_per_sec,
							accumulated_rate: One::one(),
							reference_count: 1,
						};

						rates
							.try_push(new_rate)
							.map_err(|_| Error::<T>::TooManyRates)?;

						Ok(())
					}
				}
			})
		}

		fn unreference(interest_rate_per_year: T::YearRate) -> DispatchResult {
			let interest_rate_per_sec = Self::rate_conversion(interest_rate_per_year)?;
			Rates::<T>::try_mutate(|rates| {
				let idx = rates
					.iter()
					.enumerate()
					.find(|(_, rate)| rate.interest_rate_per_sec == interest_rate_per_sec)
					.ok_or(Error::<T>::NoSuchRate)?
					.0;

				rates[idx].reference_count = rates[idx].reference_count.saturating_sub(1);
				if rates[idx].reference_count == 0 {
					rates.swap_remove(idx);
				}
				Ok(())
			})
		}

		fn cache() -> Self::Cache {
			RateVec(Rates::<T>::get())
		}
	}

	pub struct RateVec<T: Config>(BoundedVec<RateDetailsOf<T>, T::MaxRateCount>);

	impl<T: Config> RateCache<T::YearRate, T::SecRate> for RateVec<T> {
		fn accrual(
			&self,
			interest_rate_per_year: T::YearRate,
		) -> Result<T::SecRate, DispatchError> {
			let interest_rate_per_sec = Pallet::<T>::rate_conversion(interest_rate_per_year)?;
			self.0
				.iter()
				.find(|rate| rate.interest_rate_per_sec == interest_rate_per_sec)
				.map(|rate| rate.accumulated_rate)
				.ok_or(Error::<T>::NoSuchRate.into())
		}
	}
}
