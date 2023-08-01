use sp_arithmetic::{
	traits::{ensure_pow, BaseArithmetic},
	ArithmeticError, FixedPointNumber,
};

/// Transform a fixed point number to a Balance.
/// The resulting Balance will be represented with the `decimals` given.
pub fn fixed_point_to_balance<
	FixedPoint: FixedPointNumber<Inner = IntoBalance>,
	IntoBalance: BaseArithmetic + Copy + std::fmt::Debug,
>(
	fixed_point: FixedPoint,
	decimals: usize,
) -> Result<IntoBalance, ArithmeticError> {
	let integer_part = fixed_point.into_inner().ensure_div(FixedPoint::DIV)?;
	let frac_part = fixed_point
		.into_inner()
		.ensure_sub(integer_part * FixedPoint::DIV)?;

	let magnitude = ensure_pow(IntoBalance::from(10), decimals)?;

	let new_integer_part = integer_part.ensure_mul(magnitude)?;
	let new_frac_part = if magnitude > FixedPoint::DIV {
		frac_part.ensure_mul(magnitude.ensure_div(FixedPoint::DIV)?)?
	} else {
		frac_part
			.ensure_mul(magnitude)?
			.ensure_div(FixedPoint::DIV)?
	};

	new_integer_part.ensure_add(new_frac_part)
}

#[cfg(test)]
mod tests {
	use frame_support::{assert_err, assert_ok};
	use sp_arithmetic::fixed_point::FixedU64;

	use super::*;

	#[test]
	fn with_no_decimals() {
		assert_ok!(fixed_point_to_balance(FixedU64::from_float(23.42), 0), 23);
	}

	#[test]
	fn with_less_decimals_than_div() {
		assert_ok!(
			fixed_point_to_balance(FixedU64::from_float(23.42), 6),
			23_420_000
		);

		assert_ok!(fixed_point_to_balance(FixedU64::from_float(23.42), 0), 23);
	}

	#[test]
	fn with_same_decimals_as_div() {
		assert_ok!(
			fixed_point_to_balance(FixedU64::from_float(23.42), 9),
			23_420_000_000
		);
	}

	#[test]
	fn with_more_decimals_than_div() {
		assert_ok!(
			fixed_point_to_balance(FixedU64::from_float(23.42), 12),
			23_420_000_000_000
		);
	}

	#[test]
	fn with_max_decimals() {
		assert_ok!(
			fixed_point_to_balance(FixedU64::from_float(23.42), 17),
			2_342_000_000_000_000_000
		);
	}

	#[test]
	fn with_overflows() {
		assert_err!(
			// The integer part does not fit in a `u64`
			fixed_point_to_balance(FixedU64::from_float(23.42), 18),
			ArithmeticError::Overflow
		);
	}
}
