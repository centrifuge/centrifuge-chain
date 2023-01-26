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

#![cfg_attr(not(feature = "std"), no_std)]
use cfg_primitives::{Moment, SECONDS_PER_YEAR};
use cfg_traits::{InterestAccrual, RateCollection};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::UnixTime, BoundedVec, RuntimeDebug};
use scale_info::TypeInfo;
use sp_arithmetic::traits::{checked_pow, One, Zero};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedDiv, CheckedSub, Saturating},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand,
};
use sp_std::vec::Vec;

pub mod migrations;
pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod test_utils;

pub use pallet::*;

// Type aliases
type RateDetailsOf<T> = RateDetails<<T as Config>::InterestRate>;

// Storage types
#[derive(Encode, Decode, Default, Clone, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RateDetailsV1<InterestRate> {
	pub accumulated_rate: InterestRate,
	pub reference_count: u32,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RateDetails<InterestRate> {
	pub interest_rate_per_sec: InterestRate,
	pub accumulated_rate: InterestRate,
	pub reference_count: u32,
}

#[derive(Encode, Decode, TypeInfo, PartialEq, MaxEncodedLen, RuntimeDebug)]
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

		/// A fixed-point number which represents
		/// an interest rate.
		type InterestRate: Member
			+ Parameter
			+ Default
			+ core::fmt::Debug
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>
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
		/// Emits when the debt calculation failed
		DebtCalculationFailed,
		/// Emits when the debt adjustment failed
		DebtAdjustmentFailed,
		/// Emits when the interest rate was not used
		NoSuchRate,
		/// Emits when a historic rate was asked for from the future
		NotInPast,
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
			StorageVersion::<T>::put(Release::V1);
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_: T::BlockNumber) -> Weight {
			let then = LastUpdated::<T>::get();
			let now = Self::now();
			LastUpdated::<T>::set(Self::now());
			let delta = Self::now() - then;
			let bits = Moment::BITS - delta.leading_zeros();

			// reads: timestamp, last updated, rates vec
			// writes: last updated, rates vec
			let mut weight = T::DbWeight::get().reads_writes(3, 2);

			let rates = Rates::<T>::get();
			let rates: Vec<_> = rates
				.into_iter()
				.filter_map(|rate| {
					weight = weight.saturating_add(T::Weights::calculate_accumulated_rate(bits));

					let RateDetailsOf::<T> {
						interest_rate_per_sec,
						accumulated_rate,
						reference_count,
					} = rate;

					Self::calculate_accumulated_rate(
						interest_rate_per_sec,
						accumulated_rate,
						then,
						now,
					)
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
		pub fn get_current_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::Balance,
		) -> Result<T::Balance, DispatchError> {
			let rate = Self::get_rate(interest_rate_per_sec)?;
			let debt = Self::calculate_debt(normalized_debt, rate.accumulated_rate)
				.ok_or(Error::<T>::DebtCalculationFailed)?;
			Ok(debt)
		}

		pub fn get_previous_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::Balance,
			when: Moment,
		) -> Result<T::Balance, DispatchError> {
			let rate = Self::get_rate(interest_rate_per_sec)?;
			let now = LastUpdated::<T>::get();
			if when > now {
				return Err(Error::<T>::NotInPast.into());
			}
			let delta = now - when;
			let rate_adjustment = checked_pow(interest_rate_per_sec, delta as usize)
				.ok_or(ArithmeticError::Overflow)?;
			let past_rate = rate
				.accumulated_rate
				.checked_div(&rate_adjustment)
				.ok_or(ArithmeticError::Underflow)?;
			let debt = Self::calculate_debt(normalized_debt, past_rate)
				.ok_or(Error::<T>::DebtCalculationFailed)?;
			Ok(debt)
		}

		pub fn do_adjust_normalized_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::Balance,
			adjustment: Adjustment<T::Balance>,
		) -> Result<T::Balance, DispatchError> {
			let rate = Self::get_rate(interest_rate_per_sec)?;

			let debt = Self::calculate_debt(normalized_debt, rate.accumulated_rate)
				.ok_or(Error::<T>::DebtCalculationFailed)?;

			let new_debt = match adjustment {
				Adjustment::Increase(amount) => debt.checked_add(&amount),
				Adjustment::Decrease(amount) => debt.checked_sub(&amount),
			}
			.ok_or(Error::<T>::DebtAdjustmentFailed)?;

			let new_normalized_debt = rate
				.accumulated_rate
				.reciprocal()
				.and_then(|inv_rate| inv_rate.checked_mul_int(new_debt))
				.ok_or(Error::<T>::DebtAdjustmentFailed)?;

			Ok(new_normalized_debt)
		}

		pub fn do_renormalize_debt(
			old_interest_rate: T::InterestRate,
			new_interest_rate: T::InterestRate,
			normalized_debt: T::Balance,
		) -> Result<T::Balance, DispatchError> {
			let old_rate = Self::get_rate(old_interest_rate)?;
			let new_rate = Self::get_rate(new_interest_rate)?;

			let debt = Self::calculate_debt(normalized_debt, old_rate.accumulated_rate)
				.ok_or(Error::<T>::DebtCalculationFailed)?;
			let new_normalized_debt = new_rate
				.accumulated_rate
				.reciprocal()
				.and_then(|inv_rate| inv_rate.checked_mul_int(debt))
				.ok_or(Error::<T>::DebtAdjustmentFailed)?;

			Ok(new_normalized_debt)
		}

		/// Calculates the debt using debt = normalized_debt * accumulated_rate
		pub(crate) fn calculate_debt(
			normalized_debt: T::Balance,
			accumulated_rate: T::InterestRate,
		) -> Option<T::Balance> {
			accumulated_rate.checked_mul_int(normalized_debt)
		}

		pub fn calculate_accumulated_rate<Rate: FixedPointNumber>(
			interest_rate_per_sec: Rate,
			accumulated_rate: Rate,
			last_updated: Moment,
			now: Moment,
		) -> Result<Rate, DispatchError> {
			// accumulated_rate * interest_rate_per_sec ^ (now - last_updated)
			let time_difference_secs = now
				.checked_sub(last_updated)
				.ok_or(ArithmeticError::Underflow)?;

			Ok(
				checked_pow(interest_rate_per_sec, time_difference_secs as usize)
					.and_then(|new_rate| new_rate.checked_mul(&accumulated_rate))
					.ok_or(ArithmeticError::Overflow)?,
			)
		}

		pub fn now() -> Moment {
			T::Time::now().as_secs()
		}

		pub fn reference_yearly_interest_rate(
			interest_rate_per_year: T::InterestRate,
		) -> Result<T::InterestRate, DispatchError> {
			let interest_rate_per_sec = interest_rate_per_year
				.checked_div(&T::InterestRate::saturating_from_integer(SECONDS_PER_YEAR))
				.ok_or(ArithmeticError::Underflow)?
				.checked_add(&One::one())
				.ok_or(ArithmeticError::Overflow)?;

			let rate = Rates::<T>::get()
				.into_iter()
				.find(|rate| rate.interest_rate_per_sec == interest_rate_per_sec);

			if rate.is_none() {
				Self::validate_rate(interest_rate_per_year)?;
			}

			Self::reference_interest_rate(interest_rate_per_sec)?;
			Ok(interest_rate_per_sec)
		}

		pub fn reference_interest_rate(interest_rate_per_sec: T::InterestRate) -> DispatchResult {
			Rates::<T>::try_mutate(|rates| {
				for rate in rates.iter_mut() {
					if rate.interest_rate_per_sec == interest_rate_per_sec {
						rate.reference_count += 1;
						return Ok(());
					}
				}
				// Fell through the loop, so push in a new item
				let new_rate = RateDetailsOf::<T> {
					interest_rate_per_sec,
					accumulated_rate: One::one(),
					reference_count: 1,
				};
				rates
					.try_push(new_rate)
					.map_err(|_| Error::<T>::TooManyRates)?;
				Ok(())
			})
		}

		pub fn unreference_interest_rate(interest_rate_per_sec: T::InterestRate) -> DispatchResult {
			Rates::<T>::try_mutate(|rates| {
				let idx = rates
					.iter()
					.enumerate()
					.find(|(_, rate)| rate.interest_rate_per_sec == interest_rate_per_sec)
					.ok_or(Error::<T>::NoSuchRate)?
					.0;
				rates[idx].reference_count = rates[idx].reference_count.saturating_sub(1);
				if rates[idx].reference_count == 0 {
					rates.remove(idx);
				}
				Ok(())
			})
		}

		pub fn get_rate(
			interest_rate_per_sec: T::InterestRate,
		) -> Result<RateDetailsOf<T>, DispatchError> {
			let rate = Rates::<T>::get()
				.into_iter()
				.find(|rate| rate.interest_rate_per_sec == interest_rate_per_sec)
				.ok_or(Error::<T>::NoSuchRate)?;
			Ok(rate)
		}

		pub(crate) fn validate_rate(interest_rate_per_year: T::InterestRate) -> DispatchResult {
			let four_decimals = T::InterestRate::saturating_from_integer(10000);
			ensure!(
				interest_rate_per_year < One::one()
					&& interest_rate_per_year > Zero::zero()
					&& (interest_rate_per_year.saturating_mul(four_decimals)).frac()
						== Zero::zero(),
				Error::<T>::InvalidRate
			);
			Ok(())
		}
	}
}

impl<T: Config> InterestAccrual<T::InterestRate, T::Balance, Adjustment<T::Balance>> for Pallet<T> {
	type MaxRateCount = T::MaxRateCount;
	type NormalizedDebt = T::Balance;
	type Rates = RateVec<T>;

	fn current_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<T::Balance, DispatchError> {
		Pallet::<T>::get_current_debt(interest_rate_per_sec, normalized_debt)
	}

	fn previous_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
		when: Moment,
	) -> Result<T::Balance, DispatchError> {
		Pallet::<T>::get_previous_debt(interest_rate_per_sec, normalized_debt, when)
	}

	fn adjust_normalized_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
		adjustment: Adjustment<T::Balance>,
	) -> Result<Self::NormalizedDebt, DispatchError> {
		Pallet::<T>::do_adjust_normalized_debt(interest_rate_per_sec, normalized_debt, adjustment)
	}

	fn renormalize_debt(
		old_interest_rate: T::InterestRate,
		new_interest_rate: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<Self::NormalizedDebt, DispatchError> {
		Pallet::<T>::do_renormalize_debt(old_interest_rate, new_interest_rate, normalized_debt)
	}

	fn reference_yearly_rate(
		interest_rate_per_year: T::InterestRate,
	) -> Result<T::InterestRate, DispatchError> {
		Pallet::<T>::reference_yearly_interest_rate(interest_rate_per_year)
	}

	fn reference_rate(interest_rate_per_sec: T::InterestRate) -> Result<(), DispatchError> {
		Pallet::<T>::reference_interest_rate(interest_rate_per_sec)
	}

	fn unreference_rate(interest_rate_per_sec: T::InterestRate) -> Result<(), DispatchError> {
		Pallet::<T>::unreference_interest_rate(interest_rate_per_sec)
	}

	fn convert_additive_rate_to_per_sec(
		interest_rate_per_year: T::InterestRate,
	) -> Result<T::InterestRate, DispatchError> {
		Pallet::<T>::validate_rate(interest_rate_per_year)?;
		let interest_rate_per_sec = interest_rate_per_year
			.checked_div(&T::InterestRate::saturating_from_integer(SECONDS_PER_YEAR))
			.ok_or(ArithmeticError::Underflow)?;
		Ok(interest_rate_per_sec)
	}

	fn rates() -> Self::Rates {
		RateVec(Rates::<T>::get())
	}
}

pub struct RateVec<T: Config>(BoundedVec<RateDetailsOf<T>, T::MaxRateCount>);

impl<T: Config> RateCollection<T::InterestRate, T::Balance, T::Balance> for RateVec<T> {
	fn current_debt(
		&self,
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		self.0
			.iter()
			.find(|rate| rate.interest_rate_per_sec == interest_rate_per_sec)
			.ok_or(Error::<T>::NoSuchRate)
			.and_then(|rate| {
				Pallet::<T>::calculate_debt(normalized_debt, rate.accumulated_rate)
					.ok_or(Error::<T>::DebtCalculationFailed)
			})
			.map_err(Into::into)
	}
}
