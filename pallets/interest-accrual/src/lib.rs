// Copyright 2022 Centrifuge Foundation (centrifuge.io).
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
use codec::{Decode, Encode};
use common_traits::InterestAccrual;
use common_types::{Adjustment, Moment};
use frame_support::traits::UnixTime;
use scale_info::TypeInfo;
use sp_arithmetic::traits::{checked_pow, One};
use sp_runtime::ArithmeticError;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedDiv, CheckedSub},
	DispatchError, FixedPointNumber, FixedPointOperand,
};

pub mod migration;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;

pub use pallet::*;

// Type aliases
type RateDetailsOf<T> = RateDetails<<T as Config>::InterestRate>;
type RateDetailsV0Of<T> = RateDetailsV0<<T as Config>::InterestRate, Moment>;

// Storage types
#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RateDetailsV0<InterestRate, Moment> {
	pub accumulated_rate: InterestRate,
	pub last_updated: Moment,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RateDetails<InterestRate> {
	pub accumulated_rate: InterestRate,
	pub reference_count: u32,
}

#[derive(Encode, Decode, TypeInfo, PartialEq)]
#[repr(u32)]
pub enum Release {
	V0,
	V1,
}

impl Default for Release {
	fn default() -> Self {
		Self::V0
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>;

		type Time: UnixTime;

		type Weights: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn get_rate)]
	pub(super) type Rate<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InterestRate, RateDetailsOf<T>, OptionQuery>;

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
			let mut count = 0;
			let then = LastUpdated::<T>::get();
			let now = Self::now();
			LastUpdated::<T>::set(Self::now());
			let delta = Self::now() - then;
			let bits = Moment::BITS - delta.leading_zeros();
			Rate::<T>::translate(|per_sec, mut rate: RateDetailsOf<T>| {
				count += 1;
				Self::calculate_accumulated_rate(per_sec, rate.accumulated_rate, then, now)
					.ok()
					.map(|new_rate| {
						rate.accumulated_rate = new_rate;
						rate
					})
			});
			T::DbWeight::get().reads_writes(2, 1)
				+ count
					* (T::DbWeight::get().reads_writes(1, 1)
						+ T::Weights::calculate_accumulated_rate(bits))
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn get_current_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::Balance,
		) -> Result<T::Balance, DispatchError> {
			Rate::<T>::get(interest_rate_per_sec)
				.ok_or(Error::<T>::NoSuchRate.into())
				.and_then(|rate| {
					Self::calculate_debt(normalized_debt, rate.accumulated_rate)
						.ok_or(Error::<T>::DebtCalculationFailed.into())
				})
		}

		pub fn get_previous_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::Balance,
			when: Moment,
		) -> Result<T::Balance, DispatchError> {
			if let Some(rate) = Rate::<T>::get(interest_rate_per_sec) {
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
				Self::calculate_debt(normalized_debt, past_rate)
					.ok_or(Error::<T>::DebtCalculationFailed.into())
			} else {
				Err(Error::<T>::NoSuchRate.into())
			}
		}

		pub fn do_adjust_normalized_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::Balance,
			adjustment: Adjustment<T::Balance>,
		) -> Result<T::Balance, DispatchError> {
			let rate =
				Rate::<T>::try_get(interest_rate_per_sec).map_err(|_| Error::<T>::NoSuchRate)?;

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
			let old_rate =
				Rate::<T>::try_get(old_interest_rate).map_err(|_| Error::<T>::NoSuchRate)?;
			let new_rate =
				Rate::<T>::try_get(new_interest_rate).map_err(|_| Error::<T>::NoSuchRate)?;

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
		fn calculate_debt(
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

		pub fn reference_interest_rate(interest_rate_per_sec: T::InterestRate) {
			Rate::<T>::mutate(interest_rate_per_sec, |rate| {
				if let Some(rate) = rate {
					rate.reference_count += 1;
				} else {
					*rate = Some(RateDetailsOf::<T> {
						accumulated_rate: One::one(),
						reference_count: 1,
					});
				}
			})
		}

		pub fn unreference_interest_rate(interest_rate_per_sec: T::InterestRate) -> DispatchResult {
			Rate::<T>::try_mutate(interest_rate_per_sec, |maybe_rate| {
				if let Some(rate) = maybe_rate {
					rate.reference_count -= 1;
					if rate.reference_count == 0 {
						*maybe_rate = None;
					}
					Ok(())
				} else {
					Err(Error::<T>::NoSuchRate.into())
				}
			})
		}

		pub fn upgrade_to_v1() -> Weight {
			let mut weight = T::DbWeight::get().reads_writes(1, 1);
			let version = Pallet::<T>::storage_version();
			if version == Release::V0 {
				weight += migration::v1::migrate::<T>();
			}
			StorageVersion::<T>::set(Release::V1);
			weight
		}

		pub fn remove_unused_rates() -> Weight {
			let mut count = 0;
			Rate::<T>::translate(|_, rate: RateDetailsOf<T>| {
				count += 1;
				if rate.reference_count == 0 {
					None
				} else {
					Some(rate)
				}
			});
			T::DbWeight::get().reads_writes(count, count)
		}
	}
}

impl<T: Config> InterestAccrual<T::InterestRate, T::Balance, Adjustment<T::Balance>> for Pallet<T> {
	type NormalizedDebt = T::Balance;

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

	fn reference_rate(interest_rate_per_sec: T::InterestRate) {
		Pallet::<T>::reference_interest_rate(interest_rate_per_sec)
	}

	fn unreference_rate(interest_rate_per_sec: T::InterestRate) -> Result<(), DispatchError> {
		Pallet::<T>::unreference_interest_rate(interest_rate_per_sec)
	}
}
