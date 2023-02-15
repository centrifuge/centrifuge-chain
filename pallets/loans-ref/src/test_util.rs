use cfg_primitives::Moment;

use super::{
	types::{
		BorrowRestrictions, InterestPayments, LoanInfo, LoanRestrictions, Maturity,
		MaxBorrowAmount, PayDownSchedule, RepayRestrictions, RepaymentSchedule,
	},
	valuation::ValuationMethod,
};

impl<Asset, Balance, Rate> LoanInfo<Asset, Balance, Rate>
where
	Rate: Default,
	Balance: Default,
{
	pub fn empty(collateral: Asset) -> Self {
		LoanInfo {
			schedule: RepaymentSchedule {
				maturity: Maturity::Fixed(0),
				interest_payments: InterestPayments::None,
				pay_down_schedule: PayDownSchedule::None,
			},
			collateral: collateral,
			collateral_value: Balance::default(),
			valuation_method: ValuationMethod::OutstandingDebt,
			restrictions: LoanRestrictions {
				max_borrow_amount: MaxBorrowAmount::UpToTotalBorrowed {
					advance_rate: Rate::default(),
				},
				borrows: BorrowRestrictions::WrittenOff,
				repayments: RepayRestrictions::None,
			},
			interest_rate: Rate::default(),
		}
	}

	pub fn with_schedule(mut self, input: RepaymentSchedule) -> Self {
		self.schedule = input;
		self
	}

	pub fn with_maturity(mut self, moment: Moment) -> Self {
		self.schedule = RepaymentSchedule {
			maturity: Maturity::Fixed(moment),
			interest_payments: InterestPayments::None,
			pay_down_schedule: PayDownSchedule::None,
		};
		self
	}

	pub fn with_collateral_value(mut self, input: Balance) -> Self {
		self.collateral_value = input;
		self
	}

	pub fn with_valuation_method(mut self, input: ValuationMethod<Rate>) -> Self {
		self.valuation_method = input;
		self
	}

	pub fn with_restrictions(mut self, input: LoanRestrictions<Rate>) -> Self {
		self.restrictions = input;
		self
	}

	pub fn with_interest_rate(mut self, input: Rate) -> Self {
		self.interest_rate = input;
		self
	}
}
