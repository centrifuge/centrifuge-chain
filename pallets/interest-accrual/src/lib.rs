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
//!

#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use common_traits::InterestAccrual;
use common_types::{Adjustment, Moment};
use frame_support::traits::UnixTime;
use scale_info::TypeInfo;
use sp_arithmetic::traits::checked_pow;
use sp_runtime::ArithmeticError;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedMul, CheckedSub},
	DispatchError, FixedPointNumber, FixedPointOperand,
};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Type aliases
type RateDetailsOf<T> = RateDetails<<T as Config>::InterestRate, Moment>;

// Storage types
#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RateDetails<InterestRate, Moment> {
	pub accumulated_rate: InterestRate,
	pub last_updated: Moment,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
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

		/// A fixed-point number which represents
		/// the normalized debt.
		type NormalizedDebt: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber
			+ CheckedMul;

		/// The amount type
		type Amount: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ FixedPointNumber
			+ FixedPointOperand
			+ From<Self::NormalizedDebt>
			+ CheckedAdd
			+ CheckedSub
			+ TypeInfo;

		type Time: UnixTime;
	}

	#[pallet::storage]
	#[pallet::getter(fn get_sale)]
	pub(super) type Rate<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InterestRate, RateDetailsOf<T>, OptionQuery>;

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
	}

	// TODO: add permissionless extrinsic to update any rate

	impl<T: Config> Pallet<T> {
		pub fn get_current_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::NormalizedDebt,
		) -> Result<T::Amount, DispatchError> {
			Rate::<T>::try_mutate(
				interest_rate_per_sec,
				|rate_details| -> Result<T::Amount, DispatchError> {
					let rate = if let Some(rate) = rate_details {
						let new_accumulated_rate = Self::calculate_accumulated_rate(
							interest_rate_per_sec,
							rate.accumulated_rate,
							rate.last_updated,
						)
						.map_err(|_| Error::<T>::DebtCalculationFailed)?;

						rate.accumulated_rate = new_accumulated_rate;
						rate.last_updated = Self::now();

						rate
					} else {
						*rate_details = Some(RateDetails {
							accumulated_rate: T::InterestRate::saturating_from_rational(100, 100)
								.into(),
							last_updated: Self::now(),
						});
						rate_details.as_mut().expect("RateDetails now Some. qed.")
					};

					let debt = Self::calculate_debt(normalized_debt, rate.accumulated_rate)
						.ok_or(Error::<T>::DebtCalculationFailed)?;
					Ok(debt)
				},
			)
		}

		pub fn do_adjust_normalized_debt(
			interest_rate_per_sec: T::InterestRate,
			normalized_debt: T::NormalizedDebt,
			adjustment: Adjustment<T::Amount>,
		) -> Result<T::NormalizedDebt, DispatchError> {
			let rate =
				Rate::<T>::try_get(interest_rate_per_sec).map_err(|_| Error::<T>::NoSuchRate)?;

			let debt = Self::calculate_debt(normalized_debt, rate.accumulated_rate)
				.ok_or(Error::<T>::DebtCalculationFailed)?;

			let new_normalized_debt =
				Self::convert::<T::InterestRate, T::Amount>(rate.accumulated_rate)
					.and_then(|rate| {
						// Apply adjustment to debt
						match adjustment {
							Adjustment::Increase(amount) => debt.checked_add(&amount),
							Adjustment::Decrease(amount) => debt.checked_sub(&amount),
						}
						// Calculate normalized debt = debt / accumulated_rate
						.and_then(|debt| {
							debt.checked_div_int(rate).and_then(|normalized_debt| {
								Self::convert::<T::Amount, T::NormalizedDebt>(normalized_debt)
							})
						})
					})
					.ok_or(Error::<T>::DebtAdjustmentFailed)?;

			Ok(new_normalized_debt)
		}

		/// Calculates the debt using debt = normalized_debt * accumulated_rate
		fn calculate_debt(
			normalized_debt: T::NormalizedDebt,
			accumulated_rate: T::InterestRate,
		) -> Option<T::Amount> {
			// TODO: isn't there a better way of doing this, without the convert?
			Self::convert::<T::InterestRate, T::NormalizedDebt>(accumulated_rate).and_then(|rate| {
				normalized_debt
					.checked_mul(&rate)
					.and_then(|debt| Self::convert::<T::NormalizedDebt, T::Amount>(debt))
			})
		}

		fn calculate_accumulated_rate<Rate: FixedPointNumber>(
			interest_rate_per_sec: Rate,
			accumulated_rate: Rate,
			last_updated: Moment,
		) -> Result<Rate, DispatchError> {
			// accumulated_rate * interest_rate_per_sec ^ (now - last_updated)
			let time_difference_secs = Self::now()
				.checked_sub(last_updated)
				.ok_or(ArithmeticError::Underflow)?;

			checked_pow(interest_rate_per_sec, time_difference_secs as usize)
				.ok_or(ArithmeticError::Overflow)?
				.checked_mul(&accumulated_rate)
				.ok_or(ArithmeticError::Overflow.into())
		}

		/// converts a fixed point from A precision to B precision
		/// we don't convert from un-signed to signed or vice-verse
		fn convert<A: FixedPointNumber, B: FixedPointNumber>(a: A) -> Option<B> {
			if A::SIGNED != B::SIGNED {
				return None;
			}

			B::checked_from_rational(a.into_inner(), A::accuracy())
		}

		fn now() -> Moment {
			T::Time::now().as_secs()
		}
	}
}

impl<T: Config> InterestAccrual<T::InterestRate, T::Amount, Adjustment<T::Amount>> for Pallet<T> {
	type NormalizedDebt = T::NormalizedDebt;

	fn current_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<T::Amount, DispatchError> {
		Pallet::<T>::get_current_debt(interest_rate_per_sec, normalized_debt)
	}

	fn adjust_normalized_debt(
		interest_rate_per_sec: T::InterestRate,
		normalized_debt: Self::NormalizedDebt,
		adjustment: Adjustment<T::Amount>,
	) -> Result<Self::NormalizedDebt, DispatchError> {
		Pallet::<T>::do_adjust_normalized_debt(interest_rate_per_sec, normalized_debt, adjustment)
	}
}
