use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum CfgChange {
	Loan(LoanChange),
	Pool(PoolChange),
}

impl From<LoanChange> for CfgChange {
	fn from(value: LoanChange) -> Self {
		CfgChange::Loan(value)
	}
}

impl From<PoolChange> for CfgChange {
	fn from(value: PoolChange) -> Self {
		CfgChange::Pool(value)
	}
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum LoanChange {
	Maturity,
	InterestRate,
	InterestPayments,
	PayDownSchedule,
	ValuationMethod,
	ProbabilityOfDefault,
	LossGivenDefault,
	DiscountRate,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum PoolChange {
	// Unimplemented
}
