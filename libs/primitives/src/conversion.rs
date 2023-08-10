use sp_arithmetic::{
	traits::{ensure_pow, BaseArithmetic},
	ArithmeticError, FixedPointNumber,
};

/// Transform a fixed point number to a Balance.
/// The resulting Balance will be represented with the `decimals` given.
///
/// i.e:
/// ```rust
/// use cfg_primitives::conversion::fixed_point_to_balance;
/// # use frame_support::assert_ok;
/// # use sp_arithmetic::fixed_point::FixedU64;
///
/// assert_ok!(fixed_point_to_balance(FixedU64::from_float(23.1234567890), 0), 23);
/// assert_ok!(fixed_point_to_balance(FixedU64::from_float(23.1234567890), 3), 23_123);
/// assert_ok!(fixed_point_to_balance(FixedU64::from_float(23.1234567890), 6), 23_123_456);
/// assert_ok!(fixed_point_to_balance(FixedU64::from_float(23.1234567890), 9), 23_123_456_789);
/// assert_ok!(fixed_point_to_balance(FixedU64::from_float(23.1234567890), 12), 23_123_456_789_000);
/// assert_ok!(fixed_point_to_balance(FixedU64::from_float(23.1234567890), 15), 23_123_456_789_000_000);
/// ```
///
/// ```rust
/// use cfg_primitives::conversion::fixed_point_to_balance;
/// # use frame_support::assert_err;
/// # use sp_arithmetic::{fixed_point::FixedU64, ArithmeticError};
///
/// assert_err!(
///     // The integer part does not fit in a `u64` (FixedU64::Inner type)
///     fixed_point_to_balance(FixedU64::from_float(23.42), 18),
///     ArithmeticError::Overflow
/// );
/// ```
///
/// Maths:
/// ```text
/// int = (n / DIV)
/// frac = n - int * DIV
/// m = 10 ^ d
/// result = int * m + frac * m / DIV
/// ```
pub fn fixed_point_to_balance<
	FixedPoint: FixedPointNumber<Inner = IntoBalance>,
	IntoBalance: BaseArithmetic + Copy,
>(
	fixed_point: FixedPoint,
	decimals: usize,
) -> Result<IntoBalance, ArithmeticError> {
	let integer_part = fixed_point.into_inner().ensure_div(FixedPoint::DIV)?;
	let frac_part = fixed_point
		.into_inner()
		.ensure_sub(integer_part.ensure_mul(FixedPoint::DIV)?)?;

	let magnitude = ensure_pow(IntoBalance::from(10), decimals)?;

	let new_integer_part = integer_part.ensure_mul(magnitude)?;

	// Both if/else branches are mathematically equivalent, but we need to
	// distinguish each case to avoid intermediate overflow computations
	let new_frac_part = if magnitude > FixedPoint::DIV {
		frac_part.ensure_mul(magnitude.ensure_div(FixedPoint::DIV)?)?
	} else {
		frac_part
			.ensure_mul(magnitude)?
			.ensure_div(FixedPoint::DIV)?
	};

	new_integer_part.ensure_add(new_frac_part)
}
