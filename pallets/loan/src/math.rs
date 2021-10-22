use crate::math::Adjustment::{Dec, Inc};
use sp_arithmetic::traits::checked_pow;
use sp_arithmetic::FixedPointNumber;

/// calculates the latest accumulated rate since the last
pub fn calculate_accumulated_rate<Rate: FixedPointNumber>(
	rate_per_sec: Rate,
	current_accumulated_rate: Rate,
	now: u64,
	last_updated: u64,
) -> Option<Rate> {
	let pow = now - last_updated;
	checked_pow(rate_per_sec, pow as usize).and_then(|v| v.checked_mul(&current_accumulated_rate))
}

/// converts a fixed point from A precision to B precision
/// we don't convert from un-signed to signed or vice-verse
pub fn convert<A: FixedPointNumber, B: FixedPointNumber>(a: A) -> Option<B> {
	if A::SIGNED != B::SIGNED {
		return None;
	}

	B::checked_from_rational(a.into_inner(), A::accuracy())
}

/// calculates the debt using debt=normalised_debt * cumulative_rate
pub fn debt<Amount: FixedPointNumber, Rate: FixedPointNumber>(
	principal_debt: Amount,
	accumulated_rate: Rate,
) -> Option<Amount> {
	convert::<Rate, Amount>(accumulated_rate).and_then(|rate| principal_debt.checked_mul(&rate))
}

pub enum Adjustment<Amount: FixedPointNumber> {
	Inc(Amount),
	Dec(Amount),
}

/// calculates the principal debt after the adjustment
/// current_debt and cumulative_rate must be latest
pub fn calculate_principal_debt<Amount: FixedPointNumber, Rate: FixedPointNumber>(
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
/// https://docs.centrifuge.io/learn/interest-rate-methodology/
pub(crate) fn seconds_per_year() -> u64 {
	3600 * 24 * 365
}

/// calculates rate per second from the given nominal interest rate
/// https://docs.centrifuge.io/learn/interest-rate-methodology/
pub fn rate_per_sec<Rate: FixedPointNumber>(nominal_interest_rate: Rate) -> Option<Rate> {
	nominal_interest_rate
		.checked_div(&Rate::saturating_from_integer(seconds_per_year() as u128))
		.and_then(|res| res.checked_add(&Rate::one()))
}

/// calculates the risk adjusted expected cash flow for bullet loan type
/// assumes maturity date has not passed
/// https://docs.centrifuge.io/learn/pool-valuation/#simple-example-for-one-financing
pub fn bullet_loan_expected_cash_flow<Amount, Rate>(
	debt: Amount,
	now: u64,
	maturity_date: u64,
	rate_per_sec: Rate,
	term_recovery_rate: Rate,
) -> Option<Amount>
where
	Amount: FixedPointNumber,
	Rate: FixedPointNumber,
{
	// check to be sure if the maturity date has not passed
	if now > maturity_date {
		return None;
	}

	// calculate the rate^(m-now)
	checked_pow(rate_per_sec, (maturity_date - now) as usize)
		// multiply by term_recovery_rate
		.and_then(|i| i.checked_mul(&term_recovery_rate))
		// convert to amount
		.and_then(|rate| convert::<Rate, Amount>(rate))
		// calculate expected cash flow
		.and_then(|amount| debt.checked_mul(&amount))
}

/// calculates present value for bullet loan
/// assumes maturity date has not passed
/// https://docs.centrifuge.io/learn/pool-valuation/#simple-example-for-one-financing
pub fn bullet_loan_present_value<Amount, Rate>(
	expected_cash_flow: Amount,
	now: u64,
	maturity_date: u64,
	discount_rate: Rate,
) -> Option<Amount>
where
	Amount: FixedPointNumber,
	Rate: FixedPointNumber,
{
	if now > maturity_date {
		return None;
	}

	// calculate total discount rate
	checked_pow(discount_rate, (maturity_date - now) as usize)
		.and_then(|rate| convert::<Rate, Amount>(rate))
		// calculate the present value
		.and_then(|d| expected_cash_flow.checked_div(&d))
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::sp_runtime::traits::CheckedMul;
	use runtime_common::{Amount, Rate, CFG as USD};
	use sp_arithmetic::{FixedI128, PerThing};
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
		let c = Rate::checked_from_rational(1, 23).unwrap();
		let conv = convert::<Rate, FixedU128>(c);
		assert!(conv.is_some(), "conversion should pass");
		assert_eq!(
			conv.unwrap(),
			FixedU128::checked_from_rational(1, 23).unwrap()
		);

		// lower precision to higher
		let c = FixedU128::checked_from_rational(1, 23).unwrap();
		let conv = convert::<FixedU128, Rate>(c);
		assert!(conv.is_some(), "conversion should pass");
		assert_eq!(
			conv.unwrap(),
			Rate::checked_from_rational(43478260869565217000000000u128, Rate::accuracy()).unwrap()
		);
	}

	#[test]
	fn test_calculate_accumulated_rate() {
		// 5% interest rate
		let nir = Percent::from_percent(5);
		let rate = Rate::saturating_from_rational(nir.deconstruct(), Percent::ACCURACY);
		let rate_per_sec = rate_per_sec(rate).unwrap_or_default();
		assert!(rate_per_sec.is_positive(), "should not be zero");

		// initial accumulated_rate
		let accumulated_rate = Rate::from(1);

		// moment values
		let last_updated = 0u64;
		// after half a year
		let now = (3600 * 24 * 365) / 2;

		// calculate cumulative rate after half a year with compounding in seconds
		let maybe_new_cumulative_rate =
			calculate_accumulated_rate(rate_per_sec, accumulated_rate, now, last_updated);
		assert!(
			maybe_new_cumulative_rate.is_some(),
			"expect value to not overflow"
		);
		let cumulative_rate = maybe_new_cumulative_rate.unwrap();
		let expected_accumulated_rate = Rate::saturating_from_rational(
			1025315120504108509948668518u128,
			1000000000000000000000000000u128,
		);
		assert_eq!(expected_accumulated_rate, cumulative_rate);

		// calculate debt after half a year if the principal amount is 100
		let principal = FixedU128::from(100u128);
		let maybe_debt = debt(principal, cumulative_rate);
		assert!(maybe_debt.is_some(), "expect not to overflow");

		let expected_debt = principal
			.checked_mul(&convert::<Rate, FixedU128>(expected_accumulated_rate).unwrap())
			.unwrap();
		assert_eq!(expected_debt, maybe_debt.unwrap())
	}

	#[test]
	fn test_bullet_loan_expected_cash_flow() {
		// debt is 100
		let debt = Amount::from_inner(100 * USD);
		// ttr = 99.8 percent
		let term_recovery_rate = Rate::from_float(0.998);
		// maturity date is 2 years
		let md = seconds_per_year() * 2;
		// assuming now = 0
		let now = 0;
		// interest rate is 5%
		let rate_per_sec = rate_per_sec(Rate::from_float(0.05)).unwrap();
		// expected cashflow should be 110.296
		let cf = bullet_loan_expected_cash_flow(debt, now, md, rate_per_sec, term_recovery_rate)
			.unwrap();
		assert_eq!(
			cf,
			Amount::saturating_from_rational(110296057615205970100u128, Amount::accuracy())
		)
	}

	#[test]
	fn test_bullet_loan_present_value() {
		// debt is 100
		let debt = Amount::from_inner(100 * USD);
		// ttr = 99.8 percent
		let term_recovery_rate = Rate::from_float(0.998);
		// maturity date is 2 years
		let md = seconds_per_year() * 2;
		// assuming now = 0
		let now = 0;
		// interest rate is 5%
		let rp = rate_per_sec(Rate::from_float(0.05)).unwrap();
		// expected cashflow should be 110.296
		let cf = bullet_loan_expected_cash_flow(debt, now, md, rp, term_recovery_rate).unwrap();
		// discount rate is 4%
		let discount_rate = rate_per_sec(Rate::from_float(0.04)).unwrap();
		// present value should be 101.81
		let pv = bullet_loan_present_value(cf, now, md, discount_rate).unwrap();
		assert_eq!(
			pv,
			Amount::saturating_from_rational(101816093731764518466u128, Amount::accuracy())
		)
	}
}
