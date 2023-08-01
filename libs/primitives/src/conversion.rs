use sp_arithmetic::{
	traits::{ensure_pow, BaseArithmetic},
	ArithmeticError, FixedPointNumber,
};

pub fn fixed_point_to_balance<
	FixedPoint: FixedPointNumber<Inner = IntoBalance>,
	IntoBalance: BaseArithmetic + Copy,
>(
	fixed_point: FixedPoint,
	decimals: usize,
) -> Result<IntoBalance, ArithmeticError> {
	let integer_part = fixed_point.into_inner().ensure_div(FixedPoint::DIV)?;
	let frac_part = fixed_point.into_inner().ensure_sub(integer_part)?;

	let magnitude = ensure_pow(IntoBalance::from(10), decimals)?;

	integer_part
		.ensure_mul(magnitude)?
		.ensure_add(frac_part.ensure_div(FixedPoint::DIV.ensure_sub(magnitude)?)?)
}

#[cfg(test)]
mod tests {
	#[test]
	fn fixed_with_less_decimals() {
		//TODO
	}

	#[test]
	fn fixed_with_same_decimals() {
		//TODO
	}

	#[test]
	fn fixed_with_more_decimals() {
		//TODO
	}
}
