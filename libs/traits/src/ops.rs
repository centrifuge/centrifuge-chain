use sp_runtime::traits::Zero;

/// Numerical Sign
#[derive(Clone, Copy, PartialEq)]
pub enum NumSign {
	/// A negative value
	Negative,

	/// A positive/zero value
	Positive,
}

/// Request the signum of a number.
pub trait Signum: PartialOrd + Zero + Copy {
	/// Get the signum.
	fn signum(&self) -> NumSign {
		if *self < Self::zero() {
			NumSign::Negative
		} else {
			NumSign::Positive
		}
	}
}

impl<T: PartialOrd + Zero + Copy> Signum for T {}

/// Arithmetic operations with safe error handling.
///
/// This module provide a more readable way to do safe arithmetics, turning this:
///
/// ```ignore
/// self.my_value = self.my_value.checked_sub(other_value).ok_or(ArithmeticError::Overflow)?;
/// ```
///
/// into this:
///
/// ```ignore
/// self.my_value.ensure_sub_assign(other_value)?;
/// ```
///
/// And choose the correct [`ArithmeticError`] it should return in case of fail.
///
/// The *EnsureOps* family functions follows the same behavior as *CheckedOps* but
/// returning an [`ArithmeticError`] instead of `None`.
///
/// [`ArithmeticError`]: sp_runtime::ArithmeticError
pub mod ensure {
	use sp_runtime::{
		traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub},
		ArithmeticError, FixedPointNumber, FixedPointOperand,
	};

	use super::{NumSign, Signum};

	/// Performs addition that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureAdd: CheckedAdd + Signum {
		/// Adds two numbers, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// Similar to [`CheckedAdd::checked_add()`] but returning an `ArithmeticError` error
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureAdd;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     u32::MAX.ensure_add(1)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     i32::MIN.ensure_add(-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_add(self, v: Self) -> Result<Self, ArithmeticError> {
			self.checked_add(&v).ok_or_else(|| error::equivalent(v))
		}
	}

	/// Performs subtraction that returns `ArithmeticError` instead of wrapping around on underflow.
	pub trait EnsureSub: CheckedSub + Signum {
		/// Subtracts two numbers, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// Similar to [`CheckedSub::checked_sub()`] but returning an `ArithmeticError` error
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureSub;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     0u32.ensure_sub(1)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     i32::MAX.ensure_sub(-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// ```
		fn ensure_sub(self, v: Self) -> Result<Self, ArithmeticError> {
			self.checked_sub(&v).ok_or_else(|| error::inverse(v))
		}
	}

	/// Performs multiplication that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureMul: CheckedMul + Signum {
		/// Multiplies two numbers, checking for overflow. If overflow happens,
		/// `ArithmeticError` is returned.
		///
		/// Similar to [`CheckedMul::checked_mul()`] but returning an `ArithmeticError` error
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureMul;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     u32::MAX.ensure_mul(2)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     i32::MAX.ensure_mul(-2)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_mul(self, v: Self) -> Result<Self, ArithmeticError> {
			self.checked_mul(&v)
				.ok_or_else(|| error::multiplication(self, v))
		}
	}

	/// Performs division that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureDiv: CheckedDiv + Signum {
		/// Divides two numbers, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// Similar to [`CheckedDiv::checked_div()`] but returning an `ArithmeticError` error
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureDiv;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError, FixedI64};
		///
		/// fn extrinsic_zero() -> DispatchResult {
		///     1.ensure_div(0)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     FixedI64::from(i64::MIN).ensure_div(FixedI64::from(-1))?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_zero(), Err(ArithmeticError::DivisionByZero.into()));
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// ```
		fn ensure_div(self, v: Self) -> Result<Self, ArithmeticError> {
			self.checked_div(&v).ok_or_else(|| error::division(self, v))
		}
	}

	impl<T: CheckedAdd + Signum> EnsureAdd for T {}
	impl<T: CheckedSub + Signum> EnsureSub for T {}
	impl<T: CheckedMul + Signum> EnsureMul for T {}
	impl<T: CheckedDiv + Signum> EnsureDiv for T {}

	/// Performs self addition that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureAddAssign: EnsureAdd {
		/// Adds two numbers overwriting the left hand one, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureAddAssign;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let mut max = u32::MAX;
		///     max.ensure_add_assign(1)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     let mut max = i32::MIN;
		///     max.ensure_add_assign(-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_add_assign(&mut self, v: Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_add(v)?;
			Ok(())
		}
	}

	/// Performs self subtraction that returns `ArithmeticError` instead of wrapping around on underflow.
	pub trait EnsureSubAssign: EnsureSub {
		/// Subtracts two numbers overwriting the left hand one, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureSubAssign;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     let mut zero: u32 = 0;
		///     zero.ensure_sub_assign(1)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let mut zero = i32::MAX;
		///     zero.ensure_sub_assign(-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// ```
		fn ensure_sub_assign(&mut self, v: Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_sub(v)?;
			Ok(())
		}
	}

	/// Performs self multiplication that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureMulAssign: EnsureMul {
		/// Multiplies two numbers overwriting the left hand one, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureMulAssign;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let mut max = u32::MAX;
		///     max.ensure_mul_assign(2)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     let mut max = i32::MAX;
		///     max.ensure_mul_assign(-2)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_mul_assign(&mut self, v: Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_mul(v)?;
			Ok(())
		}
	}

	/// Performs self division that returns `ArithmeticError` instead of wrapping around on overflow.
	pub trait EnsureDivAssign: EnsureDiv {
		/// Divides two numbers overwriting the left hand one, checking for overflow.
		/// If overflow happens, `ArithmeticError` is returned.
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureDivAssign;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError, FixedI64};
		///
		/// fn extrinsic_zero() -> DispatchResult {
		///     let mut one = 1;
		///     one.ensure_div_assign(0)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let mut min = FixedI64::from(i64::MIN);
		///     min.ensure_div_assign(FixedI64::from(-1))?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_zero(), Err(ArithmeticError::DivisionByZero.into()));
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// ```
		fn ensure_div_assign(&mut self, v: Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_div(v)?;
			Ok(())
		}
	}

	impl<T: EnsureAdd> EnsureAddAssign for T {}
	impl<T: EnsureSub> EnsureSubAssign for T {}
	impl<T: EnsureMul> EnsureMulAssign for T {}
	impl<T: EnsureDiv> EnsureDivAssign for T {}

	/// Extends `FixedPointNumber with` the Ensure family functions.
	pub trait EnsureFixedPointNumber: FixedPointNumber {
		/// Creates `self` from a rational number. Equal to `n / d`.
		///
		/// Returns `ArithmeticError` if `d == 0` or `n / d` exceeds accuracy.
		///
		/// Similar to [`FixedPointNumber::checked_from_rational()`] but returning an `ArithmeticError` error
		/// ```
		/// use cfg_traits::ops::ensure::EnsureFixedPointNumber;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError, FixedI64};
		///
		/// fn extrinsic_zero() -> DispatchResult {
		///     FixedI64::ensure_from_rational(1, 0)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     FixedI64::ensure_from_rational(i64::MAX, -1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_zero(), Err(ArithmeticError::DivisionByZero.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_from_rational<N: FixedPointOperand, D: FixedPointOperand>(
			n: N,
			d: D,
		) -> Result<Self, ArithmeticError> {
			<Self as FixedPointNumber>::checked_from_rational(n, d)
				.ok_or_else(|| error::division(n, d))
		}

		/// Ensure multiplication for integer type `N`. Equal to `self * n`.
		///
		/// Returns `ArithmeticError` if the result does not fit in `N`.
		///
		/// Similar to [`FixedPointNumber::checked_mul_int()`] but returning an ArithmeticError error
		/// ```
		/// use cfg_traits::ops::ensure::EnsureFixedPointNumber;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError, FixedI64};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     FixedI64::from(i64::MAX).ensure_mul_int(2)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     FixedI64::from(i64::MAX).ensure_mul_int(-2)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_mul_int<N: FixedPointOperand>(self, n: N) -> Result<N, ArithmeticError> {
			self.checked_mul_int(n)
				.ok_or_else(|| error::multiplication(self, n))
		}

		/// Ensure division for integer type `N`. Equal to `self / d`.
		///
		/// Returns `ArithmeticError` if the result does not fit in `N` or `d == 0`.
		///
		/// Similar to [`FixedPointNumber::checked_div_int()`] but returning an `ArithmeticError` error
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureFixedPointNumber;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError, FixedI64};
		///
		/// fn extrinsic_zero() -> DispatchResult {
		///     FixedI64::from(1).ensure_div_int(0)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     FixedI64::from(i64::MIN).ensure_div_int(-1)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_zero(), Err(ArithmeticError::DivisionByZero.into()));
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// ```
		fn ensure_div_int<D: FixedPointOperand>(self, d: D) -> Result<D, ArithmeticError> {
			self.checked_div_int(d)
				.ok_or_else(|| error::division(self, d))
		}
	}

	impl<T: FixedPointNumber> EnsureFixedPointNumber for T {}

	/// Similar to [`TryFrom`] but returning an `ArithmeticError` error.
	pub trait EnsureFrom<T: Signum>: TryFrom<T> + Signum {
		/// Performs the conversion returning an `ArithmeticError` if fails.
		///
		/// Similar to [`TryFrom::try_from()`] but returning an `ArithmeticError` error
		/// ```
		/// use cfg_traits::ops::ensure::EnsureFrom;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let byte: u8 = u8::ensure_from(256u16)?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     let byte: i8 = i8::ensure_from(-129i16)?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_from(other: T) -> Result<Self, ArithmeticError> {
			Self::try_from(other).map_err(|_| error::equivalent(other))
		}
	}

	/// Similar to [`TryInto`] but returning an `ArithmeticError` error.
	pub trait EnsureInto<T: Signum>: TryInto<T> + Signum {
		/// Performs the conversion returning an `ArithmeticError` if fails.
		///
		/// Similar to [`TryInto::try_into()`] but returning an `ArithmeticError` error
		///
		/// ```
		/// use cfg_traits::ops::ensure::EnsureInto;
		/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
		///
		/// fn extrinsic_overflow() -> DispatchResult {
		///     let byte: u8 = 256u16.ensure_into()?;
		///     Ok(())
		/// }
		///
		/// fn extrinsic_underflow() -> DispatchResult {
		///     let byte: i8 = (-129i16).ensure_into()?;
		///     Ok(())
		/// }
		///
		/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
		/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
		/// ```
		fn ensure_into(self) -> Result<T, ArithmeticError> {
			self.try_into().map_err(|_| error::equivalent(self))
		}
	}

	impl<T: TryFrom<S> + Signum, S: Signum> EnsureFrom<S> for T {}
	impl<T: TryInto<S> + Signum, S: Signum> EnsureInto<S> for T {}

	mod error {
		use super::{ArithmeticError, NumSign, Signum};

		pub fn equivalent<R: Signum>(r: R) -> ArithmeticError {
			match r.signum() {
				NumSign::Negative => ArithmeticError::Underflow,
				NumSign::Positive => ArithmeticError::Overflow,
			}
		}

		pub fn inverse<R: Signum>(r: R) -> ArithmeticError {
			match r.signum() {
				NumSign::Negative => ArithmeticError::Overflow,
				NumSign::Positive => ArithmeticError::Underflow,
			}
		}

		pub fn multiplication<L: Signum, R: Signum>(l: L, r: R) -> ArithmeticError {
			match l.signum() != r.signum() {
				true => ArithmeticError::Underflow,
				false => ArithmeticError::Overflow,
			}
		}

		pub fn division<N: Signum, D: Signum>(n: N, d: D) -> ArithmeticError {
			if d.is_zero() {
				ArithmeticError::DivisionByZero
			} else {
				multiplication(n, d)
			}
		}
	}
}
