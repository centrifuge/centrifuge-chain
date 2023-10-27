use cfg_traits::{interest::InterestRate, Seconds};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{storage::bounded_vec::BoundedVec, RuntimeDebug};
use scale_info::TypeInfo;

use crate::{
	entities::input::{PrincipalInput, RepaidInput},
	pallet::Config,
	types::{
		policy::WriteOffRule, valuation::ValuationMethod, InterestPayments, Maturity,
		PayDownSchedule,
	},
};

/// Active loan mutation for internal pricing
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InternalMutation<Rate> {
	ValuationMethod(ValuationMethod<Rate>),
	ProbabilityOfDefault(Rate),
	LossGivenDefault(Rate),
	DiscountRate(InterestRate<Rate>),
}

/// Active loan mutation
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum LoanMutation<Rate> {
	Maturity(Maturity),
	MaturityExtension(Seconds),
	InterestRate(InterestRate<Rate>),
	InterestPayments(InterestPayments),
	PayDownSchedule(PayDownSchedule),
	Internal(InternalMutation<Rate>),
}

/// Change description
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum Change<T: Config> {
	Loan(T::LoanId, LoanMutation<T::Rate>),
	Policy(BoundedVec<WriteOffRule<T::Rate>, T::MaxWriteOffPolicySize>),
	TransferDebt(T::LoanId, T::LoanId, RepaidInput<T>, PrincipalInput<T>),
}
