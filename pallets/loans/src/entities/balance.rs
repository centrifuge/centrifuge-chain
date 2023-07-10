// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_types::fixed_point::Rate;
use sp_arithmetic::{
	traits::{BaseArithmetic, Zero},
	FixedPointNumber, FixedPointOperand,
};
use sp_runtime::traits::{checked_pow, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub};
use sp_std::{
	cmp::Ordering,
	ops::{Add, Div, Mul, Sub},
};

#[derive(Copy, Clone)]
struct Balance<T> {
	/// Decimals in base10 system
	///
	/// E.g. 1_000 for a 3-decimal system.
	decimals: T,
	amount: T,
}

// impl<T: FixedPointOperand + BaseArithmetic> FixedPointOperand for Balance<T>
// {}

fn base<T: BaseArithmetic>(decimals: T) -> T {
	let base = match decimals {
		_ if decimals == T::zero() => T::one(),
		_ if decimals == T::one() => T::one() * 10u8.into(),
		_ if decimals == T::one() * 2u8.into() => T::one() * 100u8.into(),
		// use u8
		_ if decimals == T::one() * 3u8.into() => {
			T::one() * 1000u16.try_into().unwrap_or(100.into())
		}
		_ if decimals == T::one() * 4u8.into() => {
			T::one() * 10_000u16.try_into().unwrap_or(1_000.into())
		}
		_ if decimals == T::one() * 5u8.into() => {
			T::one() * 100_000u16.try_into().unwrap_or(10_000.into())
		}
		// use u32
		_ if decimals == T::one() * 6u8.into() => {
			T::one() * 1_000_000u32.try_into().unwrap_or(100_000.into())
		}
		_ if decimals == T::one() * 7u8.into() => {
			T::one() * 10_000_000u32.try_into().unwrap_or(1_000_000.into())
		}
		_ if decimals == T::one() * 8u8.into() => {
			T::one() * 100_000_000u32.try_into().unwrap_or(1_000_000.into())
		}
		_ if decimals == 9 => 1000000000,
		// use u64
		_ if decimals == 10 => 10000000000,
		_ if decimals == 11 => 100000000000,
		_ if decimals == 12 => 1000000000000,
		_ if decimals == 13 => 10000000000000,
		_ if decimals == 14 => 100000000000000,
		_ if decimals == 15 => 1000000000000000,
		_ if decimals == 16 => 10000000000000000,
		_ if decimals == 17 => 100000000000000000,
		_ if decimals == 18 => 1000000000000000000,
		_ if decimals == 19 => 10000000000000000000,
		_ if decimals == 20 => 100000000000000000000,
		// use u128
		_ if decimals == 21 => 1000000000000000000000,
		_ if decimals == 22 => 10000000000000000000000,
		_ if decimals == 23 => 100000000000000000000000,
		_ if decimals == 24 => 1000000000000000000000000,
		_ if decimals == 25 => 10000000000000000000000000,
		_ if decimals == 26 => 100000000000000000000000000,
		_ if decimals == 27 => 1000000000000000000000000000,
		_ if decimals == 28 => 10000000000000000000000000000,
		_ if decimals == 29 => 100000000000000000000000000000,
		_ if decimals == 30 => 1000000000000000000000000000000,
		_ if decimals == 31 => 10000000000000000000000000000000,
		_ if decimals == 32 => 100000000000000000000000000000000,
		_ if decimals == 33 => 1000000000000000000000000000000000,
		_ if decimals == 34 => 10000000000000000000000000000000000,
		_ if decimals == 35 => 100000000000000000000000000000000000,
		_ if decimals == 36 => 1000000000000000000000000000000000000,
		_ => 10000000000000000000000000000000000000,
		// The highest number fitting into u128
		//   40282366920938463463374607431768211455
	};

	base.into()
}

impl<T: FixedPointOperand + BaseArithmetic> Balance<T> {
	pub fn new(decimals: T) -> Self {
		Balance {
			decimals: T::one() * base(decimals),
			amount: Zero::zero(),
		}
	}

	fn unchecked_adjust_other(&self, other: Balance<T>) -> Balance<T> {
		let adjusted_amount: T = match self.decimals.cmp(&other.decimals) {
			Ordering::Less => {
				let div = other.decimals - self.decimals;

				// Safety: div Can never be zero
				Rate::from_inner(other.amount.unique_saturated_into()).saturating_div_int(div)
			}
			Ordering::Equal => other.amount,
			Ordering::Greater => {
				let mul = self.decimals - other.decimals;

				Rate::from_inner(other.amount.unique_saturated_into()).saturating_mul_int(mul)
			}
		};

		Balance {
			decimals: self.decimals,
			amount: adjusted_amount,
		}
	}

	fn checked_adjust_other(&self, other: Balance<T>) -> Option<Balance<T>> {
		let adjusted_amount: T = match self.decimals.cmp(&other.decimals) {
			Ordering::Less => {
				let div = other.decimals - self.decimals;

				// Safety: div Can never be zero
				Rate::from_inner(other.amount.try_into().ok()?).checked_div_int(div)?
			}
			Ordering::Equal => other.amount,
			Ordering::Greater => {
				let mul = self.decimals - other.decimals;

				Rate::from_inner(other.amount.try_into().ok()?).checked_mul_int(mul)?
			}
		};

		Some(Balance {
			decimals: self.decimals,
			amount: adjusted_amount,
		})
	}
}

macro_rules! math_impl_Self {
	($trait_name:ident, $method:ident) => {
		impl<T: FixedPointOperand + BaseArithmetic> $trait_name<Self> for Balance<T> {
			type Output = Self;

			fn $method(self, rhs: Self) -> Balance<T> {
				Balance {
					decimals: self.decimals,
					amount: self.amount.$method(self.unchecked_adjust_other(rhs).amount),
				}
			}
		}
	};
}

macro_rules! math_impl_T {
	($trait_name:ident, $method:ident) => {
		impl<T: FixedPointOperand + BaseArithmetic> $trait_name<T> for Balance<T> {
			type Output = Self;

			fn $method(self, rhs: T) -> Balance<T> {
				Balance {
					decimals: self.decimals,
					amount: self.amount.$method(rhs),
				}
			}
		}
	};
}

math_impl_Self!(Mul, mul);
math_impl_T!(Mul, mul);
math_impl_Self!(Div, div);
math_impl_T!(Div, div);
math_impl_Self!(Add, add);
math_impl_T!(Add, add);
math_impl_Self!(Sub, sub);
math_impl_T!(Sub, sub);

macro_rules! checked_impl {
	($trait_name:ident, $method:ident) => {
		impl<T: FixedPointOperand + BaseArithmetic> $trait_name for Balance<T> {
			#[inline]
			fn $method(&self, v: &Balance<T>) -> Option<Balance<T>> {
				Some(Balance {
					decimals: self.decimals,
					amount: self
						.amount
						.$method(&self.checked_adjust_other(*v)?.amount)?,
				})
			}
		}
	};
}

checked_impl!(CheckedMul, checked_mul);
checked_impl!(CheckedDiv, checked_div);
checked_impl!(CheckedAdd, checked_add);
checked_impl!(CheckedSub, checked_sub);

/// A Natural Number
struct Natural<T>(T);

impl<T, N> TryFrom<Balance<T>> for Natural<N> {
	type Error = ();

	fn try_from(value: Balance<T>) -> Result<Self, Self::Error> {
		todo!()
	}
}
