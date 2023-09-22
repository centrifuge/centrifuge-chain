use std::time::Duration;

use cfg_mocks::pallet_mock_data::util::MockDataCollection;
use cfg_primitives::SECONDS_PER_DAY;
use cfg_traits::interest::{CompoundingSchedule, InterestRate};
use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok, storage::bounded_vec::BoundedVec};
use sp_runtime::{traits::BadOrigin, DispatchError, FixedPointNumber};

use super::{
	entities::{
		changes::{Change, InternalMutation, LoanMutation},
		loans::{ActiveLoan, LoanInfo},
		pricing::{
			external::{ExternalAmount, ExternalPricing, MaxBorrowAmount as ExtMaxBorrowAmount},
			internal::{InternalPricing, MaxBorrowAmount as IntMaxBorrowAmount},
			ActivePricing, Pricing, PricingAmount, RepaidPricingAmount,
		},
	},
	pallet::{ActiveLoans, CreatedLoan, Error, LastLoanId, PortfolioValuation},
	types::{
		policy::{WriteOffRule, WriteOffStatus, WriteOffTrigger},
		valuation::{DiscountedCashFlow, ValuationMethod},
		BorrowLoanError, BorrowRestrictions, CloseLoanError, CreateLoanError, InterestPayments,
		LoanRestrictions, Maturity, MutationError, PayDownSchedule, RepayLoanError,
		RepayRestrictions, RepaymentSchedule, WrittenOffError,
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
mod transfer_debt;
mod util;
mod write_off_loan;
