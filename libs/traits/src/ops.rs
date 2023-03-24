pub use ensure::{
	Ensure, EnsureAdd, EnsureAddAssign, EnsureDiv, EnsureDivAssign, EnsureFixedPointNumber,
	EnsureFrom, EnsureInto, EnsureMul, EnsureMulAssign, EnsureOp, EnsureOpAssign, EnsureSub,
	EnsureSubAssign,
};

pub mod storage {
	use sp_runtime::{traits::Get, BoundedVec};

	pub trait BoundedVecExt<T> {
		/// Same as [`BoundedVec::try_push()`] but returns a reference to the pushed element in the
		/// vector
		fn try_push_fetch(&mut self, element: T) -> Result<&mut T, T>;
	}

	impl<T, S: Get<u32>> BoundedVecExt<T> for BoundedVec<T, S> {
		fn try_push_fetch(&mut self, element: T) -> Result<&mut T, T> {
			let len = self.len();
			self.try_push(element)?;
			Ok(self.get_mut(len).expect("This can not fail. qed"))
		}
	}
}

mod ensure {
	use sp_runtime::{
		traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero},
		ArithmeticError, FixedPointNumber, FixedPointOperand,
	};

	/// Performs addition that returns [`ArithmeticError`] instead of wrapping around on overflow.
	pub trait EnsureAdd: CheckedAdd + PartialOrd + Zero + Copy {
		fn ensure_add(self, v: Self) -> Result<Self, ArithmeticError> {
			self.checked_add(&v).ok_or_else(|| error::equivalent(v))
		}
	}

	/// Performs subtraction that returns [`ArithmeticError`] instead of wrapping around on
	/// underflow.
	pub trait EnsureSub: CheckedSub + PartialOrd + Zero + Copy {
		fn ensure_sub(self, v: Self) -> Result<Self, ArithmeticError> {
			self.checked_sub(&v).ok_or_else(|| error::inverse(v))
		}
	}

	/// Performs multiplication that returns [`ArithmeticError`] instead of wrapping around on
	/// overflow.
	pub trait EnsureMul: CheckedMul + PartialOrd + Zero + Copy {
		fn ensure_mul(self, v: Self) -> Result<Self, ArithmeticError> {
			self.checked_mul(&v)
				.ok_or_else(|| error::multiplication(self, v))
		}
	}

	/// Performs division that returns [`ArithmeticError`] instead of wrapping around on overflow.
	pub trait EnsureDiv: CheckedDiv + PartialOrd + Zero + Copy {
		fn ensure_div(self, v: Self) -> Result<Self, ArithmeticError> {
			self.checked_div(&v).ok_or_else(|| error::division(self, v))
		}
	}

	impl<T: CheckedAdd + PartialOrd + Zero + Copy> EnsureAdd for T {}
	impl<T: CheckedSub + PartialOrd + Zero + Copy> EnsureSub for T {}
	impl<T: CheckedMul + PartialOrd + Zero + Copy> EnsureMul for T {}
	impl<T: CheckedDiv + PartialOrd + Zero + Copy> EnsureDiv for T {}

	/// Meta trait that supports all immutable arithmetic `Ensure*` operations
	pub trait EnsureOp: EnsureAdd + EnsureSub + EnsureMul + EnsureDiv {}
	impl<T: EnsureAdd + EnsureSub + EnsureMul + EnsureDiv> EnsureOp for T {}

	/// Performs self addition that returns [`ArithmeticError`] instead of wrapping around on
	/// overflow.
	pub trait EnsureAddAssign: EnsureAdd {
		fn ensure_add_assign(&mut self, v: Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_add(v)?;
			Ok(())
		}
	}

	/// Performs self subtraction that returns [`ArithmeticError`] instead of wrapping around on
	/// underflow.
	pub trait EnsureSubAssign: EnsureSub {
		fn ensure_sub_assign(&mut self, v: Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_sub(v)?;
			Ok(())
		}
	}

	/// Performs self multiplication that returns [`ArithmeticError`] instead of wrapping around on
	/// overflow.
	pub trait EnsureMulAssign: EnsureMul {
		fn ensure_mul_assign(&mut self, v: Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_mul(v)?;
			Ok(())
		}
	}

	/// Performs self division that returns [`ArithmeticError`] instead of wrapping around on
	/// overflow.
	pub trait EnsureDivAssign: EnsureDiv {
		fn ensure_div_assign(&mut self, v: Self) -> Result<(), ArithmeticError> {
			*self = self.ensure_div(v)?;
			Ok(())
		}
	}

	impl<T: EnsureAdd> EnsureAddAssign for T {}
	impl<T: EnsureSub> EnsureSubAssign for T {}
	impl<T: EnsureMul> EnsureMulAssign for T {}
	impl<T: EnsureDiv> EnsureDivAssign for T {}

	/// Meta trait that supports all assigned arithmetic `Ensure*` operations
	pub trait EnsureOpAssign:
		EnsureAddAssign + EnsureSubAssign + EnsureMulAssign + EnsureDivAssign
	{
	}
	impl<T: EnsureAddAssign + EnsureSubAssign + EnsureMulAssign + EnsureDivAssign> EnsureOpAssign
		for T
	{
	}

	/// Meta trait that supports all arithmetic operations
	pub trait Ensure: EnsureOp + EnsureOpAssign {}
	impl<T: EnsureOp + EnsureOpAssign> Ensure for T {}

	/// Extends [`FixedPointNumber`] with the Ensure family functions.
	pub trait EnsureFixedPointNumber: FixedPointNumber {
		fn ensure_from_rational<N: FixedPointOperand, D: FixedPointOperand>(
			n: N,
			d: D,
		) -> Result<Self, ArithmeticError> {
			<Self as FixedPointNumber>::checked_from_rational(n, d)
				.ok_or_else(|| error::division(n, d))
		}

		fn ensure_mul_int<N: FixedPointOperand>(self, n: N) -> Result<N, ArithmeticError> {
			self.checked_mul_int(n)
				.ok_or_else(|| error::multiplication(self, n))
		}

		fn ensure_div_int<D: FixedPointOperand>(self, d: D) -> Result<D, ArithmeticError> {
			self.checked_div_int(d)
				.ok_or_else(|| error::division(self, d))
		}
	}

	impl<T: FixedPointNumber> EnsureFixedPointNumber for T {}

	/// Similar to [`TryFrom`] but returning an [`ArithmeticError`] error.
	pub trait EnsureFrom<T: PartialOrd + Zero + Copy>:
		TryFrom<T> + PartialOrd + Zero + Copy
	{
		fn ensure_from(other: T) -> Result<Self, ArithmeticError> {
			Self::try_from(other).map_err(|_| error::equivalent(other))
		}
	}

	/// Similar to [`TryInto`] but returning an [`ArithmeticError`] error.
	pub trait EnsureInto<T: PartialOrd + Zero + Copy>:
		TryInto<T> + PartialOrd + Zero + Copy
	{
		fn ensure_into(self) -> Result<T, ArithmeticError> {
			self.try_into().map_err(|_| error::equivalent(self))
		}
	}

	impl<T: TryFrom<S> + PartialOrd + Zero + Copy, S: PartialOrd + Zero + Copy> EnsureFrom<S> for T {}
	impl<T: TryInto<S> + PartialOrd + Zero + Copy, S: PartialOrd + Zero + Copy> EnsureInto<S> for T {}

	mod error {
		use super::{ArithmeticError, Zero};

		#[derive(PartialEq)]
		enum Signum {
			Negative,
			Positive,
		}

		impl<T: PartialOrd + Zero> From<T> for Signum {
			fn from(value: T) -> Self {
				if value < Zero::zero() {
					Signum::Negative
				} else {
					Signum::Positive
				}
			}
		}

		impl sp_std::ops::Mul for Signum {
			type Output = Self;

			fn mul(self, rhs: Self) -> Self {
				if self != rhs {
					Signum::Negative
				} else {
					Signum::Positive
				}
			}
		}

		pub fn equivalent<R: PartialOrd + Zero + Copy>(r: R) -> ArithmeticError {
			match Signum::from(r) {
				Signum::Negative => ArithmeticError::Underflow,
				Signum::Positive => ArithmeticError::Overflow,
			}
		}

		pub fn inverse<R: PartialOrd + Zero + Copy>(r: R) -> ArithmeticError {
			match Signum::from(r) {
				Signum::Negative => ArithmeticError::Overflow,
				Signum::Positive => ArithmeticError::Underflow,
			}
		}

		pub fn multiplication<L: PartialOrd + Zero + Copy, R: PartialOrd + Zero + Copy>(
			l: L,
			r: R,
		) -> ArithmeticError {
			match Signum::from(l) * Signum::from(r) {
				Signum::Negative => ArithmeticError::Underflow,
				Signum::Positive => ArithmeticError::Overflow,
			}
		}

		pub fn division<N: PartialOrd + Zero + Copy, D: PartialOrd + Zero + Copy>(
			n: N,
			d: D,
		) -> ArithmeticError {
			if d.is_zero() {
				ArithmeticError::DivisionByZero
			} else {
				multiplication(n, d)
			}
		}
	}
}
