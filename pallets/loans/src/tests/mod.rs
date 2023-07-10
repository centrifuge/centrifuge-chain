use std::time::Duration;

use cfg_mocks::pallet_mock_data::util::MockDataCollection;
use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok, storage::bounded_vec::BoundedVec};
use sp_runtime::{traits::BadOrigin, DispatchError, FixedPointNumber};

use super::{
	entities::{
		loans::{ActiveLoan, LoanInfo},
		pricing::{
			external::{ExternalPricing, MaxBorrowAmount as ExtMaxBorrowAmount},
			internal::{InternalPricing, MaxBorrowAmount as IntMaxBorrowAmount},
			ActivePricing, Pricing,
		},
	},
	pallet::{ActiveLoans, Error, LastLoanId, PortfolioValuation},
	types::{
		policy::{WriteOffRule, WriteOffStatus, WriteOffTrigger},
		valuation::{DiscountedCashFlow, ValuationMethod},
		BorrowLoanError, BorrowRestrictions, Change, CloseLoanError, CreateLoanError,
		InterestPayments, InternalMutation, LoanMutation, LoanRestrictions, Maturity,
		MutationError, PayDownSchedule, RepaidAmount, RepayLoanError, RepayRestrictions,
		RepaymentSchedule, WrittenOffError,
	},
};

pub mod mock;
use mock::*;

mod borrow_loan;
mod close_loan;
mod create_loan;
mod mutate_loan;
mod policy;
mod portfolio_valuation;
mod repay_loan;
mod util;
mod write_off_loan;
