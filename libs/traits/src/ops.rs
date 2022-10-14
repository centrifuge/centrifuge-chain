use sp_runtime::{
	traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero},
	ArithmeticError,
};

// Numerical Sign
#[derive(Clone, Copy, PartialEq)]
pub enum NumSign {
	// A negative value
	Negative,

	// A positive/zero value
	Positive,
}

/// Request the signum of a value.
pub trait Signum: PartialOrd + Zero {
	/// Get the signum.
	fn signum(&self) -> NumSign {
		if *self < Self::zero() {
			NumSign::Negative
		} else {
			NumSign::Positive
		}
	}
}

impl<T: PartialOrd + Zero> Signum for T {}

/// Performs addition that returns `ArithmeticError` instead of wrapping around on overflow.
pub trait EnsureAdd: CheckedAdd + Signum {
	/// Adds two numbers, checking for overflow.
	/// If overflow happens, `ArithmeticError` is returned.
	///
	/// ```
	/// use cfg_traits::ops::EnsureAdd;
	/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
	///
	/// fn extrinsic_overflow() -> DispatchResult {
	///     u32::MAX.ensure_add(&1)?;
	///     Ok(())
	/// }
	///
	/// fn extrinsic_underflow() -> DispatchResult {
	///     i32::MIN.ensure_add(&-1)?;
	///     Ok(())
	/// }
	///
	/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
	/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
	/// ```
	fn ensure_add(&self, v: &Self) -> Result<Self, ArithmeticError> {
		self.checked_add(v).ok_or_else(|| match v.signum() {
			NumSign::Negative => ArithmeticError::Underflow,
			NumSign::Positive => ArithmeticError::Overflow,
		})
	}
}

/// Performs subtraction that returns `ArithmeticError` instead of wrapping around on underflow.
pub trait EnsureSub: CheckedSub + Signum {
	/// Subtracts two numbers, checking for overflow.
	/// If overflow happens, `ArithmeticError` is returned.
	///
	/// ```
	/// use cfg_traits::ops::EnsureSub;
	/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
	///
	/// fn extrinsic_underflow() -> DispatchResult {
	///     0u32.ensure_sub(&1)?;
	///     Ok(())
	/// }
	///
	/// fn extrinsic_overflow() -> DispatchResult {
	///     i32::MAX.ensure_sub(&-1)?;
	///     Ok(())
	/// }
	///
	/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
	/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
	/// ```
	fn ensure_sub(&self, v: &Self) -> Result<Self, ArithmeticError> {
		self.checked_sub(v).ok_or_else(|| match v.signum() {
			NumSign::Negative => ArithmeticError::Overflow,
			NumSign::Positive => ArithmeticError::Underflow,
		})
	}
}

/// Performs multiplication that returns `ArithmeticError` instead of wrapping around on overflow.
pub trait EnsureMul: CheckedMul + Signum {
	/// Multiplies two numbers, checking for overflow. If overflow happens,
	/// `ArithmeticError` is returned.
	///
	/// ```
	/// use cfg_traits::ops::EnsureMul;
	/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
	///
	/// fn extrinsic_overflow() -> DispatchResult {
	///     u32::MAX.ensure_mul(&2)?;
	///     Ok(())
	/// }
	///
	/// fn extrinsic_underflow() -> DispatchResult {
	///     i32::MAX.ensure_mul(&-2)?;
	///     Ok(())
	/// }
	///
	/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
	/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
	/// ```
	fn ensure_mul(&self, v: &Self) -> Result<Self, ArithmeticError> {
		self.checked_mul(v)
			.ok_or_else(|| match self.signum() != v.signum() {
				true => ArithmeticError::Underflow,
				false => ArithmeticError::Overflow,
			})
	}
}

/// Performs division that returns `ArithmeticError` instead of wrapping around on overflow.
pub trait EnsureDiv: CheckedDiv {
	/// Divides two numbers, checking for overflow.
	/// If overflow happens, `ArithmeticError` is returned.
	///
	/// ```
	/// use cfg_traits::ops::EnsureDiv;
	/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
	///
	/// fn extrinsic() -> DispatchResult {
	///     1.ensure_div(&0)?;
	///     Ok(())
	/// }
	///
	/// assert_eq!(extrinsic(), Err(ArithmeticError::DivisionByZero.into()));
	/// ```
	fn ensure_div(&self, v: &Self) -> Result<Self, ArithmeticError> {
		self.checked_div(v).ok_or(ArithmeticError::DivisionByZero)
	}
}

impl<T: CheckedAdd + Signum> EnsureAdd for T {}
impl<T: CheckedSub + Signum> EnsureSub for T {}
impl<T: CheckedMul + Signum> EnsureMul for T {}
impl<T: CheckedDiv> EnsureDiv for T {}

/// Performs self addition that returns `ArithmeticError` instead of wrapping around on overflow.
pub trait EnsureAddAssign: EnsureAdd {
	/// Adds two numbers overwriting the left hand one, checking for overflow.
	/// If overflow happens, `ArithmeticError` is returned.
	///
	/// ```
	/// use cfg_traits::ops::EnsureAddAssign;
	/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
	///
	/// fn extrinsic_overflow() -> DispatchResult {
	///     let mut max = u32::MAX;
	///     max.ensure_add_assign(&1)?;
	///     Ok(())
	/// }
	///
	/// fn extrinsic_underflow() -> DispatchResult {
	///     let mut max = i32::MIN;
	///     max.ensure_add_assign(&-1)?;
	///     Ok(())
	/// }
	///
	/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
	/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
	/// ```
	fn ensure_add_assign(&mut self, v: &Self) -> Result<(), ArithmeticError> {
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
	/// use cfg_traits::ops::EnsureSubAssign;
	/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
	///
	/// fn extrinsic_underflow() -> DispatchResult {
	///     let mut zero: u32 = 0;
	///     zero.ensure_sub_assign(&1)?;
	///     Ok(())
	/// }
	///
	/// fn extrinsic_overflow() -> DispatchResult {
	///     let mut zero = i32::MAX;
	///     zero.ensure_sub_assign(&-1)?;
	///     Ok(())
	/// }
	///
	/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
	/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
	/// ```
	fn ensure_sub_assign(&mut self, v: &Self) -> Result<(), ArithmeticError> {
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
	/// use cfg_traits::ops::EnsureMulAssign;
	/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
	///
	/// fn extrinsic_overflow() -> DispatchResult {
	///     let mut max = u32::MAX;
	///     max.ensure_mul_assign(&2)?;
	///     Ok(())
	/// }
	///
	/// fn extrinsic_underflow() -> DispatchResult {
	///     let mut max = i32::MAX;
	///     max.ensure_mul_assign(&-2)?;
	///     Ok(())
	/// }
	///
	/// assert_eq!(extrinsic_overflow(), Err(ArithmeticError::Overflow.into()));
	/// assert_eq!(extrinsic_underflow(), Err(ArithmeticError::Underflow.into()));
	/// ```
	fn ensure_mul_assign(&mut self, v: &Self) -> Result<(), ArithmeticError> {
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
	/// use cfg_traits::ops::EnsureDivAssign;
	/// use sp_runtime::{DispatchResult, ArithmeticError, DispatchError};
	///
	/// fn extrinsic() -> DispatchResult {
	///     let mut one = 1;
	///     one.ensure_div_assign(&0)?;
	///     Ok(())
	/// }
	///
	/// assert_eq!(extrinsic(), Err(ArithmeticError::DivisionByZero.into()));
	/// ```
	fn ensure_div_assign(&mut self, v: &Self) -> Result<(), ArithmeticError> {
		*self = self.ensure_div(v)?;
		Ok(())
	}
}

impl<T: EnsureAdd> EnsureAddAssign for T {}
impl<T: EnsureSub> EnsureSubAssign for T {}
impl<T: EnsureMul> EnsureMulAssign for T {}
impl<T: EnsureDiv> EnsureDivAssign for T {}

#[cfg(test)]
mod test {
	use sp_runtime::{FixedU128, Perbill};

	use super::*;

	// Ensure the following substrate types are implemented automatically for the EnsureOps
	// family traits

	#[test]
	fn fixed_point_support() {
		assert_eq!(
			FixedU128::from(3).ensure_sub(&FixedU128::from(1)),
			Ok(FixedU128::from(2))
		);
		assert_eq!(
			FixedU128::from(0).ensure_sub(&FixedU128::from(1)),
			Err(ArithmeticError::Underflow.into())
		);
	}

	#[test]
	fn per_thing_support() {
		assert_eq!(
			Perbill::from_percent(3).ensure_sub(&Perbill::from_percent(1)),
			Ok(Perbill::from_percent(2))
		);
		assert_eq!(
			Perbill::from_percent(0).ensure_sub(&Perbill::from_percent(1)),
			Err(ArithmeticError::Underflow.into())
		);
	}
}
