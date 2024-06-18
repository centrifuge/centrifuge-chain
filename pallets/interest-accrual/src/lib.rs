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

#![cfg_attr(not(feature = "std"), no_std)]
use cfg_traits::{
	adjustments::Adjustment,
	interest::{
		FullPeriod, Interest, InterestAccrual, InterestAccrualProvider, InterestModel,
		InterestPayment, InterestRate,
	},
	time::{Period, Seconds, TimeAsSecs},
};
use frame_support::ensure;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::traits::{One, Zero};
use sp_runtime::{
	traits::{
		AtLeast32BitUnsigned, CheckedAdd, CheckedSub, EnsureAdd, EnsureAddAssign, EnsureDiv,
		EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub, EnsureSubAssign, Saturating,
	},
	DispatchError, DispatchResult, FixedPointNumber, FixedPointOperand,
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

pub struct ActiveInterestModel<T: Config> {
	model: InterestModel<T::Rate>,
	activation: Seconds,
	last_updated: Seconds,
	deactivation: Option<Seconds>,
	notional: T::Balance,
	interest: T::Balance,
}

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
		/// Emits when the accrual deactivation is in time before the last
		/// update
		AccrualDeactivationInThePast,
		/// Emits when T::Rate is not cnstructable from amount of passed periods
		/// (i.e. from the given u64)
		RateCreationFailed,
	}

	impl<T: Config> Pallet<T> {
		pub fn calculate_interest(
			notional: T::Balance,
			model: &InterestModel<T::Rate>,
			last_updated: Seconds,
			at: Seconds,
		) -> Result<Interest<T::Balance>, DispatchError> {
			let rate = model.rate_per_schedule()?;

			let interest = match model.compounding {
				None => Interest::only_full(FullPeriod::new(
					InterestPayment::new(last_updated, at, rate.ensure_mul_int(notional)?),
					1,
				)),
				Some(compounding) => {
					let periods = compounding.periods_passed(last_updated, at)?;
					Interest::new(
						periods.try_map_front(|p| {
							Ok::<_, DispatchError>(InterestPayment::new(
								p.from(),
								p.to(),
								p.part().mul_floor(rate.ensure_mul_int(notional)?),
							))
						})?,
						periods.try_map_full(|p| {
							Ok::<_, DispatchError>(FullPeriod::new(
								InterestPayment::new(
									p.from(),
									p.to(),
									T::Rate::checked_from_integer(p.passed())
										.ok_or(Error::<T>::RateCreationFailed)?
										.ensure_mul(rate)?
										.ensure_mul_int(notional)?,
								),
								p.passed(),
							))
						})?,
						periods.try_map_back(|p| {
							Ok::<_, DispatchError>(InterestPayment::new(
								p.from(),
								p.to(),
								p.part().mul_floor(rate.ensure_mul_int(notional)?),
							))
						})?,
					)
				}
			};

			Ok(interest)
		}
	}
}

impl<T: Config> InterestAccrualProvider<T::Rate, T::Balance> for Pallet<T> {
	type InterestAccrual = ActiveInterestModel<T>;

	fn reference(
		model: impl Into<InterestModel<T::Rate>>,
		at: Seconds,
	) -> Result<Self::InterestAccrual, DispatchError> {
		Ok(ActiveInterestModel {
			model: model.into(),
			activation: at,
			last_updated: at,
			deactivation: None,
			notional: T::Balance::zero(),
			interest: T::Balance::zero(),
		})
	}

	fn unreference(_model: Self::InterestAccrual) -> Result<(), DispatchError> {
		Ok(())
	}
}

impl<T: Config> InterestAccrual<T::Rate, T::Balance> for ActiveInterestModel<T> {
	fn update<F: FnOnce(Interest<T::Balance>) -> Result<(T::Balance, Seconds), DispatchError>>(
		&mut self,
		at: Seconds,
		result: F,
	) -> DispatchResult {
		let interest =
			Pallet::<T>::calculate_interest(self.notional, &self.model, self.last_updated, at)?;

		let (interest, last_updated) = result(interest)?;
		self.last_updated = last_updated;
		self.interest.ensure_add_assign(interest)?;

		Ok(())
	}

	fn notional(&self) -> Result<T::Balance, DispatchError> {
		Ok(self.notional)
	}

	fn interest(&self) -> Result<T::Balance, DispatchError> {
		Ok(self.interest)
	}

	fn debt(&self) -> Result<T::Balance, DispatchError> {
		self.notional.ensure_add(self.interest).map_err(Into::into)
	}

	fn adjust_notional(&mut self, adjustment: Adjustment<T::Balance>) -> Result<(), DispatchError> {
		match adjustment {
			Adjustment::Increase(amount) => {
				self.notional.ensure_add_assign(amount)?;
			}
			Adjustment::Decrease(amount) => {
				self.notional.ensure_sub_assign(amount)?;
			}
		}

		Ok(())
	}

	fn adjust_interest(&mut self, adjustment: Adjustment<T::Balance>) -> Result<(), DispatchError> {
		match adjustment {
			Adjustment::Increase(amount) => {
				self.interest.ensure_add_assign(amount)?;
			}
			Adjustment::Decrease(amount) => {
				self.interest.ensure_sub_assign(amount)?;
			}
		}

		Ok(())
	}

	fn adjust_rate(
		&mut self,
		adjustment: Adjustment<InterestRate<T::Rate>>,
	) -> Result<(), DispatchError> {
		todo!("Adjusting rate is not implemented yet")
	}

	fn adjust_compounding(&mut self, adjustment: Option<Period>) -> Result<(), DispatchError> {
		self.model.compounding = adjustment;

		Ok(())
	}

	fn stop_accrual_at(&mut self, deactivation: Seconds) -> Result<(), DispatchError> {
		ensure!(
			deactivation >= self.last_updated,
			Error::<T>::AccrualDeactivationInThePast
		);

		self.deactivation = Some(deactivation);

		Ok(())
	}

	fn last_updated(&self) -> Seconds {
		self.last_updated
	}

	fn accrued_since(&self) -> Seconds {
		self.activation
	}

	fn accruing_till(&self) -> Option<Seconds> {
		self.deactivation
	}
}
