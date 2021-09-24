use crate::math::Adjustment::{Dec, Inc};
use sp_arithmetic::traits::checked_pow;
use sp_arithmetic::FixedPointNumber;

/// calculates the latest accumulated rate since the last
pub fn calculate_accumulated_rate<Rate: FixedPointNumber>(
	rate_per_sec: Rate,
	cumulative_rate: Rate,
	now: u64,
	last_updated: u64,
) -> Option<Rate> {
	let pow = now - last_updated;
	checked_pow(rate_per_sec, pow as usize).and_then(|v| v.checked_mul(&cumulative_rate))
}

/// converts a fixed point from A precision to B precision
/// we don't convert from un-signed to signed or vice-verse
fn convert<A: FixedPointNumber, B: FixedPointNumber>(a: A) -> Option<B> {
	if A::SIGNED != B::SIGNED {
		return None;
	}

	B::checked_from_rational(a.into_inner(), A::accuracy())
}

/// calculates the debt using debt=normalised_debt * cumulative_rate
pub fn debt<Amount: FixedPointNumber, Rate: FixedPointNumber>(
	normalised_debt: Amount,
	accumulated_rate: Rate,
) -> Option<Amount> {
	convert::<Rate, Amount>(accumulated_rate).and_then(|rate| normalised_debt.checked_mul(&rate))
}

pub enum Adjustment<Amount: FixedPointNumber> {
	Inc(Amount),
	Dec(Amount),
}

/// calculates the normalised debt after the adjustment
/// current_debt and cumulative_rate must be latest
pub fn calculate_normalised_debt<Amount: FixedPointNumber, Rate: FixedPointNumber>(
	current_debt: Amount,
	adjustment: Adjustment<Amount>,
	cumulative_rate: Rate,
) -> Option<Amount> {
	convert::<Rate, Amount>(cumulative_rate).and_then(|rate| {
		match adjustment {
			Inc(amount) => current_debt.checked_add(&amount),
			Dec(amount) => current_debt.checked_sub(&amount),
		}
		.and_then(|current_debt| current_debt.checked_div(&rate))
	})
}

/// returns the seconds in a given normal year(365 days)
/// https://docs.centrifuge.io/use/tinlake-financial-concepts/#interest-rate-methodology
fn seconds_per_year<T: FixedPointNumber>() -> T {
	T::saturating_from_integer(3600 * 24 * 365_u128)
}

/// calculates rate per second from the given nominal interest rate
/// https://docs.centrifuge.io/use/tinlake-financial-concepts/#interest-rate-methodology
pub fn rate_per_sec<Rate: FixedPointNumber>(nominal_interest_rate: Rate) -> Option<Rate> {
	nominal_interest_rate
		.checked_div(&seconds_per_year())
		.and_then(|res| res.checked_add(&Rate::one()))
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::sp_runtime::traits::CheckedMul;
	use sp_arithmetic::fixed_point::FixedU128P27;
	use sp_arithmetic::FixedI128;
	use sp_arithmetic::{FixedU128, Percent};

	#[test]
	fn test_convert() {
		// unsigned to signed should fail
		let a = FixedU128::checked_from_rational(1, 23).unwrap();
		assert!(
			convert::<FixedU128, FixedI128>(a).is_none(),
			"conversion should fail"
		);

		// signed to unsigned should fails
		let b = FixedI128::checked_from_rational(1, 23).unwrap();
		assert!(
			convert::<FixedI128, FixedU128>(b).is_none(),
			"conversion should fail"
		);

		// higher precision to lower
		let c = FixedU128P27::checked_from_rational(1, 23).unwrap();
		let conv = convert::<FixedU128P27, FixedU128>(c);
		assert!(conv.is_some(), "conversion should pass");
		assert_eq!(
			conv.unwrap(),
			FixedU128::checked_from_rational(1, 23).unwrap()
		);

		// lower precision to higher
		let c = FixedU128::checked_from_rational(1, 23).unwrap();
		let conv = convert::<FixedU128, FixedU128P27>(c);
		assert!(conv.is_some(), "conversion should pass");
		assert_eq!(
			conv.unwrap(),
			FixedU128P27::checked_from_rational(
				43478260869565217000000000u128,
				FixedU128P27::accuracy()
			)
			.unwrap()
		);
	}

	#[test]
	fn test_calculate_cumulative_rate() {
		// 5% interest rate
		let rate = FixedU128P27::from(Percent::from_percent(5));
		let rate_per_sec = rate_per_sec(rate).unwrap_or_default();
		assert!(rate_per_sec.is_positive(), "should not be zero");

		// initial cumulative_rate
		let cumulative_rate = FixedU128P27::from(1);

		// moment values
		let last_updated = 0u64;
		// after half a year
		let now = (3600 * 24 * 365) / 2;

		// calculate cumulative rate after half a year with compounding in seconds
		let maybe_new_cumulative_rate =
			calculate_accumulated_rate(rate_per_sec, cumulative_rate, now, last_updated);
		assert!(
			maybe_new_cumulative_rate.is_some(),
			"expect value to not overflow"
		);
		let cumulative_rate = maybe_new_cumulative_rate.unwrap();
		let expected_cumulative_rate = FixedU128P27::saturating_from_rational(
			1025315120504108509948668518u128,
			1000000000000000000000000000u128,
		);
		assert_eq!(expected_cumulative_rate, cumulative_rate);

		// calculate debt after half a year if the principal amount is 100
		let principal = FixedU128::from(100u128);
		let maybe_debt = debt(principal, cumulative_rate);
		assert!(maybe_debt.is_some(), "expect not to overflow");

		let expected_debt = principal
			.checked_mul(&convert::<FixedU128P27, FixedU128>(expected_cumulative_rate).unwrap())
			.unwrap();
		assert_eq!(expected_debt, maybe_debt.unwrap())
	}
}
