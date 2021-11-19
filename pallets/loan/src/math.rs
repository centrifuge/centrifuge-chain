use crate::math::Adjustment::{Dec, Inc};
use crate::WriteOffGroup;
use sp_arithmetic::traits::{checked_pow, One};
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

/// calculates the debt using debt=principal_debt * cumulative_rate
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

/// returns the seconds in a given normal day
#[inline]
pub(crate) fn seconds_per_day() -> u64 {
	3600 * 24
}

/// returns the seconds in a given normal year(365 days)
/// https://docs.centrifuge.io/learn/interest-rate-methodology/
#[inline]
pub(crate) fn seconds_per_year() -> u64 {
	seconds_per_day() * 365
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
pub fn bullet_loan_risk_adjusted_expected_cash_flow<Amount, Rate>(
	debt: Amount,
	now: u64,
	maturity_date: u64,
	rate_per_sec: Rate,
	expected_loss_over_asset_maturity: Rate,
) -> Option<Amount>
where
	Amount: FixedPointNumber,
	Rate: FixedPointNumber + One,
{
	// check to be sure if the maturity date has not passed
	if now > maturity_date {
		return None;
	}

	// calculate the rate^(m-now)
	checked_pow(rate_per_sec, (maturity_date - now) as usize)
		// convert to amount
		.and_then(|rate| convert::<Rate, Amount>(rate))
		// calculate expected cash flow
		.and_then(|amount| debt.checked_mul(&amount))
		// calculate risk adjusted cash flow
		.and_then(|cf| {
			// cf*(1-expected_loss)
			let one: Rate = One::one();
			one.checked_sub(&expected_loss_over_asset_maturity)
				.and_then(|rr| convert::<Rate, Amount>(rr))
				.and_then(|rr| cf.checked_mul(&rr))
		})
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

/// returns the valid write off group given the maturity date and current time
/// since the write off groups are not guaranteed to be in a sorted order and
/// we want to preserve the index of the group,
/// we also pick the group that has the highest overdue days found in the vector
pub(crate) fn valid_write_off_group<Rate>(
	maturity_date: u64,
	now: u64,
	groups: Vec<WriteOffGroup<Rate>>,
) -> Option<u32> {
	let mut index = None;
	let mut highest_overdue_days = 0;
	let seconds_per_day = seconds_per_day();
	groups.iter().enumerate().for_each(|(idx, group)| {
		let overdue_days = group.overdue_days;
		let offset = maturity_date + seconds_per_day * overdue_days;
		if overdue_days >= highest_overdue_days && now >= offset {
			index = Some(idx as u32);
			highest_overdue_days = overdue_days;
		}
	});
	index
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
		// expected loss over asset maturity is 0.15% => 0.0015
		let expected_loss_over_asset_maturity = Rate::saturating_from_rational(15, 10000);
		// maturity date is 2 years
		let md = seconds_per_year() * 2;
		// assuming now = 0
		let now = 0;
		// interest rate is 5%
		let rate_per_sec = rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
		// expected cashflow should be 110.35
		let cf = bullet_loan_risk_adjusted_expected_cash_flow(
			debt,
			now,
			md,
			rate_per_sec,
			expected_loss_over_asset_maturity,
		)
		.unwrap();
		assert_eq!(
			cf,
			Amount::saturating_from_rational(110351316161105372133u128, Amount::accuracy())
		)
	}

	#[test]
	fn test_bullet_loan_present_value() {
		// debt is 100
		let debt = Amount::from_inner(100 * USD);
		// expected loss over asset maturity is 0.15% => 0.0015
		let expected_loss_over_asset_maturity = Rate::saturating_from_rational(15, 10000);
		// maturity date is 2 years
		let md = seconds_per_year() * 2;
		// assuming now = 0
		let now = 0;
		// interest rate is 5%
		let rp = rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
		// expected cashflow should be 110.35
		let cf = bullet_loan_risk_adjusted_expected_cash_flow(
			debt,
			now,
			md,
			rp,
			expected_loss_over_asset_maturity,
		)
		.unwrap();
		// discount rate is 4%
		let discount_rate = rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap();
		// present value should be 101.87
		let pv = bullet_loan_present_value(cf, now, md, discount_rate).unwrap();
		assert_eq!(
			pv,
			Amount::saturating_from_rational(101867103798764401467u128, Amount::accuracy())
		)
	}
}

#[test]
fn valid_write_off_groups() {
	let groups = vec![
		WriteOffGroup {
			percentage: (),
			overdue_days: 3,
		},
		WriteOffGroup {
			percentage: (),
			overdue_days: 5,
		},
		WriteOffGroup {
			percentage: (),
			overdue_days: 6,
		},
		WriteOffGroup {
			percentage: (),
			overdue_days: 14,
		},
		WriteOffGroup {
			percentage: (),
			overdue_days: 9,
		},
		WriteOffGroup {
			percentage: (),
			overdue_days: 7,
		},
	];

	let sec_per_day = seconds_per_day();

	// maturity date in days and current time offset to maturity date  and resultant index from the group
	let tests: Vec<(u64, u64, Option<u32>)> = vec![
		// day 0, and now is at zero, index is None
		(0, 0, None),
		(0, 1, None),
		// now is 3 and less than 5 days, the index is valid
		(0, 3, Some(0)),
		(0, 4, Some(0)),
		// now is 5 and less than 6 days, the index is valid
		(0, 5, Some(1)),
		// now is 6 and less than 7 days, the index is valid
		(0, 6, Some(2)),
		// now is 7 and 8 and less than 9 days, the index is valid
		(0, 7, Some(5)),
		(0, 8, Some(5)),
		// 9 <= now < 14, the index is valid
		(0, 9, Some(4)),
		// 14 <= now , the index is valid
		(0, 15, Some(3)),
	];
	tests.into_iter().for_each(|(maturity, now, index)| {
		let md = maturity * sec_per_day;
		let now = md + now * sec_per_day;
		let got_index = valid_write_off_group(md, now, groups.clone());
		assert_eq!(index, got_index);
	})
}
