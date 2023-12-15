use cfg_primitives::SECONDS_PER_WEEK;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use pallet_loans::entities::changes::{Change as LoansChange, InternalMutation, LoanMutation};
use pallet_pool_fees::types::Change as PoolFeesChange;
use pallet_pool_system::pool_types::changes::{PoolChangeProposal, Requirement};
use scale_info::TypeInfo;
use sp_runtime::DispatchError;
use sp_std::{marker::PhantomData, vec, vec::Vec};

/// Auxiliar type to carry all pallets bounds used by RuntimeChange
pub trait Changeable: pallet_loans::Config + pallet_pool_fees::Config {}
impl<T: pallet_loans::Config + pallet_pool_fees::Config> Changeable for T {}

/// A change done in the runtime, shared between pallets
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RuntimeChange<T: Changeable, Options: Clone = ()> {
	Loans(LoansChange<T>),
	PoolFee(PoolFeesChange<T>),
	_Unreachable(PhantomData<Options>),
}

impl<T: Changeable, Options: Clone> RuntimeChange<T, Options> {
	fn requirement_list(self) -> Vec<Requirement> {
		let epoch = Requirement::NextEpoch;
		let week = Requirement::DelayTime(SECONDS_PER_WEEK as u32);
		let blocked = Requirement::BlockedByLockedRedemptions;

		match self {
			RuntimeChange::Loans(change) => match change {
				// Requirements gathered from
				// <https://docs.google.com/spreadsheets/d/1RJ5RLobAdumXUK7k_ugxy2eDAwI5akvtuqUM2Tyn5ts>
				LoansChange::<T>::Loan(_, loan_mutation) => match loan_mutation {
					LoanMutation::Maturity(_) => vec![week, blocked],
					LoanMutation::MaturityExtension(_) => vec![],
					LoanMutation::InterestPayments(_) => vec![week, blocked],
					LoanMutation::PayDownSchedule(_) => vec![week, blocked],
					LoanMutation::InterestRate(_) => vec![epoch],
					LoanMutation::Internal(mutation) => match mutation {
						InternalMutation::ValuationMethod(_) => vec![week, blocked],
						InternalMutation::ProbabilityOfDefault(_) => vec![epoch],
						InternalMutation::LossGivenDefault(_) => vec![epoch],
						InternalMutation::DiscountRate(_) => vec![epoch],
					},
				},
				LoansChange::<T>::Policy(_) => vec![week, blocked],
				LoansChange::<T>::TransferDebt(_, _, _, _) => vec![],
			},
			RuntimeChange::PoolFee(pool_fees_change) => match pool_fees_change {
				// TODO(william): Gather requirements similar to above
				PoolFeesChange::AppendFee(_, _) => vec![week],
			},
			RuntimeChange::_Unreachable(_) => vec![],
		}
	}
}

impl<T: Changeable> From<RuntimeChange<T>> for PoolChangeProposal {
	fn from(runtime_change: RuntimeChange<T>) -> Self {
		if cfg!(feature = "runtime-benchmarks") {
			PoolChangeProposal::new([])
		} else {
			PoolChangeProposal::new(runtime_change.requirement_list())
		}
	}
}

/// Option to pass to RuntimeChange to enable fast delays
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct FastDelay;

impl<T: Changeable> From<RuntimeChange<T, FastDelay>> for PoolChangeProposal {
	fn from(runtime_change: RuntimeChange<T, FastDelay>) -> Self {
		if cfg!(feature = "runtime-benchmarks") {
			PoolChangeProposal::new([])
		} else {
			let new_requirements =
				runtime_change
					.requirement_list()
					.into_iter()
					.map(|req| match req {
						Requirement::DelayTime(_) => Requirement::DelayTime(60), // 1 min
						req => req,
					});

			PoolChangeProposal::new(new_requirements)
		}
	}
}

macro_rules! runtime_change_support {
	($change:ident, $variant:ident) => {
		/// Used by `ChangeGuard::note()`
		impl<T: Changeable, Option: Clone> From<$change<T>> for RuntimeChange<T, Option> {
			fn from(change: $change<T>) -> RuntimeChange<T, Option> {
				RuntimeChange::$variant(change)
			}
		}

		/// Used `ChangeGuard::released()`
		impl<T: Changeable, Option: Clone> TryInto<$change<T>> for RuntimeChange<T, Option> {
			type Error = DispatchError;

			fn try_into(self) -> Result<$change<T>, DispatchError> {
				match self {
					RuntimeChange::$variant(change) => Ok(change),
					_ => Err(DispatchError::Other("Expected another RuntimeChange")),
				}
			}
		}
	};
}

// Add the variants you want to support for RuntimeChange
runtime_change_support!(LoansChange, Loans);
runtime_change_support!(PoolFeesChange, PoolFee);
