use cfg_traits::{interest::InterestRate, Seconds};
use frame_support::{pallet_prelude::RuntimeDebug, storage::bounded_vec::BoundedVec};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use crate::{
	entities::input::{PrincipalInput, RepaidInput},
	pallet::Config,
	types::{
		cashflow::{InterestPayments, Maturity, PayDownSchedule},
		policy::WriteOffRule,
		valuation::ValuationMethod,
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
