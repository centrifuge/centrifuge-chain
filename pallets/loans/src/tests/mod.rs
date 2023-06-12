use std::time::Duration;

use cfg_mocks::pallet_mock_data::util::MockDataCollection;
use cfg_types::permissions::{PermissionScope, PoolRole, Role};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;
use sp_runtime::{traits::BadOrigin, DispatchError, FixedPointNumber};

use super::{
	entities::{
		loans::{ActiveLoan, LoanInfo},
		pricing::{
			external::ExternalPricing,
			internal::{InternalPricing, MaxBorrowAmount},
			ActivePricing, Pricing,
		},
	},
	pallet::{ActiveLoans, Error, LastLoanId, PortfolioValuation},
	types::{
		policy::{WriteOffRule, WriteOffStatus, WriteOffTrigger},
		valuation::{DiscountedCashFlow, ValuationMethod},
		BorrowLoanError, BorrowRestrictions, Change, CloseLoanError, CreateLoanError,
		InterestPayments, InternalMutation, LoanMutation, LoanRestrictions, Maturity,
		MutationError, PayDownSchedule, RepayLoanError, RepayRestrictions, RepaymentSchedule,
		WrittenOffError,
	},
};

const COLLATERAL_VALUE: Balance = 10000;
const DEFAULT_INTEREST_RATE: f64 = 0.5;
const POLICY_PERCENTAGE: f64 = 0.5;
const POLICY_PENALTY: f64 = 0.5;
const REGISTER_PRICE_ID: PriceId = 42;
const UNREGISTER_PRICE_ID: PriceId = 88;
const PRICE_VALUE: Rate = Rate::from_u32(1000);
const QUANTITY: Balance = 20;
const CHANGE_ID: ChangeId = H256::repeat_byte(0x42);

/// Used where the error comes from other pallet impl. unknown from the tests
const DEPENDENCY_ERROR: DispatchError = DispatchError::Other("dependency error");

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
