// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Decimal Fixed Point implementations for Substrate runtime.
//! Copied over from sp_arithmetic

use codec::{CompactAs, Decode, Encode, MaxEncodedLen};
#[cfg(feature = "std")]
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use sp_arithmetic::{
	helpers_128bit::multiply_by_rational_with_rounding,
	traits::{
		Bounded, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Saturating,
		UniqueSaturatedInto, Zero,
	},
	FixedPointNumber, FixedPointOperand, Rounding, SignedRounding,
};
use sp_std::{
	ops::{self},
	prelude::*,
};

/// Data type used as intermediate storage in some computations to avoid
/// overflow.
struct I129 {
	value: u128,
	negative: bool,
}

impl<N: FixedPointOperand> From<N> for I129 {
	fn from(n: N) -> I129 {
		if n < N::zero() {
			let value: u128 = n
				.checked_neg()
				.map(|n| n.unique_saturated_into())
				.unwrap_or_else(|| N::max_value().unique_saturated_into().saturating_add(1));
			I129 {
				value,
				negative: true,
			}
		} else {
			I129 {
				value: n.unique_saturated_into(),
				negative: false,
			}
		}
	}
}

/// Transforms an `I129` to `N` if it is possible.
fn from_i129<N: FixedPointOperand>(n: I129) -> Option<N> {
	let max_plus_one: u128 = N::max_value().unique_saturated_into().saturating_add(1);
	if n.negative && N::min_value() < N::zero() && n.value == max_plus_one {
		Some(N::min_value())
	} else {
		let unsigned_inner: N = n.value.try_into().ok()?;
		let inner = if n.negative {
			unsigned_inner.checked_neg()?
		} else {
			unsigned_inner
		};
		Some(inner)
	}
}

/// Returns `R::max` if the sign of `n * m` is positive, `R::min` otherwise.
fn to_bound<N: FixedPointOperand, D: FixedPointOperand, R: Bounded>(n: N, m: D) -> R {
	if (n < N::zero()) != (m < D::zero()) {
		R::min_value()
	} else {
		R::max_value()
	}
}

// Trait that allows us to specify rounding behaviour fixed point multiplication
pub trait FixedPointNumberExtension: FixedPointNumber {
	/// Checked multiplication by FixedPointOperand, with
	/// Rounding:SignedRounding rounding preference. Returns None if out of
	/// bounds.
	fn checked_mul_int_with_rounding<N: FixedPointOperand>(
		self,
		int: N,
		r: SignedRounding,
	) -> Option<N> {
		let lhs: I129 = self.into_inner().into();
		let rhs: I129 = int.into();
		let negative = lhs.negative != rhs.negative;

		multiply_by_rational_with_rounding(
			lhs.value,
			rhs.value,
			Self::DIV.unique_saturated_into(),
			Rounding::from_signed(r, negative),
		)
		.and_then(|value| from_i129(I129 { value, negative }))
	}

	/// Multiples by FixedPointOperand, with Rounding::SignedRounding rounding
	/// preference. Saturates if out of bounds.
	fn saturating_mul_int_with_rounding<N: FixedPointOperand>(
		self,
		int: N,
		r: SignedRounding,
	) -> N {
		self.checked_mul_int_with_rounding(int, r)
			.unwrap_or_else(|| to_bound(self.into_inner(), int))
	}

	/// Checked multiplication by FixedPointOperand; precision rounded to floor.
	/// Returns None if out of bounds.
	fn checked_mul_int_floor<N: FixedPointOperand>(self, int: N) -> Option<N> {
		self.checked_mul_int_with_rounding(int, SignedRounding::Minor)
	}

	/// Checked multiplication by FixedPointOperand; precision rounded to ceil.
	/// Returns None if out of bounds.
	fn checked_mul_int_ceil<N: FixedPointOperand>(self, int: N) -> Option<N> {
		self.checked_mul_int_with_rounding(int, SignedRounding::Major)
	}

	/// Checked multiplication by another val of Type Self, with
	/// Rounding::SignedRounding rounding preference. Returns None if out of
	/// bounds.
	fn checked_mul_with_rounding(&self, other: &Self, r: SignedRounding) -> Option<Self>;

	/// Checked multiplication by another val of Type Self; rounds precision to
	/// floor. Returns None if out of bounds.
	fn checked_mul_floor(&self, other: &Self) -> Option<Self> {
		self.checked_mul_with_rounding(other, SignedRounding::Minor)
	}

	/// Checked multiplication by another val of Type Self; rounds precision to
	/// ceil. Returns None if out of bounds.
	fn checked_mul_ceil(&self, other: &Self) -> Option<Self> {
		self.checked_mul_with_rounding(other, SignedRounding::Major)
	}

	/// Multiples by another val of type Self, with Rounding::SignedRounding
	/// rounding preference. Saturates if out of bounds.
	fn saturating_mul_with_rounding(self, other: Self, r: SignedRounding) -> Self;

	/// Multiples by another val of type Self; rounds precision to floor.
	/// Saturates if out of bounds.
	fn saturating_mul_floor(self, other: Self) -> Self {
		self.saturating_mul_with_rounding(other, SignedRounding::Minor)
	}

	/// Multiples by another val of type Self; rounds precision to ceil.
	/// Saturates if out of bounds.
	fn saturating_mul_ceil(self, other: Self) -> Self {
		self.saturating_mul_with_rounding(other, SignedRounding::Major)
	}

	/// Multiplies by FixedPointOperand with Rounding::SignedRounding rounding
	/// preference. Saturates if result out of bounds.
	// this should be superfluous though
	fn saturating_mul_int_floor<N: FixedPointOperand>(self, int: N) -> N {
		self.saturating_mul_int_with_rounding(int, SignedRounding::Minor)
	}

	/// Multiplies by FixedPointOperand; precision rounded to ceil
	/// Saturates if result out of bounds.
	fn saturating_mul_int_ceil<N: FixedPointOperand>(self, int: N) -> N {
		self.saturating_mul_int_with_rounding(int, SignedRounding::Major)
	}

	/// Creates Self from rational of FixedPointOperands, with
	/// Rounding::SignedRounding rounding preference Returns None if out of
	/// bounds
	fn checked_from_rational_with_rounding<N: FixedPointOperand, D: FixedPointOperand>(
		n: N,
		d: D,
		pref: SignedRounding,
	) -> Option<Self> {
		if d == D::zero() {
			return None;
		}

		let n: I129 = n.into();
		let d: I129 = d.into();
		let negative = n.negative != d.negative;

		multiply_by_rational_with_rounding(
			n.value,
			Self::DIV.unique_saturated_into(),
			d.value,
			Rounding::from_signed(pref, negative),
		)
		.and_then(|value| from_i129(I129 { value, negative }))
		.map(Self::from_inner)
	}

	/// Creates Self from rational of FixedPointOperands; rounds precision to
	/// ceil. Returns None if out of bounds.
	fn checked_from_rational_ceil<N: FixedPointOperand, D: FixedPointOperand>(
		n: N,
		d: D,
	) -> Option<Self> {
		Self::checked_from_rational_with_rounding(n, d, SignedRounding::Major)
	}

	/// Creates Self from rational of FixedPointOperands; rounds precision to
	/// floor. Returns None if out of bounds.
	fn checked_from_rational_floor<N: FixedPointOperand, D: FixedPointOperand>(
		n: N,
		d: D,
	) -> Option<Self> {
		Self::checked_from_rational_with_rounding(n, d, SignedRounding::Minor)
	}

	/// Creates Self from rational of FixedPointOperands, with
	/// Rounding::SignedRounding rounding preference. Panics if denominator 0
	/// is. Saturates if result out of bounds.
	fn saturating_from_rational_with_rounding<N: FixedPointOperand, D: FixedPointOperand>(
		n: N,
		d: D,
		r: SignedRounding,
	) -> Self {
		if d == D::zero() {
			panic!("attempted to create fixed point from rational with zero denominator")
		}
		Self::checked_from_rational_with_rounding(n, d, r).unwrap_or_else(|| to_bound(n, d))
	}

	/// Creates Self from rational of FixedPointOperands; rounds precision to
	/// floor. Panics if denominator 0 is.
	/// Saturates if result out of bounds.
	fn saturating_from_rational_floor<N: FixedPointOperand, D: FixedPointOperand>(
		n: N,
		d: D,
	) -> Self {
		Self::saturating_from_rational_with_rounding(n, d, SignedRounding::Minor)
	}

	/// Creates Self from rational of FixedPointOperands; rounds precision to
	/// ceil. Panics if denominator 0 is.
	/// Saturates if result out of bounds.
	fn saturating_from_rational_ceil<N: FixedPointOperand, D: FixedPointOperand>(
		n: N,
		d: D,
	) -> Self {
		Self::saturating_from_rational_with_rounding(n, d, SignedRounding::Major)
	}

	/// Checked division by another val of Type Self, with
	/// Rounding::SignedRounding rounding preference. Returns None if out of
	/// bounds.
	fn checked_div_with_rounding(&self, other: &Self, r: SignedRounding) -> Option<Self>;

	/// Checked division by another val of Type Self; rounds precision to floor.
	/// Returns None if out of bounds.
	fn checked_div_floor(&self, other: &Self) -> Option<Self> {
		self.checked_div_with_rounding(other, SignedRounding::Minor)
	}

	/// Checked division by another val of Type Self; rounds precision to ceil.
	/// Returns None if out of bounds.
	fn checked_div_ceil(&self, other: &Self) -> Option<Self> {
		self.checked_div_with_rounding(other, SignedRounding::Major)
	}

	/// Divides by another val of type Self, with Rounding::SignedRounding
	/// rounding preference. Saturates if out of bounds.
	fn saturating_div_with_rounding(&self, other: &Self, r: SignedRounding) -> Self;

	/// Divides by another val of type Self; rounds precision to floor.
	/// Saturates if out of bounds.
	fn saturating_div_floor(&self, other: &Self) -> Self {
		self.saturating_div_with_rounding(other, SignedRounding::Minor)
	}

	/// Divides by another val of type Self; rounds precision to ceil.
	/// Saturates if out of bounds.
	fn saturating_div_ceil(&self, other: &Self) -> Self {
		self.saturating_div_with_rounding(other, SignedRounding::Major)
	}

	/// Checked division by FixedPointOperand, with Rounding:SignedRounding
	/// rounding preference. Returns None if out of bounds.
	///
	/// Note:  This assumes that the FP accuracy has been adjusted to match
	/// the accuracy of the FP extended type in question (FixedU128 in this
	/// case). For example:
	///
	///    FixedU128::saturating_from_rational(2)
	///        .checked_div_with_rounding(2, SignedRounding::..)
	///
	/// would be equivalent to    
	///           
	///      (2 * FixedU128::accuracy) * (FixedU128::accuracy / 2)
	///
	/// instead of
	///
	///      2 * 1/2.
	///
	/// Whereas
	///
	///     FixedU128::saturating_from_rational(2)
	///         .checked_div_with_rounding(2 * FixedU128::accuracy)
	///
	/// would be equivalent to
	///
	///     2 * FixedU128::accuracy *
	///        (FixedU128::accuracy / 2 * FixedU128::accuracy)
	///
	/// Which would be 1 * FixedU128::accuracy
	fn checked_div_int_with_rounding<N: FixedPointOperand>(
		self,
		int: N,
		r: SignedRounding,
	) -> Option<N>;

	/// Checked division by FixedPointOperand; rounds precision to floor.
	/// Returns None if out of bounds.
	///
	/// Note:  This assumes that the FP accuracy has been adjusted to match
	/// the accuracy of the FP extended type in question (FixedU128 in this
	/// case).
	fn checked_div_int_floor<N: FixedPointOperand>(self, int: N) -> Option<N> {
		self.checked_div_int_with_rounding(int, SignedRounding::Minor)
	}

	/// Checked division by FixedPointOperand; rounds precision to ceil.
	/// Returns None if out of bounds.
	///
	/// Note:  This assumes that the FP accuracy has been adjusted to match
	/// the accuracy of the FP extended type in question (FixedU128 in this
	/// case).
	fn checked_div_int_ceil<N: FixedPointOperand>(self, int: N) -> Option<N> {
		self.checked_div_int_with_rounding(int, SignedRounding::Major)
	}

	/// Divides by FixedPointOperand, with Rounding:SignedRounding rounding
	/// preference. Panics if denominator 0 is.
	/// Saturates if result out of bounds.
	///
	/// Note:  This assumes that the FP accuracy has been adjusted to match
	/// the accuracy of the FP extended type in question (FixedU128 in this
	/// case).
	fn saturating_div_int_with_rounding<N: FixedPointOperand>(
		self,
		int: N,
		r: SignedRounding,
	) -> N {
		if int == N::zero() {
			panic!("attempt to divide by zero")
		}
		self.checked_div_int_with_rounding(int, r)
			.unwrap_or_else(|| to_bound(self.into_inner(), int))
	}

	/// Divides by FixedPointOperand; rounds precision to floor.
	/// Panics if denominator 0 is.
	/// Saturates if result out of bounds.
	///
	/// Note:  This assumes that the FP accuracy has been adjusted to match
	/// the accuracy of the FP extended type in question (FixedU128 in this
	/// case).
	fn saturating_div_int_floor<N: FixedPointOperand>(self, int: N) -> N {
		self.saturating_div_int_with_rounding(int, SignedRounding::Minor)
	}

	/// Divides by FixedPointOperand; rounds precision to ceil.
	/// Panics if denominator 0 is.
	/// Saturates if result out of bounds.
	///
	/// Note:  This assumes that the FP accuracy has been adjusted to match
	/// the accuracy of the FP extended type in question (FixedU128 in this
	/// case).
	fn saturating_div_int_ceil<N: FixedPointOperand>(self, int: N) -> N {
		self.saturating_div_int_with_rounding(int, SignedRounding::Major)
	}

	/// Returns the reciprocal -- 1 / self, with Rounding:SignedRounding
	/// rounding preference. Returns None if self is 0
	fn reciprocal_with_rounding(self, r: SignedRounding) -> Option<Self> {
		Self::one().checked_div_with_rounding(&self, r)
	}
	/// Returns reciprocal; rounds precision to floor.
	/// Returns None if self is 0
	fn reciprocal_floor(self) -> Option<Self> {
		self.reciprocal_with_rounding(SignedRounding::Minor)
	}

	/// Returns reciprocal; rounds precision to ceil.
	/// Returns None if self is 0
	fn reciprocal_ceil(self) -> Option<Self> {
		self.reciprocal_with_rounding(SignedRounding::Major)
	}

	/// Checked self raised to pow.
	/// Saturate if result out of bounds.
	fn saturating_pow_with_rounding(self, pow: usize, r: SignedRounding) -> Self {
		// Note:  this is using binary exponentiation
		// including explanatory comments here as the Substrate implementation
		// was initially unclear
		if pow == 0 {
			return Self::one();
		}
		let mut accum_a = Self::one();
		let mut accum_b = self;
		let exp = pow as u32;

		// the number of bytes the exponent uses -- also most significant bits
		// we'll use this later for the number of iterations, and the binary value
		// as what we'll ultimately end up doing is
		// self ** ( sum ( 2 ** i for msb(right to left) where i == 1 ))
		// with each iteration having its computation stored in accum_a if i == 1
		// allowing us to reuse prev calculated results and avoid extra computations
		let msb_pos = 32 - exp.leading_zeros();
		for i in 0..msb_pos {
			// if the result of 1 bitshifted i times and bitwise-and is greater than 0
			if ((1 << i) & exp) > 0 {
				accum_a = accum_a.saturating_mul_with_rounding(accum_b, r)
			}
			accum_b = accum_b.saturating_mul_with_rounding(accum_b, r)
		}
		accum_a
	}

	/// Checked self raised to pow; rounds precision to floor.
	/// Saturates if result out of bounds.
	fn saturating_pow_floor(self, pow: usize) -> Self {
		self.saturating_pow_with_rounding(pow, SignedRounding::Minor)
	}

	/// Checked self raised to pow; rounds precision to ceil.
	/// Saturates if result out of bounds.
	fn saturating_pow_ceil(self, pow: usize) -> Self {
		self.saturating_pow_with_rounding(pow, SignedRounding::Major)
	}
}

const DECIMALS_18: u128 = 1_000_000_000_000_000_000u128;
const DECIMALS_27: u128 = 1_000_000_000_000_000_000_000_000_000u128;

/// A fixed point number representation with 27 decimals. Used for
/// representing a rate, mostly interest rate.
#[doc = "_Fixed Point 128 bits unsigned with 27 decimals precision"]
pub type Rate = FixedU128<DECIMALS_27>;

/// A fixed point number representation with 18 decimals. Used for
/// representing a ratio between two things.
#[doc = "_Fixed Point 128 bits unsigned with 18 decimals precision"]
pub type Ratio = FixedU128<DECIMALS_18>;

/// A fixed point number representation with 18 decimals. Used for
/// representing a quantity of something.
#[doc = "_Fixed Point 128 bits unsigned with 18 decimals precision"]
pub type Quantity = FixedU128<DECIMALS_18>;

/// A fixed point number representation in the range.
#[doc = "_Fixed Point 128 bits unsigned with variable precision."]
#[derive(
	Encode,
	Decode,
	CompactAs,
	Default,
	Copy,
	Clone,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	scale_info::TypeInfo,
	MaxEncodedLen,
)]
pub struct FixedU128<const DIV: u128>(u128);

impl<const DIV: u128> From<u128> for FixedU128<DIV> {
	fn from(int: u128) -> Self {
		FixedU128::saturating_from_integer(int)
	}
}

impl<N: FixedPointOperand, D: FixedPointOperand, const DIV: u128> From<(N, D)> for FixedU128<DIV> {
	fn from(r: (N, D)) -> Self {
		FixedU128::saturating_from_rational(r.0, r.1)
	}
}

impl<const DIV: u128> FixedPointNumber for FixedU128<DIV> {
	type Inner = u128;

	const DIV: Self::Inner = DIV;
	const SIGNED: bool = false;

	fn from_inner(inner: Self::Inner) -> Self {
		Self(inner)
	}

	fn into_inner(self) -> Self::Inner {
		self.0
	}

	/// Creates `self` from a rational number. Equal to `n / d`.
	///
	/// Returns `None` if `d == 0` or `n / d` exceeds accuracy.
	fn checked_from_rational<N: FixedPointOperand, D: FixedPointOperand>(
		n: N,
		d: D,
	) -> Option<Self> {
		Self::checked_from_rational_with_rounding(n, d, SignedRounding::NearestPrefLow)
	}

	/// Checked multiplication for integer type `N`. Equal to `self * n`.
	///
	/// Returns `None` if the result does not fit in `N`.
	fn checked_mul_int<N: FixedPointOperand>(self, n: N) -> Option<N> {
		self.checked_mul_int_with_rounding(n, SignedRounding::NearestPrefLow)
	}
}

impl<const DIV: u128> FixedPointNumberExtension for FixedU128<DIV> {
	/// Checks multiplication of val with FixedPoint
	/// Returns None if out of bounds
	fn checked_mul_with_rounding(&self, other: &Self, r: SignedRounding) -> Option<Self> {
		let lhs: I129 = self.into_inner().into();
		let rhs: I129 = other.into_inner().into();
		let negative = lhs.negative != rhs.negative;

		multiply_by_rational_with_rounding(
			lhs.value,
			rhs.value,
			Self::DIV.unique_saturated_into(),
			Rounding::from_signed(r, negative),
		)
		.and_then(|value| from_i129(I129 { value, negative }))
		.map(Self)
	}

	fn checked_div_with_rounding(&self, other: &Self, r: SignedRounding) -> Option<Self> {
		if other.0 == 0 {
			return None;
		}

		let lhs: I129 = self.0.into();
		let rhs: I129 = other.0.into();
		let negative = lhs.negative != rhs.negative;

		multiply_by_rational_with_rounding(
			lhs.value,
			Self::DIV as u128,
			rhs.value,
			Rounding::from_signed(r, negative),
		)
		.and_then(|value| from_i129(I129 { value, negative }))
		.map(Self)
	}

	/// multiplies self by param and rounds precision with SignedRounding
	/// Saturates if result out of bounds
	fn saturating_mul_with_rounding(self, other: Self, r: SignedRounding) -> Self {
		self.checked_mul_with_rounding(&other, r)
			.unwrap_or_else(|| to_bound(self.0, other.0))
	}

	/// divides by param and takes rounding preference for accuracy
	/// Saturates result if out of bounds -- panics if 0 is denominator
	fn saturating_div_with_rounding(&self, other: &Self, r: SignedRounding) -> Self {
		if other.is_zero() {
			panic!("attempt to divide by zero")
		}
		self.checked_div_with_rounding(other, r)
			.unwrap_or_else(|| to_bound(self.0, other.0))
	}

	/// Checked division by FixedPointOperand, with Rounding:SignedRounding
	/// rounding preference. Returns None if out of bounds.
	///
	/// Note:  This assumes that the FP accuracy has been adjusted to match
	/// the accuracy of the FP extended type in question (FixedU128 in this
	/// case). For example:
	///
	///    FixedU128::saturating_from_rational(2)
	///        .checked_div_with_rounding(2, SignedRounding::..)
	///
	/// would be equivalent to    
	///           
	///      (2 * FixedU128::accuracy) * (FixedU128::accuracy / 2)
	///
	/// instead of
	///
	///      2 * 1/2.
	///
	/// Whereas
	///
	///     FixedU128::saturating_from_rational(2)
	///         .checked_div_with_rounding(2 * FixedU128::accuracy)
	///
	/// would be equivalent to
	///
	///     2 * FixedU128::accuracy *
	///        (FixedU128::accuracy / 2 * FixedU128::accuracy)
	///
	/// Which would be 1 * FixedU128::accuracy
	fn checked_div_int_with_rounding<N: FixedPointOperand>(
		self,
		int: N,
		r: SignedRounding,
	) -> Option<N> {
		let rhs: I129 = int.into();

		self.checked_div_with_rounding(&Self::from_inner(rhs.value), r)
			.and_then(|n| n.into_inner().into())
			.and_then(|n| N::try_from(n).ok())
	}
}

impl<const DIV: u128> FixedU128<DIV> {
	/// const version of `FixedPointNumber::from_inner`.
	pub const fn from_inner(inner: u128) -> Self {
		Self(inner)
	}
}

impl<const DIV: u128> Saturating for FixedU128<DIV> {
	fn saturating_add(self, rhs: Self) -> Self {
		Self(self.0.saturating_add(rhs.0))
	}

	fn saturating_sub(self, rhs: Self) -> Self {
		Self(self.0.saturating_sub(rhs.0))
	}

	fn saturating_mul(self, rhs: Self) -> Self {
		self.saturating_mul_with_rounding(rhs, SignedRounding::NearestPrefLow)
	}

	fn saturating_pow(self, exp: usize) -> Self {
		self.saturating_pow_with_rounding(exp, SignedRounding::NearestPrefLow)
	}
}

impl<const DIV: u128> ops::Neg for FixedU128<DIV> {
	type Output = Self;

	fn neg(self) -> Self::Output {
		Self(<Self as FixedPointNumber>::Inner::zero() - self.0)
	}
}

impl<const DIV: u128> ops::Add for FixedU128<DIV> {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		Self(self.0 + rhs.0)
	}
}

impl<const DIV: u128> ops::Sub for FixedU128<DIV> {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		Self(self.0 - rhs.0)
	}
}

impl<const DIV: u128> ops::Mul for FixedU128<DIV> {
	type Output = Self;

	fn mul(self, rhs: Self) -> Self::Output {
		self.checked_mul(&rhs)
			.unwrap_or_else(|| panic!("attempt to multiply with overflow"))
	}
}

impl<const DIV: u128> ops::Div for FixedU128<DIV> {
	type Output = Self;

	fn div(self, rhs: Self) -> Self::Output {
		if rhs.0 == 0 {
			panic!("attempt to divide by zero")
		}
		self.checked_div(&rhs)
			.unwrap_or_else(|| panic!("attempt to divide with overflow"))
	}
}

impl<const DIV: u128> CheckedSub for FixedU128<DIV> {
	fn checked_sub(&self, rhs: &Self) -> Option<Self> {
		self.0.checked_sub(rhs.0).map(Self)
	}
}

impl<const DIV: u128> CheckedAdd for FixedU128<DIV> {
	fn checked_add(&self, rhs: &Self) -> Option<Self> {
		self.0.checked_add(rhs.0).map(Self)
	}
}

impl<const DIV: u128> CheckedDiv for FixedU128<DIV> {
	fn checked_div(&self, other: &Self) -> Option<Self> {
		self.checked_div_with_rounding(other, SignedRounding::NearestPrefLow)
	}
}

impl<const DIV: u128> CheckedMul for FixedU128<DIV> {
	fn checked_mul(&self, other: &Self) -> Option<Self> {
		self.checked_mul_with_rounding(other, SignedRounding::NearestPrefLow)
	}
}

impl<const DIV: u128> Bounded for FixedU128<DIV> {
	fn min_value() -> Self {
		Self(<Self as FixedPointNumber>::Inner::min_value())
	}

	fn max_value() -> Self {
		Self(<Self as FixedPointNumber>::Inner::max_value())
	}
}

impl<const DIV: u128> Zero for FixedU128<DIV> {
	fn zero() -> Self {
		Self::from_inner(<Self as FixedPointNumber>::Inner::zero())
	}

	fn is_zero(&self) -> bool {
		self.into_inner() == <Self as FixedPointNumber>::Inner::zero()
	}
}

impl<const DIV: u128> One for FixedU128<DIV> {
	fn one() -> Self {
		Self::from_inner(Self::DIV)
	}
}

impl<const DIV: u128> sp_std::fmt::Debug for FixedU128<DIV> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		let integral = {
			let int = self.0 / Self::accuracy();
			let signum_for_zero = if int == 0 && self.is_negative() {
				"-"
			} else {
				""
			};
			format!("{signum_for_zero}{int}")
		};
		let precision = (Self::accuracy() as f64).log10() as usize;
		let fractional = format!(
			"{:0>weight$}",
			((self.0 % Self::accuracy()) as i128).abs(),
			weight = precision
		);
		write!(f, "{}({}.{})", stringify!(FixedU128), integral, fractional)
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

#[cfg(feature = "std")]
impl<const DIV: u128> sp_std::fmt::Display for FixedU128<DIV> {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

#[cfg(feature = "std")]
impl<const DIV: u128> sp_std::str::FromStr for FixedU128<DIV> {
	type Err = &'static str;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let inner: <Self as FixedPointNumber>::Inner = s
			.parse()
			.map_err(|_| "invalid string input for fixed point number")?;
		Ok(Self::from_inner(inner))
	}
}

// Manual impl `Serialize` as serde_json does not support i128.
// TODO: remove impl if issue https://github.com/serde-rs/json/issues/548 fixed.
#[cfg(feature = "std")]
impl<const DIV: u128> Serialize for FixedU128<DIV> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&self.to_string())
	}
}

// Manual impl `Deserialize` as serde_json does not support i128.
// TODO: remove impl if issue https://github.com/serde-rs/json/issues/548 fixed.
#[cfg(feature = "std")]
impl<'de, const DIV: u128> Deserialize<'de> for FixedU128<DIV> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		use sp_std::str::FromStr;
		let s = String::deserialize(deserializer)?;
		FixedU128::from_str(&s).map_err(de::Error::custom)
	}
}

#[cfg(test)]
mod test_fixed_u128 {
	use super::*;

	fn max() -> FixedU128<DECIMALS_27> {
		FixedU128::<DECIMALS_27>::max_value()
	}

	fn min() -> FixedU128<DECIMALS_27> {
		FixedU128::<DECIMALS_27>::min_value()
	}

	fn precision() -> usize {
		(FixedU128::<DECIMALS_27>::accuracy() as f64).log10() as usize
	}

	#[test]
	fn macro_preconditions() {
		assert!(FixedU128::<DECIMALS_27>::DIV > 0);
	}

	#[test]
	fn from_i129_works() {
		let a = I129 {
			value: 1,
			negative: true,
		};

		// Can't convert negative number to unsigned.
		assert_eq!(from_i129::<u128>(a), None);

		let a = I129 {
			value: u128::MAX - 1,
			negative: false,
		};

		// Max - 1 value fits.
		assert_eq!(from_i129::<u128>(a), Some(u128::MAX - 1));

		let a = I129 {
			value: u128::MAX,
			negative: false,
		};

		// Max value fits.
		assert_eq!(from_i129::<u128>(a), Some(u128::MAX));

		let a = I129 {
			value: i128::MAX as u128 + 1,
			negative: true,
		};

		// Min value fits.
		assert_eq!(from_i129::<i128>(a), Some(i128::MIN));

		let a = I129 {
			value: i128::MAX as u128 + 1,
			negative: false,
		};

		// Max + 1 does not fit.
		assert_eq!(from_i129::<i128>(a), None);

		let a = I129 {
			value: i128::MAX as u128,
			negative: false,
		};

		// Max value fits.
		assert_eq!(from_i129::<i128>(a), Some(i128::MAX));
	}

	#[test]
	fn to_bound_works() {
		let a = 1i32;
		let b = 1i32;

		// Pos + Pos => Max.
		assert_eq!(to_bound::<_, _, i32>(a, b), i32::MAX);

		let a = -1i32;
		let b = -1i32;

		// Neg + Neg => Max.
		assert_eq!(to_bound::<_, _, i32>(a, b), i32::MAX);

		let a = 1i32;
		let b = -1i32;

		// Pos + Neg => Min.
		assert_eq!(to_bound::<_, _, i32>(a, b), i32::MIN);

		let a = -1i32;
		let b = 1i32;

		// Neg + Pos => Min.
		assert_eq!(to_bound::<_, _, i32>(a, b), i32::MIN);

		let a = 1i32;
		let b = -1i32;

		// Pos + Neg => Min (unsigned).
		assert_eq!(to_bound::<_, _, u32>(a, b), 0);
	}

	#[test]
	fn op_neg_works() {
		let a = FixedU128::<DECIMALS_27>::zero();
		let b = -a;

		// Zero.
		assert_eq!(a, b);

		if FixedU128::<DECIMALS_27>::SIGNED {
			let a = FixedU128::<DECIMALS_27>::saturating_from_integer(5);
			let b = -a;

			// Positive.
			assert_eq!(FixedU128::<DECIMALS_27>::saturating_from_integer(-5), b);

			let a = FixedU128::<DECIMALS_27>::saturating_from_integer(-5);
			let b = -a;

			// Negative
			assert_eq!(FixedU128::<DECIMALS_27>::saturating_from_integer(5), b);

			let a = FixedU128::<DECIMALS_27>::max_value();
			let b = -a;

			// Max.
			assert_eq!(
				FixedU128::<DECIMALS_27>::min_value() + FixedU128::<DECIMALS_27>::from_inner(1),
				b
			);

			let a = FixedU128::<DECIMALS_27>::min_value() + FixedU128::<DECIMALS_27>::from_inner(1);
			let b = -a;

			// Min.
			assert_eq!(FixedU128::<DECIMALS_27>::max_value(), b);
		}
	}

	#[test]
	fn op_checked_add_overflow_works() {
		let a = FixedU128::<DECIMALS_27>::max_value();
		let b = 1.into();
		assert!(a.checked_add(&b).is_none());
	}

	#[test]
	fn op_add_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2);
		let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);

		// Positive case: 6/2 = 3.
		assert_eq!(FixedU128::<DECIMALS_27>::saturating_from_integer(3), a + b);

		if FixedU128::<DECIMALS_27>::SIGNED {
			// Negative case: 4/2 = 2.
			let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, -2);
			assert_eq!(FixedU128::<DECIMALS_27>::saturating_from_integer(2), a + b);
		}
	}

	#[test]
	fn op_checked_sub_underflow_works() {
		let a = FixedU128::<DECIMALS_27>::min_value();
		let b = 1.into();
		assert!(a.checked_sub(&b).is_none());
	}

	#[test]
	fn op_sub_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2);
		let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);

		assert_eq!(FixedU128::<DECIMALS_27>::saturating_from_integer(2), a - b);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(-2),
			b.saturating_sub(a)
		);
	}

	#[test]
	fn op_checked_mul_overflow_works() {
		let a = FixedU128::<DECIMALS_27>::max_value();
		let b = 2.into();
		assert!(a.checked_mul(&b).is_none());
	}

	#[test]
	fn op_mul_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(42);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(2);
		assert_eq!(FixedU128::<DECIMALS_27>::saturating_from_integer(84), a * b);

		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(42);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(-2);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(-84),
			a * b
		);
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn op_div_panics_on_zero_divisor() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(1);
		let b = 0.into();
		let _c = a / b;
	}

	#[test]
	fn op_checked_div_overflow_works() {
		if FixedU128::<DECIMALS_27>::SIGNED {
			let a = FixedU128::<DECIMALS_27>::min_value();
			let b =
				FixedU128::<DECIMALS_27>::zero().saturating_sub(FixedU128::<DECIMALS_27>::one());
			assert!(a.checked_div(&b).is_none());
		}
	}

	#[test]
	fn op_div_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(42);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(2);
		assert_eq!(FixedU128::<DECIMALS_27>::saturating_from_integer(21), a / b);

		if FixedU128::<DECIMALS_27>::SIGNED {
			let a = FixedU128::<DECIMALS_27>::saturating_from_integer(42);
			let b = FixedU128::<DECIMALS_27>::saturating_from_integer(-2);
			assert_eq!(
				FixedU128::<DECIMALS_27>::saturating_from_integer(-21),
				a / b
			);
		}
	}

	#[test]
	fn saturating_from_integer_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		// Cases where integer fits.
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(42);
		assert_eq!(a.into_inner(), 42 * accuracy);

		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(-42);
		assert_eq!(a.into_inner(), 0.saturating_sub(42 * accuracy));

		// Max/min integers that fit.
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(inner_max / accuracy);
		assert_eq!(a.into_inner(), (inner_max / accuracy) * accuracy);

		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(inner_min / accuracy);
		assert_eq!(a.into_inner(), (inner_min / accuracy) * accuracy);

		// Cases where integer doesn't fit, so it saturates.
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(inner_max / accuracy + 1);
		assert_eq!(a.into_inner(), inner_max);

		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(
			(inner_min / accuracy).saturating_sub(1),
		);
		assert_eq!(a.into_inner(), inner_min);
	}

	#[test]
	fn checked_from_integer_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		// Case where integer fits.
		let a = FixedU128::<DECIMALS_27>::checked_from_integer(42u128)
			.expect("42 * accuracy <= inner_max; qed");
		assert_eq!(a.into_inner(), 42 * accuracy);

		// Max integer that fit.
		let a = FixedU128::<DECIMALS_27>::checked_from_integer(inner_max / accuracy)
			.expect("(inner_max / accuracy) * accuracy <= inner_max; qed");
		assert_eq!(a.into_inner(), (inner_max / accuracy) * accuracy);

		// Case where integer doesn't fit, so it returns `None`.
		let a = FixedU128::<DECIMALS_27>::checked_from_integer(inner_max / accuracy + 1);
		assert_eq!(a, None);

		if FixedU128::<DECIMALS_27>::SIGNED {
			// Case where integer fits.
			let a = FixedU128::<DECIMALS_27>::checked_from_integer(0.saturating_sub(4u128))
				.expect("-42 * accuracy >= inner_min; qed");
			assert_eq!(a.into_inner(), 0 - 42 * accuracy);

			// Min integer that fit.
			let a = FixedU128::<DECIMALS_27>::checked_from_integer(inner_min / accuracy)
				.expect("(inner_min / accuracy) * accuracy <= inner_min; qed");
			assert_eq!(a.into_inner(), (inner_min / accuracy) * accuracy);

			// Case where integer doesn't fit, so it returns `None`.
			let a = FixedU128::<DECIMALS_27>::checked_from_integer(inner_min / accuracy - 1);
			assert_eq!(a, None);
		}
	}

	#[test]
	fn from_inner_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();

		assert_eq!(max(), FixedU128::<DECIMALS_27>::from_inner(inner_max));
		assert_eq!(min(), FixedU128::<DECIMALS_27>::from_inner(inner_min));
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn saturating_from_rational_panics_on_zero_divisor() {
		let _ = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 0);
	}

	#[test]
	fn saturating_from_rational_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2);

		// Positive case: 2.5
		assert_eq!(a.into_inner(), 25 * accuracy / 10);

		// Max - 1.
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max - 1, accuracy);
		assert_eq!(a.into_inner(), inner_max - 1);

		// Min + 1.
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min + 1, accuracy);
		assert_eq!(a.into_inner(), inner_min + 1);

		// Max.
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max, accuracy);
		assert_eq!(a.into_inner(), inner_max);

		// Min.
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min, accuracy);
		assert_eq!(a.into_inner(), inner_min);

		// Zero.
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(0, 1);
		assert_eq!(a.into_inner(), 0);

		if FixedU128::<DECIMALS_27>::SIGNED {
			// Negative case: -2.5
			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(-5, 2);
			assert_eq!(a.into_inner(), 0 - 25 * accuracy / 10);

			// Other negative case: -2.5
			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(5, -2);
			assert_eq!(a.into_inner(), 0 - 25 * accuracy / 10);

			// Other positive case: 2.5
			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(-5, -2);
			assert_eq!(a.into_inner(), 25 * accuracy / 10);

			// Max + 1, saturates.
			let a =
				FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max as u128 + 1, accuracy);
			assert_eq!(a.into_inner(), inner_max);

			// Min - 1, saturates.
			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
				inner_max as u128 + 2,
				0 - accuracy,
			);
			assert_eq!(a.into_inner(), inner_min);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max, 0 - accuracy);
			assert_eq!(a.into_inner(), 0 - inner_max);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min, 0 - accuracy);
			assert_eq!(a.into_inner(), inner_max);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min + 1, 0 - accuracy);
			assert_eq!(a.into_inner(), inner_max);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min, 0 - 1);
			assert_eq!(a.into_inner(), inner_max);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max, 0 - 1);
			assert_eq!(a.into_inner(), inner_min);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max, 0 - inner_max);
			assert_eq!(a.into_inner(), 0 - accuracy);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(0 - inner_max, inner_max);
			assert_eq!(a.into_inner(), 0 - accuracy);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max, 0 - 3 * accuracy);
			assert_eq!(a.into_inner(), 0 - inner_max / 3);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min, 0 - accuracy / 3);
			assert_eq!(a.into_inner(), inner_max);

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 0 - accuracy);
			assert_eq!(a.into_inner(), 0.saturating_sub(1));

			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min, inner_min);
			assert_eq!(a.into_inner(), accuracy);

			// Out of accuracy.
			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 0 - accuracy - 1);
			assert_eq!(a.into_inner(), 0);
		}

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max - 1, accuracy);
		assert_eq!(a.into_inner(), inner_max - 1);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min + 1, accuracy);
		assert_eq!(a.into_inner(), inner_min + 1);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max, 1);
		assert_eq!(a.into_inner(), inner_max);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min, 1);
		assert_eq!(a.into_inner(), inner_min);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max, inner_max);
		assert_eq!(a.into_inner(), accuracy);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_max, 3 * accuracy);
		assert_eq!(a.into_inner(), inner_max / 3);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min, 2 * accuracy);
		assert_eq!(a.into_inner(), inner_min / 2);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(inner_min, accuracy / 3);
		assert_eq!(a.into_inner(), inner_min);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, accuracy);
		assert_eq!(a.into_inner(), 1);

		// Round to zero if equidistant
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 2 * accuracy).unwrap();
		assert_eq!(a.into_inner(), 0);

		// Round to nearest if slightly of equidistant
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 2 * accuracy - 1).unwrap();
		assert_eq!(a.into_inner(), 1);

		// Round to nearest if slightly of equidistant
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 2 * accuracy + 1).unwrap();
		assert_eq!(a.into_inner(), 0);
	}

	#[test]
	#[should_panic(
		expected = "attempted to create fixed point from rational with zero denominator"
	)]
	fn saturating_from_rational_with_rounding_panics_on_zero_divisor() {
		let _ = FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
			1,
			0,
			SignedRounding::NearestPrefLow,
		);
	}

	#[test]
	fn saturating_from_rational_with_rounding_works() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				0,
				1,
				SignedRounding::NearestPrefLow
			),
			FixedU128::<DECIMALS_27>::zero()
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				5,
				FixedU128::<DECIMALS_27>::accuracy() * 10,
				SignedRounding::NearestPrefLow
			)
			.into_inner(),
			0
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				6,
				FixedU128::<DECIMALS_27>::accuracy() * 10,
				SignedRounding::NearestPrefLow
			)
			.into_inner(),
			1
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				3,
				SignedRounding::Minor
			)
			.into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				3,
				SignedRounding::Major
			)
			.into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				3,
				SignedRounding::NearestPrefLow
			)
			.into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				6,
				SignedRounding::Minor
			)
			.into_inner(),
			166666666666666666666666666
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				6,
				SignedRounding::Major
			)
			.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				6,
				SignedRounding::NearestPrefLow
			)
			.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				222220,
				SignedRounding::Minor
			)
			.into_inner(),
			4500045000450004500045
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				222220,
				SignedRounding::Major
			)
			.into_inner(),
			4500045000450004500046
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_with_rounding(
				1,
				222220,
				SignedRounding::NearestPrefLow
			)
			.into_inner(),
			4500045000450004500045
		);
	}

	#[test]
	#[should_panic(
		expected = "attempted to create fixed point from rational with zero denominator"
	)]
	fn saturating_from_rational_floor_panics_on_zero_divisor() {
		let _ = FixedU128::<DECIMALS_27>::saturating_from_rational_floor(1, 0);
	}

	#[test]
	fn saturating_from_rational_floor_works() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_floor(0, 1),
			FixedU128::<DECIMALS_27>::zero()
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_floor(
				5,
				FixedU128::<DECIMALS_27>::accuracy() * 10,
			)
			.into_inner(),
			0
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_floor(
				1,
				FixedU128::<DECIMALS_27>::accuracy()
			)
			.into_inner(),
			1
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_floor(1, 3).into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_floor(1, 6).into_inner(),
			166666666666666666666666666
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_floor(1, 222220).into_inner(),
			4500045000450004500045
		);
	}

	#[test]
	#[should_panic(
		expected = "attempted to create fixed point from rational with zero denominator"
	)]
	fn saturating_from_rational_ceil_panics_on_zero_divisor() {
		let _ = FixedU128::<DECIMALS_27>::saturating_from_rational_ceil(1, 0);
	}

	#[test]
	fn saturating_from_rational_ceil_works() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_ceil(0, 1),
			FixedU128::<DECIMALS_27>::zero()
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_ceil(
				1,
				FixedU128::<DECIMALS_27>::accuracy()
			)
			.into_inner(),
			1
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_ceil(
				5,
				FixedU128::<DECIMALS_27>::accuracy() * 10,
			)
			.into_inner(),
			1
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_ceil(1, 3).into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_ceil(1, 6).into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational_ceil(1, 222220).into_inner(),
			4500045000450004500046
		);
	}

	#[test]
	fn checked_from_rational_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		// Divide by zero => None.
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 0);
		assert_eq!(a, None);

		// Max - 1.
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_max - 1, accuracy).unwrap();
		assert_eq!(a.into_inner(), inner_max - 1);

		// Min + 1.
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_min + 1, accuracy).unwrap();
		assert_eq!(a.into_inner(), inner_min + 1);

		// Max.
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_max, accuracy).unwrap();
		assert_eq!(a.into_inner(), inner_max);

		// Min.
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_min, accuracy).unwrap();
		assert_eq!(a.into_inner(), inner_min);

		// Max + 1 => Overflow => None.
		let a =
			FixedU128::<DECIMALS_27>::checked_from_rational(inner_min, 0.saturating_sub(accuracy));
		assert_eq!(a, None);

		if FixedU128::<DECIMALS_27>::SIGNED {
			// Min - 1 => Underflow => None.
			let a = FixedU128::<DECIMALS_27>::checked_from_rational(
				inner_max as u128 + 2,
				0.saturating_sub(accuracy),
			);
			assert_eq!(a, None);

			let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_max, 0 - 3 * accuracy)
				.unwrap();
			assert_eq!(a.into_inner(), 0 - inner_max / 3);

			let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_min, 0 - accuracy / 3);
			assert_eq!(a, None);

			let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 0 - accuracy).unwrap();
			assert_eq!(a.into_inner(), 0.saturating_sub(1));

			let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 0 - accuracy - 1).unwrap();
			assert_eq!(a.into_inner(), 0);

			let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_min, accuracy / 3);
			assert_eq!(a, None);
		}

		let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_max, 3 * accuracy).unwrap();
		assert_eq!(a.into_inner(), inner_max / 3);

		let a = FixedU128::<DECIMALS_27>::checked_from_rational(inner_min, 2 * accuracy).unwrap();
		assert_eq!(a.into_inner(), inner_min / 2);

		let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, accuracy).unwrap();
		assert_eq!(a.into_inner(), 1);

		// Round to zero if equidistant
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 2 * accuracy).unwrap();
		assert_eq!(a.into_inner(), 0);

		// Round to nearest if slightly of equidistant
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 2 * accuracy - 1).unwrap();
		assert_eq!(a.into_inner(), 1);

		// Round to nearest if slightly of equidistant
		let a = FixedU128::<DECIMALS_27>::checked_from_rational(1, 2 * accuracy + 1).unwrap();
		assert_eq!(a.into_inner(), 0);
	}

	#[test]
	fn checked_from_rational_with_rounding_works() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				0,
				SignedRounding::NearestPrefLow
			),
			None
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				0,
				1,
				SignedRounding::NearestPrefLow
			),
			Some(FixedU128::<DECIMALS_27>::zero())
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				3,
				SignedRounding::Minor
			)
			.unwrap()
			.into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				3,
				SignedRounding::Major
			)
			.unwrap()
			.into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				3,
				SignedRounding::NearestPrefLow
			)
			.unwrap()
			.into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				6,
				SignedRounding::Minor
			)
			.unwrap()
			.into_inner(),
			166666666666666666666666666
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				6,
				SignedRounding::Major
			)
			.unwrap()
			.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				6,
				SignedRounding::NearestPrefLow
			)
			.unwrap()
			.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				222220,
				SignedRounding::Minor
			)
			.unwrap()
			.into_inner(),
			4500045000450004500045
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				222220,
				SignedRounding::Major
			)
			.unwrap()
			.into_inner(),
			4500045000450004500046
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_with_rounding(
				1,
				222220,
				SignedRounding::NearestPrefLow
			)
			.unwrap()
			.into_inner(),
			4500045000450004500045
		);
	}

	#[test]
	fn checked_from_rational_floor_works() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_floor(1, 0),
			None
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_floor(0, 1),
			Some(FixedU128::<DECIMALS_27>::zero())
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_floor(1, 3)
				.unwrap()
				.into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_floor(1, 6)
				.unwrap()
				.into_inner(),
			166666666666666666666666666
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_floor(1, 222220)
				.unwrap()
				.into_inner(),
			4500045000450004500045
		);
	}

	#[test]
	fn checked_from_rational_ceil_works() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_ceil(1, 0),
			None
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_ceil(0, 1),
			Some(FixedU128::<DECIMALS_27>::zero())
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_ceil(1, 3)
				.unwrap()
				.into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_ceil(1, 6)
				.unwrap()
				.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::checked_from_rational_ceil(1, 222220)
				.unwrap()
				.into_inner(),
			4500045000450004500046
		);
	}

	#[test]
	fn checked_mul_int_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(2);
		// Max - 1.
		assert_eq!(a.checked_mul_int((i128::MAX - 1) / 2), Some(i128::MAX - 1));
		// Max.
		assert_eq!(a.checked_mul_int(i128::MAX / 2), Some(i128::MAX - 1));
		// Max + 1 => None.
		assert_eq!(a.checked_mul_int(i128::MAX / 2 + 1), None);

		if FixedU128::<DECIMALS_27>::SIGNED {
			// Min - 1.
			assert_eq!(a.checked_mul_int((i128::MIN + 1) / 2), Some(i128::MIN + 2));
			// Min.
			assert_eq!(a.checked_mul_int(i128::MIN / 2), Some(i128::MIN));
			// Min + 1 => None.
			assert_eq!(a.checked_mul_int(i128::MIN / 2 - 1), None);

			let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, -2);
			assert_eq!(b.checked_mul_int(42i128), Some(-21));
			assert_eq!(b.checked_mul_int(u128::MAX), None);
			assert_eq!(b.checked_mul_int(i128::MAX), Some(i128::MAX / -2));
			assert_eq!(b.checked_mul_int(i128::MIN), Some(i128::MIN / -2));
		}

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		assert_eq!(a.checked_mul_int(42i128), Some(21));
		assert_eq!(a.checked_mul_int(i128::MAX), Some(i128::MAX / 2));
		assert_eq!(a.checked_mul_int(i128::MIN), Some(i128::MIN / 2));

		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(255);
		assert_eq!(c.checked_mul_int(2i8), None);
		assert_eq!(c.checked_mul_int(2i128), Some(510));
		assert_eq!(c.checked_mul_int(i128::MAX), None);
		assert_eq!(c.checked_mul_int(i128::MIN), None);
	}

	#[test]
	fn checked_mul_int_with_rounding_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().checked_mul_int_with_rounding(
				FixedU128::<DECIMALS_27>::max_value().into_inner(),
				SignedRounding::NearestPrefLow
			),
			None
		);

		assert_eq!(
			a.checked_mul_int_with_rounding(5, SignedRounding::NearestPrefLow),
			Some(2)
		);
		assert_eq!(
			a.checked_mul_int_with_rounding(5, SignedRounding::Minor),
			Some(2)
		);

		assert_eq!(
			a.checked_mul_int_with_rounding(5, SignedRounding::Major),
			Some(3)
		);

		assert_eq!(
			b.checked_mul_int_with_rounding(100, SignedRounding::NearestPrefLow),
			Some(33)
		);

		assert_eq!(
			b.checked_mul_int_with_rounding(100, SignedRounding::Minor),
			Some(33)
		);

		assert_eq!(
			b.checked_mul_int_with_rounding(100, SignedRounding::Major),
			Some(34)
		);

		assert_eq!(
			b.checked_mul_int_with_rounding(200, SignedRounding::NearestPrefLow),
			Some(67)
		);

		assert_eq!(
			b.checked_mul_int_with_rounding(200, SignedRounding::Minor),
			Some(66)
		);

		assert_eq!(
			b.checked_mul_int_with_rounding(200, SignedRounding::Major),
			Some(67)
		)
	}

	#[test]
	fn checked_mul_int_floor_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value()
				.checked_mul_int_floor(FixedU128::<DECIMALS_27>::max_value().into_inner()),
			None
		);

		assert_eq!(a.checked_mul_int_floor(5), Some(2));

		assert_eq!(b.checked_mul_int_floor(100), Some(33));

		assert_eq!(b.checked_mul_int_floor(200), Some(66));
	}

	#[test]
	fn checked_mul_int_ceil_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value()
				.checked_mul_int_ceil(FixedU128::<DECIMALS_27>::max_value().into_inner()),
			None
		);

		assert_eq!(a.checked_mul_int_ceil(5), Some(3));

		assert_eq!(b.checked_mul_int_ceil(100), Some(34));

		assert_eq!(b.checked_mul_int_ceil(200), Some(67));
	}

	#[test]
	fn saturating_mul_int_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(2);
		// Max - 1.
		assert_eq!(a.saturating_mul_int((i128::MAX - 1) / 2), i128::MAX - 1);
		// Max.
		assert_eq!(a.saturating_mul_int(i128::MAX / 2), i128::MAX - 1);
		// Max + 1 => saturates to max.
		assert_eq!(a.saturating_mul_int(i128::MAX / 2 + 1), i128::MAX);

		// Min - 1.
		assert_eq!(a.saturating_mul_int((i128::MIN + 1) / 2), i128::MIN + 2);
		// Min.
		assert_eq!(a.saturating_mul_int(i128::MIN / 2), i128::MIN);
		// Min + 1 => saturates to min.
		assert_eq!(a.saturating_mul_int(i128::MIN / 2 - 1), i128::MIN);

		if FixedU128::<DECIMALS_27>::SIGNED {
			let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, -2);
			assert_eq!(b.saturating_mul_int(42i32), -21);
			assert_eq!(b.saturating_mul_int(i128::MAX), i128::MAX / -2);
			assert_eq!(b.saturating_mul_int(i128::MIN), i128::MIN / -2);
			assert_eq!(b.saturating_mul_int(u128::MAX), u128::MIN);
		}

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		assert_eq!(a.saturating_mul_int(42i32), 21);
		assert_eq!(a.saturating_mul_int(i128::MAX), i128::MAX / 2);
		assert_eq!(a.saturating_mul_int(i128::MIN), i128::MIN / 2);

		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(255);
		assert_eq!(c.saturating_mul_int(2i8), i8::MAX);
		assert_eq!(c.saturating_mul_int(-2i8), i8::MIN);
		assert_eq!(c.saturating_mul_int(i128::MAX), i128::MAX);
		assert_eq!(c.saturating_mul_int(i128::MIN), i128::MIN);
	}

	#[test]
	fn saturating_mul_int_with_rounding_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().saturating_mul_int_with_rounding(
				FixedU128::<DECIMALS_27>::max_value().into_inner(),
				SignedRounding::NearestPrefLow
			),
			FixedU128::<DECIMALS_27>::max_value().into_inner()
		);

		assert_eq!(
			a.saturating_mul_int_with_rounding(5, SignedRounding::NearestPrefLow),
			2
		);
		assert_eq!(
			a.saturating_mul_int_with_rounding(5, SignedRounding::Minor),
			2
		);

		assert_eq!(
			a.saturating_mul_int_with_rounding(5, SignedRounding::Major),
			3
		);

		assert_eq!(
			b.saturating_mul_int_with_rounding(100, SignedRounding::NearestPrefLow),
			33
		);

		assert_eq!(
			b.saturating_mul_int_with_rounding(100, SignedRounding::Minor),
			33
		);

		assert_eq!(
			b.saturating_mul_int_with_rounding(100, SignedRounding::Major),
			34
		);

		assert_eq!(
			b.saturating_mul_int_with_rounding(200, SignedRounding::NearestPrefLow),
			67
		);

		assert_eq!(
			b.saturating_mul_int_with_rounding(200, SignedRounding::Minor),
			66
		);

		assert_eq!(
			b.saturating_mul_int_with_rounding(200, SignedRounding::Major),
			67
		)
	}

	#[test]
	fn saturating_mul_int_floor_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value()
				.saturating_mul_int_floor(FixedU128::<DECIMALS_27>::max_value().into_inner()),
			FixedU128::<DECIMALS_27>::max_value().into_inner()
		);

		assert_eq!(a.saturating_mul_int_floor(5), 2);

		assert_eq!(b.saturating_mul_int_floor(100), 33);

		assert_eq!(b.saturating_mul_int_floor(200), 66);
	}

	#[test]
	fn saturating_mul_int_ceil_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value()
				.saturating_mul_int_ceil(FixedU128::<DECIMALS_27>::max_value().into_inner()),
			FixedU128::<DECIMALS_27>::max_value().into_inner()
		);

		assert_eq!(a.saturating_mul_int_ceil(5), 3);

		assert_eq!(b.saturating_mul_int_ceil(100), 34);

		assert_eq!(b.saturating_mul_int_ceil(200), 67);
	}

	#[test]
	fn checked_mul_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();

		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(2);

		// Max - 1.
		let b = FixedU128::<DECIMALS_27>::from_inner(inner_max - 1);
		assert_eq!(a.checked_mul(&(b / 2.into())), Some(b));

		// Max.
		let c = FixedU128::<DECIMALS_27>::from_inner(inner_max);
		assert_eq!(a.checked_mul(&(c / 2.into())), Some(b));

		// Max + 1 => None.
		let e = FixedU128::<DECIMALS_27>::from_inner(1);
		assert_eq!(a.checked_mul(&(c / 2.into() + e)), None);

		if FixedU128::<DECIMALS_27>::SIGNED {
			// Min + 1.
			let b = FixedU128::<DECIMALS_27>::from_inner(inner_min + 1) / 2.into();
			let c = FixedU128::<DECIMALS_27>::from_inner(inner_min + 2);
			assert_eq!(a.checked_mul(&b), Some(c));

			// Min.
			let b = FixedU128::<DECIMALS_27>::from_inner(inner_min) / 2.into();
			let c = FixedU128::<DECIMALS_27>::from_inner(inner_min);
			assert_eq!(a.checked_mul(&b), Some(c));

			// Min - 1 => None.
			let b = FixedU128::<DECIMALS_27>::from_inner(inner_min) / 2.into()
				- FixedU128::<DECIMALS_27>::from_inner(1);
			assert_eq!(a.checked_mul(&b), None);

			let c = FixedU128::<DECIMALS_27>::saturating_from_integer(255);
			let b = FixedU128::<DECIMALS_27>::saturating_from_rational(1, -2);

			assert_eq!(b.checked_mul(&42.into()), Some(0.saturating_sub(21).into()));
			assert_eq!(
				b.checked_mul(&FixedU128::<DECIMALS_27>::max_value()),
				FixedU128::<DECIMALS_27>::max_value().checked_div(&0.saturating_sub(2).into())
			);
			assert_eq!(
				b.checked_mul(&FixedU128::<DECIMALS_27>::min_value()),
				FixedU128::<DECIMALS_27>::min_value().checked_div(&0.saturating_sub(2).into())
			);
			assert_eq!(c.checked_mul(&FixedU128::<DECIMALS_27>::min_value()), None);
		}

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(255);

		assert_eq!(a.checked_mul(&42.into()), Some(21.into()));
		assert_eq!(c.checked_mul(&2.into()), Some(510.into()));
		assert_eq!(c.checked_mul(&FixedU128::<DECIMALS_27>::max_value()), None);
		assert_eq!(
			a.checked_mul(&FixedU128::<DECIMALS_27>::max_value()),
			FixedU128::<DECIMALS_27>::max_value().checked_div(&2.into())
		);
		assert_eq!(
			a.checked_mul(&FixedU128::<DECIMALS_27>::min_value()),
			FixedU128::<DECIMALS_27>::min_value().checked_div(&2.into())
		);
	}

	#[test]
	fn checked_mul_floor_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value()
				.checked_mul_floor(&FixedU128::<DECIMALS_27>::max_value()),
			None
		);

		// Round down if equidistant
		assert_eq!(
			a.checked_mul_floor(&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2))
				.unwrap()
				.into_inner(),
			0
		);

		// Round to floor when closer to floor
		assert_eq!(
			a.checked_mul_floor(&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3))
				.unwrap()
				.into_inner(),
			0
		);

		// Round up even if closer to ceil
		assert_eq!(
			a.checked_mul_floor(&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 6))
				.unwrap()
				.into_inner(),
			0
		);
	}

	#[test]
	fn checked_mul_ceil_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value()
				.checked_mul_floor(&FixedU128::<DECIMALS_27>::max_value()),
			None
		);

		// Round up if equidistant
		assert_eq!(
			a.checked_mul_ceil(&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2))
				.unwrap()
				.into_inner(),
			1
		);

		// Round to ceil even when closer to floor
		assert_eq!(
			a.checked_mul_ceil(&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3))
				.unwrap()
				.into_inner(),
			1
		);

		// Round up if closer to ceil
		assert_eq!(
			a.checked_mul_ceil(&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 6))
				.unwrap()
				.into_inner(),
			1
		);
	}

	#[test]
	fn checked_mul_with_rounding_works() {
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::max_value(),
				SignedRounding::NearestPrefLow
			),
			None
		);

		// Round down if equidistant and NearestPrefLow
		assert_eq!(
			a.checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2),
				SignedRounding::NearestPrefLow
			)
			.unwrap()
			.into_inner(),
			0
		);

		// Round to floor when closer to floor and NearestPrefLow
		assert_eq!(
			a.checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3),
				SignedRounding::NearestPrefLow
			)
			.unwrap()
			.into_inner(),
			0
		);

		// Round up  if closer to ceil and NearestPrefLow
		assert_eq!(
			a.checked_mul_floor(&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 6))
				.unwrap()
				.into_inner(),
			0
		);

		// note: these behaviours are also tested in floor/ceil
		// switching to have tests in both with_rounding and floor/ceil so as to allow
		// implementations to be decoupled

		// Round down with Minor when closer to floor
		assert_eq!(
			a.checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2),
				SignedRounding::Minor
			)
			.unwrap()
			.into_inner(),
			0
		);

		// Round to floor when closer to floor
		assert_eq!(
			a.checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3),
				SignedRounding::Minor
			)
			.unwrap()
			.into_inner(),
			0
		);

		// Round down even if closer to ceil
		assert_eq!(
			a.checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 6),
				SignedRounding::Minor
			)
			.unwrap()
			.into_inner(),
			0
		);

		// Round up if equidistant with Major
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);
		assert_eq!(
			a.checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2),
				SignedRounding::Major
			)
			.unwrap()
			.into_inner(),
			1
		);

		// Round to ceil even when closer to floor with Major
		assert_eq!(
			a.checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3),
				SignedRounding::Major
			)
			.unwrap()
			.into_inner(),
			1
		);

		// Round up if closer to ceil with Major
		assert_eq!(
			a.checked_mul_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(1, 6),
				SignedRounding::Major
			)
			.unwrap()
			.into_inner(),
			1
		);
	}

	#[test]
	fn saturating_mul_with_rounding_works() {
		// Round down if equidistant and NearestPrefLow
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2),
				SignedRounding::NearestPrefLow
			)
			.into_inner(),
			0
		);

		// Round up if equidistant and NearestPrefLow
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2),
				SignedRounding::NearestPrefHigh
			)
			.into_inner(),
			1
		);

		// Round to floor when closer to floor and NearestPrefLow
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3),
				SignedRounding::NearestPrefLow
			)
			.into_inner(),
			0
		);

		// Round up  if closer to ceil and NearestPrefLow
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(2, 3),
				SignedRounding::NearestPrefLow
			)
			.into_inner(),
			1
		);

		// Verify result saturates when out of bounds
		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_integer(2),
				SignedRounding::Major
			),
			FixedU128::<DECIMALS_27>::max_value()
		);

		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2),
				SignedRounding::Minor
			)
			.into_inner(),
			0
		);

		// Round to floor when closer to floor
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3),
				SignedRounding::Minor
			)
			.into_inner(),
			0
		);

		// Round down even if closer to ceil
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(2, 3),
				SignedRounding::Minor
			)
			.into_inner(),
			0
		);

		// Round up if equidistant with Major
		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2),
				SignedRounding::Major
			)
			.into_inner(),
			1
		);

		// Round to ceil even when closer to floor with Major
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3),
				SignedRounding::Major
			)
			.into_inner(),
			1
		);

		// Round up if closer to ceil with Major
		assert_eq!(
			a.saturating_mul_with_rounding(
				FixedU128::<DECIMALS_27>::saturating_from_rational(2, 3),
				SignedRounding::Major
			)
			.into_inner(),
			1
		);
	}

	#[test]
	fn saturating_mul_floor_works() {
		// Verify result saturates when out of bounds
		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value()
				.saturating_mul_floor(FixedU128::<DECIMALS_27>::saturating_from_integer(2)),
			FixedU128::<DECIMALS_27>::max_value()
		);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(1);

		// Round down when equidistant
		assert_eq!(
			a.saturating_mul_floor(FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2))
				.into_inner(),
			0
		);

		// Round to floor when closer to floor
		assert_eq!(
			a.saturating_mul_floor(FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3))
				.into_inner(),
			0
		);

		assert_eq!(
			b.saturating_mul_floor(FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3))
				.into_inner(),
			333333333333333333333333333
		);

		// Round down even if closer to ceil
		assert_eq!(
			a.saturating_mul_floor(FixedU128::<DECIMALS_27>::saturating_from_rational(2, 3))
				.into_inner(),
			0
		);
	}

	#[test]
	fn saturating_mul_ceil_works() {
		// Verify result saturates when out of bounds
		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value()
				.saturating_mul_ceil(FixedU128::<DECIMALS_27>::saturating_from_integer(2)),
			FixedU128::<DECIMALS_27>::max_value()
		);

		let a = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);

		// Round up when equidistant
		assert_eq!(
			a.saturating_mul_ceil(FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2))
				.into_inner(),
			1
		);

		// Round to ceil when closer to floor
		assert_eq!(
			a.saturating_mul_ceil(FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3))
				.into_inner(),
			1
		);

		// Round to ceil if closer to ceil
		assert_eq!(
			a.saturating_mul_ceil(FixedU128::<DECIMALS_27>::saturating_from_rational(2, 3))
				.into_inner(),
			1
		);
	}

	#[test]
	fn checked_div_int_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::from_inner(inner_max);
		let b = FixedU128::<DECIMALS_27>::from_inner(inner_min);
		let c = FixedU128::<DECIMALS_27>::zero();
		let d = FixedU128::<DECIMALS_27>::one();
		let e = FixedU128::<DECIMALS_27>::saturating_from_integer(6);
		let f = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		assert_eq!(e.checked_div_int(2.into()), Some(3));
		assert_eq!(f.checked_div_int(2.into()), Some(2));

		assert_eq!(a.checked_div_int(i128::MAX), Some(0));
		assert_eq!(a.checked_div_int(2), Some(inner_max / (2 * accuracy)));
		assert_eq!(a.checked_div_int(inner_max / accuracy), Some(1));
		assert_eq!(a.checked_div_int(1i8), None);

		if b < c {
			// Not executed by unsigned inners.
			assert_eq!(
				a.checked_div_int(0.saturating_sub(2)),
				Some(0.saturating_sub(inner_max / (2 * accuracy)))
			);
			assert_eq!(
				a.checked_div_int(0.saturating_sub(inner_max / accuracy)),
				Some(0.saturating_sub(1))
			);
			assert_eq!(b.checked_div_int(i128::MIN), Some(0));
			assert_eq!(b.checked_div_int(inner_min / accuracy), Some(1));
			assert_eq!(b.checked_div_int(1i8), None);
			assert_eq!(
				b.checked_div_int(0.saturating_sub(2)),
				Some(0.saturating_sub(inner_min / (2 * accuracy)))
			);
			assert_eq!(
				b.checked_div_int(0.saturating_sub(inner_min / accuracy)),
				Some(0.saturating_sub(1))
			);
			assert_eq!(c.checked_div_int(i128::MIN), Some(0));
			assert_eq!(d.checked_div_int(i32::MIN), Some(0));
		}

		assert_eq!(b.checked_div_int(2), Some(inner_min / (2 * accuracy)));

		assert_eq!(c.checked_div_int(1), Some(0));
		assert_eq!(c.checked_div_int(i128::MAX), Some(0));
		assert_eq!(c.checked_div_int(1i8), Some(0));

		assert_eq!(d.checked_div_int(1), Some(1));
		assert_eq!(d.checked_div_int(i32::MAX), Some(0));
		assert_eq!(d.checked_div_int(1i8), Some(1));

		assert_eq!(a.checked_div_int(0), None);
		assert_eq!(b.checked_div_int(0), None);
		assert_eq!(c.checked_div_int(0), None);
		assert_eq!(d.checked_div_int(0), None);
	}

	#[test]
	fn checked_div_int_with_rounding_works() {
		// Note:  This assumes that the FP accuracy has been adjusted to match
		// the accuracy of the FP extended type in question (FixedU128::<DECIMALS_27> in
		// this case)
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::from_inner(inner_max);
		let b = FixedU128::<DECIMALS_27>::from_inner(inner_min);
		let c = FixedU128::<DECIMALS_27>::zero();
		let d = FixedU128::<DECIMALS_27>::one();
		let e = FixedU128::<DECIMALS_27>::saturating_from_integer(6);
		let f = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		let max = FixedU128::<DECIMALS_27>::max_value();

		// verify it actually returns None when result too large
		// note 2 would be equivalent to 2/accuracy
		assert_eq!(
			max.checked_div_int_with_rounding(2, SignedRounding::NearestPrefLow),
			None
		);

		// Note: adjusted for Fixed Point accuracy would be .3333....
		assert_eq!(
			d.checked_div_int_with_rounding(3 * accuracy, SignedRounding::Minor),
			Some(333333333333333333333333333)
		);

		assert_eq!(
			d.checked_div_int_with_rounding(3 * accuracy, SignedRounding::Major),
			Some(333333333333333333333333334)
		);

		assert_eq!(
			d.checked_div_int_with_rounding(3 * accuracy, SignedRounding::NearestPrefLow),
			Some(333333333333333333333333333)
		);

		// Note 166666666666666666666666666 adjusted for Fixed Point accuracy would be
		// .16666....
		assert_eq!(
			d.checked_div_int_with_rounding(6 * accuracy, SignedRounding::Minor),
			Some(166666666666666666666666666)
		);

		assert_eq!(
			d.checked_div_int_with_rounding(6 * accuracy, SignedRounding::Major),
			Some(166666666666666666666666667)
		);

		assert_eq!(
			d.checked_div_int_with_rounding(6 * accuracy, SignedRounding::NearestPrefLow),
			Some(166666666666666666666666667)
		);

		// Note: adjusted for FP accuracy would be .555555.....
		assert_eq!(
			f.checked_div_int_with_rounding(9 * accuracy, SignedRounding::Minor),
			Some(555555555555555555555555555)
		);

		assert_eq!(
			f.checked_div_int_with_rounding(9 * accuracy, SignedRounding::Major),
			Some(555555555555555555555555556)
		);

		assert_eq!(
			f.checked_div_int_with_rounding(9 * accuracy, SignedRounding::NearestPrefLow),
			Some(555555555555555555555555556)
		);

		assert_eq!(
			e.checked_div_int_with_rounding(
				2000000000000000000000000000u128,
				SignedRounding::NearestPrefLow
			),
			Some(3000000000000000000000000000)
		);
		assert_eq!(
			f.checked_div_int_with_rounding(
				2000000000000000000000000000u128,
				SignedRounding::NearestPrefLow
			),
			Some(2500000000000000000000000000u128) /*
												   Some(FixedU128::<DECIMALS_27>::saturating_from_rational(5, * 2).into_inner().into()) */
		);

		assert_eq!(
			a.checked_div_int_with_rounding(u128::MAX, SignedRounding::NearestPrefLow),
			Some(1 * accuracy)
		);
		// Note with FP decimal point accounted for this would be:
		// 3402823669209.38463463374607431768211455/2.000000000000000000000000000 ==
		// 17014118346.0469231731687303715884105727
		assert_eq!(
			a.checked_div_int_with_rounding(
				2000000000000000000000000000u128,
				SignedRounding::NearestPrefLow
			),
			Some(170141183460469231731687303715884105727)
		);
		assert_eq!(
			a.checked_div_int_with_rounding(
				340282366920938463463374607431768211455u128,
				SignedRounding::NearestPrefLow
			),
			Some(1 * accuracy)
		);

		// With accuracy correction this would be a * (accuracy/1)
		// not a * (1/1)
		assert_eq!(
			a.checked_div_int_with_rounding(1i8, SignedRounding::NearestPrefLow),
			None
		);

		assert_eq!(
			b.checked_div_int_with_rounding(2, SignedRounding::NearestPrefLow),
			Some(inner_min / (2 * accuracy))
		);

		assert_eq!(
			c.checked_div_int_with_rounding(1, SignedRounding::NearestPrefLow),
			Some(0)
		);

		assert_eq!(
			c.checked_div_int_with_rounding(inner_max, SignedRounding::Major),
			Some(0)
		);
		assert_eq!(
			c.checked_div_int_with_rounding(inner_min, SignedRounding::NearestPrefLow),
			None
		);

		assert_eq!(
			a.checked_div_int_with_rounding(0, SignedRounding::NearestPrefLow),
			None
		);
		assert_eq!(
			b.checked_div_int_with_rounding(0, SignedRounding::NearestPrefLow),
			None
		);
		assert_eq!(
			c.checked_div_int_with_rounding(0, SignedRounding::NearestPrefLow),
			None
		);
		assert_eq!(
			d.checked_div_int_with_rounding(0, SignedRounding::NearestPrefLow),
			None
		);
	}

	#[test]
	fn checked_div_int_floor() {
		// Note:  This assumes that the FP accuracy has been adjusted to match
		// the accuracy of the FP extended type in question (FixedU128::<DECIMALS_27> in
		// this case)
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::one();
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		let max = FixedU128::<DECIMALS_27>::max_value();

		// verify it actually returns None when result too large
		// note 2 would be equivalent to 2/accuracy
		assert_eq!(max.checked_div_int_floor(2), None);

		// Note: adjusted for Fixed Point accuracy would be .3333....
		assert_eq!(
			a.checked_div_int_floor(3 * accuracy),
			Some(333333333333333333333333333)
		);

		// Note 166666666666666666666666666 adjusted for Fixed Point accuracy would be
		// .16666....
		assert_eq!(
			a.checked_div_int_floor(6 * accuracy),
			Some(166666666666666666666666666)
		);

		// Note: adjusted for FP accuracy would be .555555.....
		assert_eq!(
			b.checked_div_int_floor(9 * accuracy),
			Some(555555555555555555555555555)
		)
	}

	#[test]
	fn checked_div_int_ceil() {
		// Note:  This assumes that the FP accuracy has been adjusted to match
		// the accuracy of the FP extended type in question (FixedU128::<DECIMALS_27> in
		// this case)
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::one();
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		let max = FixedU128::<DECIMALS_27>::max_value();

		// verify it actually returns None when result too large
		// note 2 would be equivalent to 2/accuracy
		assert_eq!(max.checked_div_int_ceil(2), None);

		// Note: adjusted for Fixed Point accuracy would be .3333....
		assert_eq!(
			a.checked_div_int_with_rounding(3 * accuracy, SignedRounding::Major),
			Some(333333333333333333333333334)
		);

		// Note 166666666666666666666666667 adjusted for Fixed Point accuracy would be
		// .16666....
		assert_eq!(
			a.checked_div_int_with_rounding(6 * accuracy, SignedRounding::Major),
			Some(166666666666666666666666667)
		);
		// Note: adjusted for FP accuracy would be .555555.....
		assert_eq!(
			b.checked_div_int_with_rounding(9 * accuracy, SignedRounding::Major),
			Some(555555555555555555555555556)
		);
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn saturating_div_int_panics_when_divisor_is_zero() {
		let _ = FixedU128::<DECIMALS_27>::one().saturating_div_int(0);
	}

	#[test]
	fn saturating_div_int_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(5);
		assert_eq!(a.saturating_div_int(2), 2);

		let a = FixedU128::<DECIMALS_27>::min_value();
		assert_eq!(a.saturating_div_int(1i128), (inner_min / accuracy) as i128);

		if FixedU128::<DECIMALS_27>::SIGNED {
			let a = FixedU128::<DECIMALS_27>::saturating_from_integer(5);
			assert_eq!(a.saturating_div_int(-2), -2);

			let a = FixedU128::<DECIMALS_27>::min_value();
			assert_eq!(a.saturating_div_int(-1i128), (inner_max / accuracy) as i128);
		}
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn saturating_div_int_with_rounding_panics_when_divisor_is_zero() {
		let _ = FixedU128::<DECIMALS_27>::one()
			.saturating_div_int_with_rounding(0, SignedRounding::NearestPrefLow);
	}

	#[test]
	fn saturating_div_int_with_rounding_works() {
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(5);
		let b = FixedU128::<DECIMALS_27>::min_value();
		let d = FixedU128::<DECIMALS_27>::one();
		let e = FixedU128::<DECIMALS_27>::saturating_from_integer(5);
		let max = FixedU128::<DECIMALS_27>::max_value();

		assert_eq!(
			a.saturating_div_int_with_rounding(2 * accuracy, SignedRounding::NearestPrefLow),
			2500000000000000000000000000
		);

		assert_eq!(
			b.saturating_div_int_with_rounding(1i128, SignedRounding::NearestPrefLow),
			(inner_min / accuracy) as i128
		);

		// verify it actually saturates
		assert_eq!(
			max.saturating_div_int_with_rounding(2, SignedRounding::NearestPrefLow),
			inner_max
		);

		// Note: adjusted for Fixed Point accuracy would be .3333....
		assert_eq!(
			d.saturating_div_int_with_rounding(3 * accuracy, SignedRounding::Minor),
			333333333333333333333333333
		);

		assert_eq!(
			d.saturating_div_int_with_rounding(3 * accuracy, SignedRounding::Major),
			333333333333333333333333334
		);

		assert_eq!(
			d.saturating_div_int_with_rounding(3 * accuracy, SignedRounding::NearestPrefLow),
			333333333333333333333333333
		);

		// Note 166666666666666666666666666 adjusted for Fixed Point accuracy would be
		// .16666....
		assert_eq!(
			d.saturating_div_int_with_rounding(6 * accuracy, SignedRounding::Minor),
			166666666666666666666666666
		);

		assert_eq!(
			d.saturating_div_int_with_rounding(6 * accuracy, SignedRounding::Major),
			166666666666666666666666667
		);

		assert_eq!(
			d.saturating_div_int_with_rounding(6 * accuracy, SignedRounding::NearestPrefLow),
			166666666666666666666666667
		);

		// Note: adjusted for FP accuracy would be .555555.....
		assert_eq!(
			e.saturating_div_int_with_rounding(9 * accuracy, SignedRounding::Minor),
			555555555555555555555555555
		);

		assert_eq!(
			e.saturating_div_int_with_rounding(9 * accuracy, SignedRounding::Major),
			555555555555555555555555556
		);

		assert_eq!(
			e.saturating_div_int_with_rounding(9 * accuracy, SignedRounding::NearestPrefLow),
			555555555555555555555555556
		);
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn saturating_div_int_floor_panics_when_divisor_is_zero() {
		let _ = FixedU128::<DECIMALS_27>::one().saturating_div_int_floor(0);
	}
	#[test]
	fn saturating_div_int_floor() {
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::one();
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		// verify it actually saturates
		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().saturating_div_int_floor(2),
			FixedU128::<DECIMALS_27>::max_value().into_inner()
		);

		// Note: adjusted for Fixed Point accuracy would be .3333....
		assert_eq!(
			a.saturating_div_int_floor(3 * accuracy),
			333333333333333333333333333
		);

		// Note 166666666666666666666666666 adjusted for Fixed Point accuracy would be
		// .16666....
		assert_eq!(
			a.saturating_div_int_floor(6 * accuracy),
			166666666666666666666666666
		);

		// Note: adjusted for FP accuracy would be .555555.....
		assert_eq!(
			b.saturating_div_int_floor(9 * accuracy),
			555555555555555555555555555
		);
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn saturating_div_int_ceil_panics_when_divisor_is_zero() {
		let _ = FixedU128::<DECIMALS_27>::one().saturating_div_int_ceil(0);
	}

	#[test]
	fn saturating_div_int_ceil() {
		let accuracy = FixedU128::<DECIMALS_27>::accuracy();

		let a = FixedU128::<DECIMALS_27>::one();
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		// verify it actually saturates
		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().saturating_div_int_ceil(2),
			FixedU128::<DECIMALS_27>::max_value().into_inner()
		);

		// Note: adjusted for Fixed Point accuracy would be .3333....
		assert_eq!(
			a.saturating_div_int_ceil(3 * accuracy),
			333333333333333333333333334
		);

		// Note 166666666666666666666666666 adjusted for Fixed Point accuracy would be
		// .16666....
		assert_eq!(
			a.saturating_div_int_ceil(6 * accuracy),
			166666666666666666666666667
		);

		// Note: adjusted for FP accuracy would be .555555.....
		assert_eq!(
			b.saturating_div_int_ceil(9 * accuracy),
			555555555555555555555555556
		);
	}

	#[test]
	fn saturating_abs_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();

		assert_eq!(
			FixedU128::<DECIMALS_27>::from_inner(inner_max).saturating_abs(),
			FixedU128::<DECIMALS_27>::max_value()
		);
		assert_eq!(FixedU128::<DECIMALS_27>::zero().saturating_abs(), 0.into());

		if FixedU128::<DECIMALS_27>::SIGNED {
			assert_eq!(
				FixedU128::<DECIMALS_27>::from_inner(inner_min).saturating_abs(),
				FixedU128::<DECIMALS_27>::max_value()
			);
			assert_eq!(
				FixedU128::<DECIMALS_27>::saturating_from_rational(-1, 2).saturating_abs(),
				(1, 2).into()
			);
		}
	}

	#[test]
	fn saturating_mul_acc_int_works() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::zero().saturating_mul_acc_int(42i8),
			42i8
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::one().saturating_mul_acc_int(42i8),
			2 * 42i8
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::one().saturating_mul_acc_int(i128::MAX),
			i128::MAX
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::one().saturating_mul_acc_int(i128::MIN),
			i128::MIN
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::one().saturating_mul_acc_int(u128::MAX / 2),
			u128::MAX - 1
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::one().saturating_mul_acc_int(u128::MIN),
			u128::MIN
		);

		if FixedU128::<DECIMALS_27>::SIGNED {
			let a = FixedU128::<DECIMALS_27>::saturating_from_rational(-1, 2);
			assert_eq!(a.saturating_mul_acc_int(42i8), 21i8);
			assert_eq!(a.saturating_mul_acc_int(42u8), 21u8);
			assert_eq!(a.saturating_mul_acc_int(u128::MAX - 1), u128::MAX / 2);
		}
	}

	#[test]
	fn saturating_pow_should_work() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow(0),
			FixedU128::<DECIMALS_27>::saturating_from_integer(1)
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow(1),
			FixedU128::<DECIMALS_27>::saturating_from_integer(2)
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow(2),
			FixedU128::<DECIMALS_27>::saturating_from_integer(4)
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow(50),
			FixedU128::<DECIMALS_27>::saturating_from_integer(1125899906842624i64)
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(1).saturating_pow(1000),
			(1).into()
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(1).saturating_pow(usize::MAX),
			(1).into()
		);

		if FixedU128::<DECIMALS_27>::SIGNED {
			// Saturating.
			assert_eq!(
				FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow(68),
				FixedU128::<DECIMALS_27>::max_value()
			);

			assert_eq!(
				FixedU128::<DECIMALS_27>::saturating_from_integer(-1).saturating_pow(1000),
				(1).into()
			);
			assert_eq!(
				FixedU128::<DECIMALS_27>::saturating_from_integer(-1).saturating_pow(1001),
				0.saturating_sub(1).into()
			);
			assert_eq!(
				FixedU128::<DECIMALS_27>::saturating_from_integer(-1).saturating_pow(usize::MAX),
				0.saturating_sub(1).into()
			);
			assert_eq!(
				FixedU128::<DECIMALS_27>::saturating_from_integer(-1)
					.saturating_pow(usize::MAX - 1),
				(1).into()
			);
		}

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(114209).saturating_pow(5),
			FixedU128::<DECIMALS_27>::max_value()
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(1).saturating_pow(usize::MAX),
			(1).into()
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(0).saturating_pow(usize::MAX),
			(0).into()
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow(usize::MAX),
			FixedU128::<DECIMALS_27>::max_value()
		);
	}

	#[test]
	fn saturating_pow_with_rounding_works() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2)
				.saturating_pow_with_rounding(0, SignedRounding::NearestPrefLow),
			FixedU128::<DECIMALS_27>::saturating_from_integer(1)
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2)
				.saturating_pow_with_rounding(1, SignedRounding::NearestPrefLow),
			FixedU128::<DECIMALS_27>::saturating_from_integer(2)
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2)
				.saturating_pow_with_rounding(2, SignedRounding::NearestPrefLow),
			FixedU128::<DECIMALS_27>::saturating_from_integer(4)
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3)
				.saturating_pow_with_rounding(2, SignedRounding::Minor)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.1111....)
			111111111111111111111111110
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3)
				.saturating_pow_with_rounding(2, SignedRounding::NearestPrefLow)
				.into_inner(),
			111111111111111111111111111
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3)
				.saturating_pow_with_rounding(2, SignedRounding::Major)
				.into_inner(),
			111111111111111111111111111
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 15)
				.saturating_pow_with_rounding(2, SignedRounding::Minor)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.004....)
			4444444444444444444444444
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 15)
				.saturating_pow_with_rounding(2, SignedRounding::NearestPrefLow)
				.into_inner(),
			4444444444444444444444444
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 15)
				.saturating_pow_with_rounding(2, SignedRounding::Major)
				.into_inner(),
			4444444444444444444444445
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(5, 100000000000000i64)
				.saturating_pow_with_rounding(2, SignedRounding::Minor)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.000000000000000000000000002)
			0000000000000000000000000002
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(5, 100000000000000i64)
				.saturating_pow_with_rounding(2, SignedRounding::NearestPrefLow)
				.into_inner(),
			0000000000000000000000000002
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(5, 100000000000000i64)
				.saturating_pow_with_rounding(2, SignedRounding::NearestPrefHigh)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.000000000000000000000000003)
			0000000000000000000000000003
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(5, 100000000000000i64)
				.saturating_pow_with_rounding(2, SignedRounding::Major)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.000000000000000000000000003)
			0000000000000000000000000003
		)
	}

	#[test]
	fn saturating_pow_floor() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow_floor(0),
			FixedU128::<DECIMALS_27>::saturating_from_integer(1)
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow_floor(1),
			FixedU128::<DECIMALS_27>::saturating_from_integer(2)
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow_floor(2),
			FixedU128::<DECIMALS_27>::saturating_from_integer(4)
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3)
				.saturating_pow_floor(2)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.1111....)
			111111111111111111111111110
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 15)
				.saturating_pow_floor(2)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.004....)
			4444444444444444444444444
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(5, 100000000000000i64)
				.saturating_pow_floor(2)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.000000000000000000000000002)
			0000000000000000000000000002
		);
	}

	#[test]
	fn saturating_pow_ceil() {
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow_ceil(0),
			FixedU128::<DECIMALS_27>::saturating_from_integer(1)
		);
		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow_ceil(1),
			FixedU128::<DECIMALS_27>::saturating_from_integer(2)
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_integer(2).saturating_pow_ceil(2),
			FixedU128::<DECIMALS_27>::saturating_from_integer(4)
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 3)
				.saturating_pow_ceil(2)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.1111....)
			111111111111111111111111111
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(1, 15)
				.saturating_pow_ceil(2)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.004....)
			4444444444444444444444445
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::saturating_from_rational(5, 100000000000000i64)
				.saturating_pow_ceil(2)
				.into_inner(),
			// equiv to FixedU128::<DECIMALS_27>(0.000000000000000000000000003)
			0000000000000000000000000003
		);
	}

	#[test]
	fn checked_div_works() {
		let inner_max = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::max_value();
		let inner_min = <FixedU128<DECIMALS_27> as FixedPointNumber>::Inner::min_value();

		let a = FixedU128::<DECIMALS_27>::from_inner(inner_max);
		let b = FixedU128::<DECIMALS_27>::from_inner(inner_min);
		let c = FixedU128::<DECIMALS_27>::zero();
		let d = FixedU128::<DECIMALS_27>::one();
		let e = FixedU128::<DECIMALS_27>::saturating_from_integer(6);
		let f = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		assert_eq!(e.checked_div(&2.into()), Some(3.into()));
		assert_eq!(f.checked_div(&2.into()), Some((5, 2).into()));

		assert_eq!(a.checked_div(&inner_max.into()), Some(1.into()));
		assert_eq!(
			a.checked_div(&2.into()),
			Some(FixedU128::<DECIMALS_27>::from_inner(inner_max / 2))
		);
		assert_eq!(
			a.checked_div(&FixedU128::<DECIMALS_27>::max_value()),
			Some(1.into())
		);
		assert_eq!(a.checked_div(&d), Some(a));

		if b < c {
			// Not executed by unsigned inners.
			assert_eq!(
				a.checked_div(&0.saturating_sub(2).into()),
				Some(FixedU128::<DECIMALS_27>::from_inner(
					0.saturating_sub(inner_max / 2)
				))
			);
			assert_eq!(
				a.checked_div(&-FixedU128::<DECIMALS_27>::max_value()),
				Some(0.saturating_sub(1).into())
			);
			assert_eq!(
				b.checked_div(&0.saturating_sub(2).into()),
				Some(FixedU128::<DECIMALS_27>::from_inner(
					0.saturating_sub(inner_min / 2)
				))
			);
			assert_eq!(
				c.checked_div(&FixedU128::<DECIMALS_27>::max_value()),
				Some(0.into())
			);
			assert_eq!(b.checked_div(&b), Some(FixedU128::<DECIMALS_27>::one()));
		}

		assert_eq!(
			b.checked_div(&2.into()),
			Some(FixedU128::<DECIMALS_27>::from_inner(inner_min / 2))
		);
		assert_eq!(b.checked_div(&a), Some(0.saturating_sub(1).into()));
		assert_eq!(c.checked_div(&1.into()), Some(0.into()));
		assert_eq!(d.checked_div(&1.into()), Some(1.into()));

		assert_eq!(a.checked_div(&FixedU128::<DECIMALS_27>::one()), Some(a));
		assert_eq!(b.checked_div(&FixedU128::<DECIMALS_27>::one()), Some(b));
		assert_eq!(c.checked_div(&FixedU128::<DECIMALS_27>::one()), Some(c));
		assert_eq!(d.checked_div(&FixedU128::<DECIMALS_27>::one()), Some(d));

		assert_eq!(a.checked_div(&FixedU128::<DECIMALS_27>::zero()), None);
		assert_eq!(b.checked_div(&FixedU128::<DECIMALS_27>::zero()), None);
		assert_eq!(c.checked_div(&FixedU128::<DECIMALS_27>::zero()), None);
		assert_eq!(d.checked_div(&FixedU128::<DECIMALS_27>::zero()), None);
	}

	#[test]
	fn checked_div_with_rounding_works() {
		let zero = FixedU128::<DECIMALS_27>::zero();
		let one = FixedU128::<DECIMALS_27>::one();
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(6);

		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(9);
		let d = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		assert_eq!(
			one.checked_div_with_rounding(&zero, SignedRounding::NearestPrefLow),
			None
		);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().checked_div_with_rounding(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(
					1,
					FixedU128::<DECIMALS_27>::accuracy()
				),
				SignedRounding::NearestPrefLow
			),
			None
		);

		assert_eq!(
			one.checked_div_with_rounding(&a, SignedRounding::Minor)
				.unwrap()
				.into_inner(),
			333333333333333333333333333
		);
		assert_eq!(
			one.checked_div_with_rounding(&a, SignedRounding::NearestPrefLow)
				.unwrap()
				.into_inner(),
			333333333333333333333333333
		);
		assert_eq!(
			one.checked_div_with_rounding(&a, SignedRounding::Major)
				.unwrap()
				.into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			one.checked_div_with_rounding(&b, SignedRounding::Minor)
				.unwrap()
				.into_inner(),
			166666666666666666666666666
		);
		assert_eq!(
			one.checked_div_with_rounding(&b, SignedRounding::NearestPrefLow)
				.unwrap()
				.into_inner(),
			166666666666666666666666667
		);
		assert_eq!(
			one.checked_div_with_rounding(&b, SignedRounding::Major)
				.unwrap()
				.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			d.checked_div_with_rounding(&c, SignedRounding::Minor)
				.unwrap()
				.into_inner(),
			555555555555555555555555555
		);
		assert_eq!(
			d.checked_div_with_rounding(&c, SignedRounding::NearestPrefLow)
				.unwrap()
				.into_inner(),
			555555555555555555555555556
		);
		assert_eq!(
			d.checked_div_with_rounding(&c, SignedRounding::Major)
				.unwrap()
				.into_inner(),
			555555555555555555555555556
		);
	}

	#[test]
	fn checked_div_floor_works() {
		let zero = FixedU128::<DECIMALS_27>::zero();
		let one = FixedU128::<DECIMALS_27>::one();
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(6);

		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(9);
		let d = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		assert_eq!(one.checked_div_floor(&zero), None);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().checked_div_floor(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(
					1,
					FixedU128::<DECIMALS_27>::accuracy()
				)
			),
			None
		);

		assert_eq!(
			one.checked_div_floor(&a).unwrap().into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			one.checked_div_floor(&b).unwrap().into_inner(),
			166666666666666666666666666
		);

		assert_eq!(
			d.checked_div_floor(&c).unwrap().into_inner(),
			555555555555555555555555555
		);
	}

	#[test]
	fn checked_div_ceil_works() {
		let zero = FixedU128::<DECIMALS_27>::zero();
		let one = FixedU128::<DECIMALS_27>::one();
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(6);

		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(9);
		let d = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		assert_eq!(one.checked_div_ceil(&zero), None);

		assert_eq!(
			FixedU128::<DECIMALS_27>::max_value().checked_div_floor(
				&FixedU128::<DECIMALS_27>::saturating_from_rational(
					1,
					FixedU128::<DECIMALS_27>::accuracy()
				)
			),
			None
		);

		assert_eq!(
			one.checked_div_ceil(&a).unwrap().into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			one.checked_div_ceil(&b).unwrap().into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			d.checked_div_ceil(&c).unwrap().into_inner(),
			555555555555555555555555556
		);
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn saturating_div_with_rounding_panics_on_zero_divizor() {
		let _ = FixedU128::<DECIMALS_27>::saturating_from_integer(6).saturating_div_with_rounding(
			&FixedU128::<DECIMALS_27>::zero(),
			SignedRounding::NearestPrefLow,
		);
	}

	#[test]
	fn saturating_div_with_rounding_works() {
		let one = FixedU128::<DECIMALS_27>::one();
		let zero = FixedU128::<DECIMALS_27>::zero();
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(6);

		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(9);
		let d = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		let e = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);

		assert_eq!(
			zero.saturating_div_with_rounding(&a, SignedRounding::NearestPrefLow)
				.into_inner(),
			0
		);

		assert_eq!(
			d.saturating_div_with_rounding(&e, SignedRounding::NearestPrefLow),
			FixedU128::<DECIMALS_27>::max_value()
		);

		assert_eq!(
			one.saturating_div_with_rounding(&a, SignedRounding::Minor)
				.into_inner(),
			333333333333333333333333333
		);
		assert_eq!(
			one.saturating_div_with_rounding(&a, SignedRounding::NearestPrefLow)
				.into_inner(),
			333333333333333333333333333
		);
		assert_eq!(
			one.saturating_div_with_rounding(&a, SignedRounding::Major)
				.into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			one.saturating_div_with_rounding(&b, SignedRounding::Minor)
				.into_inner(),
			166666666666666666666666666
		);
		assert_eq!(
			one.saturating_div_with_rounding(&b, SignedRounding::NearestPrefLow)
				.into_inner(),
			166666666666666666666666667
		);
		assert_eq!(
			one.saturating_div_with_rounding(&b, SignedRounding::Major)
				.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			d.saturating_div_with_rounding(&c, SignedRounding::Minor)
				.into_inner(),
			555555555555555555555555555
		);
		assert_eq!(
			d.saturating_div_with_rounding(&c, SignedRounding::NearestPrefLow)
				.into_inner(),
			555555555555555555555555556
		);
		assert_eq!(
			d.saturating_div_with_rounding(&c, SignedRounding::Major)
				.into_inner(),
			555555555555555555555555556
		);
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn saturating_div_floor_panics_on_zero_divizor() {
		let _ = FixedU128::<DECIMALS_27>::saturating_from_integer(6)
			.saturating_div_floor(&FixedU128::<DECIMALS_27>::zero());
	}

	#[test]
	fn saturating_div_floor_works() {
		let one = FixedU128::<DECIMALS_27>::one();
		let zero = FixedU128::<DECIMALS_27>::zero();
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(6);

		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(9);
		let d = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		let e = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);

		assert_eq!(zero.saturating_div_floor(&a).into_inner(), 0);

		assert_eq!(
			d.saturating_div_floor(&e),
			FixedU128::<DECIMALS_27>::max_value()
		);

		assert_eq!(
			one.saturating_div_floor(&a).into_inner(),
			333333333333333333333333333
		);
		assert_eq!(
			one.saturating_div_floor(&b).into_inner(),
			166666666666666666666666666
		);

		assert_eq!(
			d.saturating_div_floor(&c).into_inner(),
			555555555555555555555555555
		);
	}

	#[test]
	#[should_panic(expected = "attempt to divide by zero")]
	fn saturating_div_ceil_panics_on_zero_divizor() {
		let _ = FixedU128::<DECIMALS_27>::saturating_from_integer(6)
			.saturating_div_ceil(&FixedU128::<DECIMALS_27>::zero());
	}

	#[test]
	fn saturating_div_ceil_works() {
		let one = FixedU128::<DECIMALS_27>::one();
		let zero = FixedU128::<DECIMALS_27>::zero();
		let a = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let b = FixedU128::<DECIMALS_27>::saturating_from_integer(6);

		let c = FixedU128::<DECIMALS_27>::saturating_from_integer(9);
		let d = FixedU128::<DECIMALS_27>::saturating_from_integer(5);

		let e = FixedU128::<DECIMALS_27>::saturating_from_rational(
			1,
			FixedU128::<DECIMALS_27>::accuracy(),
		);

		assert_eq!(zero.saturating_div_ceil(&a).into_inner(), 0);

		assert_eq!(
			d.saturating_div_ceil(&e),
			FixedU128::<DECIMALS_27>::max_value()
		);

		assert_eq!(
			one.saturating_div_ceil(&a).into_inner(),
			333333333333333333333333334
		);
		assert_eq!(
			one.saturating_div_ceil(&b).into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			d.saturating_div_ceil(&c).into_inner(),
			555555555555555555555555556
		);
	}

	#[test]
	fn is_positive_negative_works() {
		let one = FixedU128::<DECIMALS_27>::one();
		assert!(one.is_positive());
		assert!(!one.is_negative());

		let zero = FixedU128::<DECIMALS_27>::zero();
		assert!(!zero.is_positive());
		assert!(!zero.is_negative());

		if false {
			let minus_one = FixedU128::<DECIMALS_27>::saturating_from_integer(-1);
			assert!(minus_one.is_negative());
			assert!(!minus_one.is_positive());
		}
	}

	#[test]
	fn trunc_works() {
		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2).trunc();
		assert_eq!(n, FixedU128::<DECIMALS_27>::saturating_from_integer(2));

		if FixedU128::<DECIMALS_27>::SIGNED {
			let n = FixedU128::<DECIMALS_27>::saturating_from_rational(-5, 2).trunc();
			assert_eq!(n, FixedU128::<DECIMALS_27>::saturating_from_integer(-2));
		}
	}

	#[test]
	fn frac_works() {
		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2);
		let i = n.trunc();
		let f = n.frac();

		assert_eq!(n, i + f);

		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2)
			.frac()
			.saturating_mul(10.into());
		assert_eq!(n, 5.into());

		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2)
			.frac()
			.saturating_mul(10.into());
		assert_eq!(n, 5.into());

		if FixedU128::<DECIMALS_27>::SIGNED {
			let n = FixedU128::<DECIMALS_27>::saturating_from_rational(-5, 2);
			let i = n.trunc();
			let f = n.frac();
			assert_eq!(n, i - f);

			// The sign is attached to the integer part unless it is zero.
			let n = FixedU128::<DECIMALS_27>::saturating_from_rational(-5, 2)
				.frac()
				.saturating_mul(10.into());
			assert_eq!(n, 5.into());

			let n = FixedU128::<DECIMALS_27>::saturating_from_rational(-1, 2)
				.frac()
				.saturating_mul(10.into());
			assert_eq!(n, 0.saturating_sub(5).into());
		}
	}

	#[test]
	fn ceil_works() {
		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2);
		assert_eq!(n.ceil(), 3.into());

		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(-5, 2);
		assert_eq!(n.ceil(), 0.saturating_sub(2).into());

		// On the limits:
		let n = FixedU128::<DECIMALS_27>::max_value();
		assert_eq!(n.ceil(), n.trunc());

		let n = FixedU128::<DECIMALS_27>::min_value();
		assert_eq!(n.ceil(), n.trunc());
	}

	#[test]
	fn floor_works() {
		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2);
		assert_eq!(n.floor(), 2.into());

		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(-5, 2);
		assert_eq!(n.floor(), 0.saturating_sub(3).into());

		// On the limits:
		let n = FixedU128::<DECIMALS_27>::max_value();
		assert_eq!(n.floor(), n.trunc());

		let n = FixedU128::<DECIMALS_27>::min_value();
		assert_eq!(n.floor(), n.trunc());
	}

	#[test]
	fn round_works() {
		let n = FixedU128::<DECIMALS_27>::zero();
		assert_eq!(n.round(), n);

		let n = FixedU128::<DECIMALS_27>::one();
		assert_eq!(n.round(), n);

		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2);
		assert_eq!(n.round(), 3.into());

		let n = FixedU128::<DECIMALS_27>::saturating_from_rational(-5, 2);
		assert_eq!(n.round(), 0.saturating_sub(3).into());

		// Saturating:
		let n = FixedU128::<DECIMALS_27>::max_value();
		assert_eq!(n.round(), n.trunc());

		let n = FixedU128::<DECIMALS_27>::min_value();
		assert_eq!(n.round(), n.trunc());

		// On the limit:

		// floor(max - 1) + 0.33..
		let n = FixedU128::<DECIMALS_27>::max_value()
			.saturating_sub(1.into())
			.trunc()
			.saturating_add((1, 3).into());

		assert_eq!(
			n.round(),
			(FixedU128::<DECIMALS_27>::max_value() - 1.into()).trunc()
		);

		// floor(max - 1) + 0.5
		let n = FixedU128::<DECIMALS_27>::max_value()
			.saturating_sub(1.into())
			.trunc()
			.saturating_add((1, 2).into());

		assert_eq!(n.round(), FixedU128::<DECIMALS_27>::max_value().trunc());

		if FixedU128::<DECIMALS_27>::SIGNED {
			// floor(min + 1) - 0.33..
			let n = FixedU128::<DECIMALS_27>::min_value()
				.saturating_add(1.into())
				.trunc()
				.saturating_sub((1, 3).into());

			assert_eq!(
				n.round(),
				(FixedU128::<DECIMALS_27>::min_value() + 1.into()).trunc()
			);

			// floor(min + 1) - 0.5
			let n = FixedU128::<DECIMALS_27>::min_value()
				.saturating_add(1.into())
				.trunc()
				.saturating_sub((1, 2).into());

			assert_eq!(n.round(), FixedU128::<DECIMALS_27>::min_value().trunc());
		}
	}

	#[test]
	fn reciprocal_with_rounding_works() {
		let zero = FixedU128::<DECIMALS_27>::zero();
		let one = FixedU128::<DECIMALS_27>::one();
		let three = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let six = FixedU128::<DECIMALS_27>::saturating_from_integer(6);
		let pref_precision_check_val = FixedU128::<DECIMALS_27>::saturating_from_integer(222220);

		assert_eq!(zero.reciprocal_with_rounding(SignedRounding::Minor), None);

		assert_eq!(zero.reciprocal_with_rounding(SignedRounding::Major), None);

		assert_eq!(
			one.reciprocal_with_rounding(SignedRounding::Major),
			Some(FixedU128::<DECIMALS_27>::one())
		);

		assert_eq!(
			one.reciprocal_with_rounding(SignedRounding::Minor),
			Some(FixedU128::<DECIMALS_27>::one())
		);

		assert_eq!(
			three
				.reciprocal_with_rounding(SignedRounding::Minor)
				.unwrap()
				.into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			three
				.reciprocal_with_rounding(SignedRounding::Major)
				.unwrap()
				.into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			three
				.reciprocal_with_rounding(SignedRounding::NearestPrefLow)
				.unwrap()
				.into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			three.reciprocal_with_rounding(SignedRounding::Minor),
			FixedU128::<DECIMALS_27>::checked_from_rational_floor(1, 3),
		);

		assert_eq!(
			six.reciprocal_with_rounding(SignedRounding::Minor)
				.unwrap()
				.into_inner(),
			166666666666666666666666666
		);

		assert_eq!(
			six.reciprocal_with_rounding(SignedRounding::Major)
				.unwrap()
				.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			six.reciprocal_with_rounding(SignedRounding::NearestPrefLow)
				.unwrap()
				.into_inner(),
			166666666666666666666666667
		);

		assert_eq!(
			pref_precision_check_val
				.reciprocal_with_rounding(SignedRounding::Minor)
				.unwrap()
				.into_inner(),
			4500045000450004500045
		);

		assert_eq!(
			pref_precision_check_val
				.reciprocal_with_rounding(SignedRounding::Major)
				.unwrap()
				.into_inner(),
			4500045000450004500046
		);

		assert_eq!(
			pref_precision_check_val
				.reciprocal_with_rounding(SignedRounding::NearestPrefLow)
				.unwrap()
				.into_inner(),
			4500045000450004500045
		);
	}

	#[test]
	fn reciprocal_floor_works() {
		let zero = FixedU128::<DECIMALS_27>::zero();
		let one = FixedU128::<DECIMALS_27>::one();
		let three = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let six = FixedU128::<DECIMALS_27>::saturating_from_integer(6);
		let pref_precision_check_val = FixedU128::<DECIMALS_27>::saturating_from_integer(222220);

		assert_eq!(zero.reciprocal_floor(), None);

		assert_eq!(
			one.reciprocal_floor(),
			Some(FixedU128::<DECIMALS_27>::one())
		);

		assert_eq!(
			three.reciprocal_floor().unwrap().into_inner(),
			333333333333333333333333333
		);

		assert_eq!(
			three.reciprocal_floor(),
			FixedU128::<DECIMALS_27>::checked_from_rational_floor(1, 3),
		);

		assert_eq!(
			six.reciprocal_floor().unwrap().into_inner(),
			166666666666666666666666666
		);
		assert_eq!(
			pref_precision_check_val
				.reciprocal_floor()
				.unwrap()
				.into_inner(),
			4500045000450004500045
		);
	}

	#[test]
	fn reciprocal_ceil_works() {
		let zero = FixedU128::<DECIMALS_27>::zero();
		let one = FixedU128::<DECIMALS_27>::one();
		let three = FixedU128::<DECIMALS_27>::saturating_from_integer(3);
		let six = FixedU128::<DECIMALS_27>::saturating_from_integer(6);
		let pref_precision_check_val = FixedU128::<DECIMALS_27>::saturating_from_integer(222220);

		assert_eq!(zero.reciprocal_ceil(), None);

		assert_eq!(one.reciprocal_ceil(), Some(FixedU128::<DECIMALS_27>::one()));

		assert_eq!(
			three.reciprocal_ceil().unwrap().into_inner(),
			333333333333333333333333334
		);

		assert_eq!(
			three.reciprocal_ceil(),
			FixedU128::<DECIMALS_27>::checked_from_rational_ceil(1, 3),
		);

		assert_eq!(
			six.reciprocal_ceil().unwrap().into_inner(),
			166666666666666666666666667
		);
		assert_eq!(
			pref_precision_check_val
				.reciprocal_ceil()
				.unwrap()
				.into_inner(),
			4500045000450004500046
		);
	}

	#[test]
	fn fmt_should_work() {
		let zero = FixedU128::<DECIMALS_27>::zero();
		assert_eq!(
			format!("{:?}", zero),
			format!(
				"{}(0.{:0>weight$})",
				stringify!(FixedU128),
				0,
				weight = precision()
			)
		);

		let one = FixedU128::<DECIMALS_27>::one();
		assert_eq!(
			format!("{:?}", one),
			format!(
				"{}(1.{:0>weight$})",
				stringify!(FixedU128),
				0,
				weight = precision()
			)
		);

		let frac = FixedU128::<DECIMALS_27>::saturating_from_rational(1, 2);
		assert_eq!(
			format!("{:?}", frac),
			format!(
				"{}(0.{:0<weight$})",
				stringify!(FixedU128),
				5,
				weight = precision()
			)
		);

		let frac = FixedU128::<DECIMALS_27>::saturating_from_rational(5, 2);
		assert_eq!(
			format!("{:?}", frac),
			format!(
				"{}(2.{:0<weight$})",
				stringify!(FixedU128),
				5,
				weight = precision()
			)
		);

		let frac = FixedU128::<DECIMALS_27>::saturating_from_rational(314, 100);
		assert_eq!(
			format!("{:?}", frac),
			format!(
				"{}(3.{:0<weight$})",
				stringify!(FixedU128),
				14,
				weight = precision()
			)
		);

		if FixedU128::<DECIMALS_27>::SIGNED {
			let neg = -FixedU128::<DECIMALS_27>::one();
			assert_eq!(
				format!("{:?}", neg),
				format!(
					"{}(-1.{:0>weight$})",
					stringify!(FixedU128),
					0,
					weight = precision()
				)
			);

			let frac = FixedU128::<DECIMALS_27>::saturating_from_rational(-314, 100);
			assert_eq!(
				format!("{:?}", frac),
				format!(
					"{}(-3.{:0<weight$})",
					stringify!(FixedU128),
					14,
					weight = precision()
				)
			);
		}
	}
}
