use cfg_primitives::{Balance, CollectionId, ItemId, LoanId, PoolId, SECONDS_PER_MINUTE};
use cfg_traits::{
	interest::{CompoundingSchedule, InterestRate},
	Seconds, TimeAsSecs,
};
use cfg_types::{
	fixed_point::{Quantity, Rate},
	oracles::OracleKey,
	permissions::PoolRole,
};
use frame_support::{assert_err, traits::Get};
use pallet_loans::{
	entities::{
		changes::LoanMutation,
		input::{PrincipalInput, RepaidInput},
		loans::LoanInfo,
		pricing::{
			external::{ExternalAmount, ExternalPricing, MaxBorrowAmount as ExtMaxBorrowAmount},
			internal::{InternalPricing, MaxBorrowAmount as IntMaxBorrowAmount},
			Pricing,
		},
	},
	types::{
		valuation::ValuationMethod, BorrowLoanError, BorrowRestrictions, InterestPayments,
		LoanRestrictions, Maturity, PayDownSchedule, RepayRestrictions, RepaymentSchedule,
	},
};
use runtime_common::apis::{
	runtime_decl_for_loans_api::LoansApiV1, runtime_decl_for_pools_api::PoolsApiV1,
};

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::runtime_env::RuntimeEnv,
		utils::{
			self,
			currency::{self, cfg, usd6, CurrencyInfo, Usd6},
			genesis::{self, Genesis},
			POOL_MIN_EPOCH_TIME,
		},
	},
	utils::{accounts::Keyring, tokens::rate_from_percent},
};

const POOL_ADMIN: Keyring = Keyring::Admin;
const INVESTOR: Keyring = Keyring::Alice;
const BORROWER: Keyring = Keyring::Bob;
const LOAN_ADMIN: Keyring = Keyring::Charlie;
const ANY: Keyring = Keyring::Dave;

const POOL_A: PoolId = 23;
const NFT_A: (CollectionId, ItemId) = (1, ItemId(10));
const PRICE_A: OracleKey = OracleKey::Isin(*b"INE123456AB1");
const PRICE_VALUE_A: Quantity = Quantity::from_integer(1_000);
const PRICE_VALUE_B: Quantity = Quantity::from_integer(500);

const FOR_FEES: Balance = cfg(1);
const EXPECTED_POOL_BALANCE: Balance = usd6(1_000_000);
const COLLATERAL_VALUE: Balance = usd6(100_000);
const QUANTITY: Quantity = Quantity::from_integer(100);

/// Common utilities for loan use cases
mod common {
	use super::*;

	pub fn initialize_state_for_loans<E: Env<T>, T: Runtime>() -> E {
		let mut env = E::from_storage(
			Genesis::<T>::default()
				.add(genesis::balances(T::ExistentialDeposit::get() + FOR_FEES))
				.add(genesis::assets(vec![Usd6::ID]))
				.add(genesis::tokens(vec![(Usd6::ID, Usd6::ED)]))
				.storage(),
			Genesis::<T>::default().storage(),
		);

		env.parachain_state_mut(|| {
			// Creating a pool
			utils::give_balance::<T>(POOL_ADMIN.id(), T::PoolDeposit::get());
			utils::create_empty_pool::<T>(POOL_ADMIN.id(), POOL_A, Usd6::ID);

			// Setting borrower
			utils::give_pool_role::<T>(BORROWER.id(), POOL_A, PoolRole::Borrower);
			utils::give_nft::<T>(BORROWER.id(), NFT_A);

			// Setting a loan admin
			utils::give_pool_role::<T>(LOAN_ADMIN.id(), POOL_A, PoolRole::LoanAdmin);

			// Funding a pool
			let tranche_id = T::Api::tranche_id(POOL_A, 0).unwrap();
			let tranche_investor = PoolRole::TrancheInvestor(tranche_id, Seconds::MAX);
			utils::give_pool_role::<T>(INVESTOR.id(), POOL_A, tranche_investor);
			utils::give_tokens::<T>(INVESTOR.id(), Usd6::ID, EXPECTED_POOL_BALANCE);
			utils::invest::<T>(INVESTOR.id(), POOL_A, tranche_id, EXPECTED_POOL_BALANCE);
		});

		env.pass(Blocks::BySeconds(POOL_MIN_EPOCH_TIME));

		env.parachain_state_mut(|| {
			// New epoch with the investor funds available
			utils::close_pool_epoch::<T>(POOL_ADMIN.id(), POOL_A);
		});

		env
	}

	pub fn last_loan_id<E: Env<T>, T: Runtime>(env: &E) -> LoanId {
		env.find_event(|e| match e {
			pallet_loans::Event::<T>::Created { loan_id, .. } => Some(loan_id),
			_ => None,
		})
		.unwrap()
	}

	pub fn last_change_id<E: Env<T>, T: Runtime>(env: &E) -> T::Hash {
		env.find_event(|e| match e {
			pallet_pool_system::Event::<T>::ProposedChange { change_id, .. } => Some(change_id),
			_ => None,
		})
		.unwrap()
	}

	pub fn default_loan_info<T: Runtime>(now: Seconds, pricing: Pricing<T>) -> LoanInfo<T> {
		LoanInfo {
			schedule: RepaymentSchedule {
				maturity: Maturity::Fixed {
					date: now + SECONDS_PER_MINUTE,
					extension: SECONDS_PER_MINUTE / 2,
				},
				interest_payments: InterestPayments::None,
				pay_down_schedule: PayDownSchedule::None,
			},
			interest_rate: InterestRate::Fixed {
				rate_per_year: rate_from_percent(20),
				compounding: CompoundingSchedule::Secondly,
			},
			collateral: NFT_A,
			pricing: pricing,
			restrictions: LoanRestrictions {
				borrows: BorrowRestrictions::NotWrittenOff,
				repayments: RepayRestrictions::None,
			},
		}
	}

	pub fn default_internal_pricing<T: Runtime>() -> Pricing<T> {
		Pricing::Internal(InternalPricing {
			collateral_value: COLLATERAL_VALUE,
			max_borrow_amount: IntMaxBorrowAmount::UpToTotalBorrowed {
				advance_rate: rate_from_percent(100),
			},
			valuation_method: ValuationMethod::OutstandingDebt,
		})
	}

	pub fn default_external_pricing<T: Runtime>() -> Pricing<T> {
		Pricing::External(ExternalPricing {
			price_id: PRICE_A,
			max_borrow_amount: ExtMaxBorrowAmount::Quantity(QUANTITY),
			notional: currency::price_to_currency(PRICE_VALUE_A, Usd6::ID),
			max_price_variation: rate_from_percent(0),
		})
	}
}

/// Predefined loan calls for use cases
mod call {
	use super::*;

	pub fn create<T: Runtime>(info: &LoanInfo<T>) -> pallet_loans::Call<T> {
		pallet_loans::Call::create {
			pool_id: POOL_A,
			info: info.clone(),
		}
	}

	pub fn borrow_internal<T: Runtime>(loan_id: LoanId) -> pallet_loans::Call<T> {
		pallet_loans::Call::borrow {
			pool_id: POOL_A,
			loan_id,
			amount: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
		}
	}

	pub fn borrow_external<T: Runtime>(loan_id: LoanId) -> pallet_loans::Call<T> {
		pallet_loans::Call::borrow {
			pool_id: POOL_A,
			loan_id,
			amount: PrincipalInput::External(ExternalAmount {
				quantity: Quantity::from_integer(50),
				settlement_price: currency::price_to_currency(PRICE_VALUE_A, Usd6::ID),
			}),
		}
	}

	pub fn repay_internal<T: Runtime>(loan_id: LoanId, interest: Balance) -> pallet_loans::Call<T> {
		pallet_loans::Call::repay {
			pool_id: POOL_A,
			loan_id,
			amount: RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
				interest,
				unscheduled: 0,
			},
		}
	}

	pub fn repay_external<T: Runtime>(
		loan_id: LoanId,
		interest: Balance,
		settlement_price: Quantity,
	) -> pallet_loans::Call<T> {
		pallet_loans::Call::repay {
			pool_id: POOL_A,
			loan_id,
			amount: RepaidInput {
				principal: PrincipalInput::External(ExternalAmount {
					quantity: Quantity::from_integer(50),
					settlement_price: currency::price_to_currency(settlement_price, Usd6::ID),
				}),
				interest,
				unscheduled: 0,
			},
		}
	}

	pub fn close<T: Runtime>(loan_id: LoanId) -> pallet_loans::Call<T> {
		pallet_loans::Call::close {
			pool_id: POOL_A,
			loan_id,
		}
	}

	pub fn propose_loan_mutation<T: Runtime>(
		loan_id: LoanId,
		mutation: LoanMutation<Rate>,
	) -> pallet_loans::Call<T> {
		pallet_loans::Call::propose_loan_mutation {
			pool_id: POOL_A,
			loan_id,
			mutation,
		}
	}

	pub fn apply_loan_mutation<T: Runtime>(change_id: T::Hash) -> pallet_loans::Call<T> {
		pallet_loans::Call::apply_loan_mutation {
			pool_id: POOL_A,
			change_id,
		}
	}
}

/// Test the basic loan flow, which consist in:
/// - create a loan
/// - borrow from the loan
/// - fully repay the loan until
/// - close the loan
fn internal_priced<T: Runtime>() {
	let mut env = common::initialize_state_for_loans::<RuntimeEnv<T>, T>();

	let info = env.parachain_state(|| {
		let now = <pallet_timestamp::Pallet<T> as TimeAsSecs>::now();
		common::default_loan_info::<T>(now, common::default_internal_pricing())
	});
	env.submit_now(BORROWER, call::create(&info)).unwrap();

	let loan_id = common::last_loan_id(&env);

	env.submit_now(BORROWER, call::borrow_internal(loan_id))
		.unwrap();

	env.pass(Blocks::BySeconds(SECONDS_PER_MINUTE / 2));

	let loan_portfolio = env.parachain_state(|| T::Api::portfolio_loan(POOL_A, loan_id).unwrap());
	env.parachain_state_mut(|| {
		// Give required tokens to the borrower to be able to repay the interest accrued
		// until this moment
		utils::give_tokens::<T>(BORROWER.id(), Usd6::ID, loan_portfolio.outstanding_interest);
	});

	env.submit_now(
		BORROWER,
		call::repay_internal(loan_id, loan_portfolio.outstanding_interest),
	)
	.unwrap();

	// Closing the loan succesfully means that the loan has been fully repaid
	env.submit_now(BORROWER, call::close(loan_id)).unwrap();
}

/// Test using oracles to price the loan
fn oracle_priced<T: Runtime>() {
	let mut env = common::initialize_state_for_loans::<RuntimeEnv<T>, T>();

	env.parachain_state_mut(|| utils::feed_oracle::<T>(vec![(PRICE_A, PRICE_VALUE_A)]));

	let info = env.parachain_state(|| {
		let now = <pallet_timestamp::Pallet<T> as TimeAsSecs>::now();
		common::default_loan_info::<T>(now, common::default_external_pricing())
	});
	env.submit_now(BORROWER, call::create(&info)).unwrap();

	let loan_id = common::last_loan_id(&env);

	env.submit_now(BORROWER, call::borrow_external(loan_id))
		.unwrap();

	env.pass(Blocks::BySeconds(SECONDS_PER_MINUTE / 2));

	let loan_portfolio = env.parachain_state(|| T::Api::portfolio_loan(POOL_A, loan_id).unwrap());
	env.parachain_state_mut(|| {
		// Give required tokens to the borrower to be able to repay the interest accrued
		// until this moment
		utils::give_tokens::<T>(BORROWER.id(), Usd6::ID, loan_portfolio.outstanding_interest);

		// Oracle modify the value
		utils::feed_oracle::<T>(vec![(PRICE_A, PRICE_VALUE_B)])
	});

	env.submit_now(
		BORROWER,
		call::repay_external(loan_id, loan_portfolio.outstanding_interest, PRICE_VALUE_B),
	)
	.unwrap();

	// Closing the loan succesfully means that the loan has been fully repaid
	env.submit_now(BORROWER, call::close(loan_id)).unwrap();
}

fn update_maturity_extension<T: Runtime>() {
	let mut env = common::initialize_state_for_loans::<RuntimeEnv<T>, T>();

	let info = env.parachain_state(|| {
		let now = <pallet_timestamp::Pallet<T> as TimeAsSecs>::now();
		common::default_loan_info::<T>(now, common::default_internal_pricing())
	});
	env.submit_now(BORROWER, call::create(&info)).unwrap();

	let loan_id = common::last_loan_id(&env);

	env.submit_now(BORROWER, call::borrow_internal(loan_id))
		.unwrap();

	env.pass(Blocks::BySeconds(SECONDS_PER_MINUTE));

	// Loan at this point is overdue and trying to borrow it will fail
	assert_err!(
		env.submit_now(BORROWER, call::borrow_internal(loan_id)),
		pallet_loans::Error::<T>::BorrowLoanError(BorrowLoanError::MaturityDatePassed),
	);

	env.submit_now(
		LOAN_ADMIN,
		call::propose_loan_mutation(loan_id, LoanMutation::MaturityExtension(12 /* seconds */)),
	)
	.unwrap();

	let change_id = common::last_change_id(&env);
	env.submit_now(ANY, call::apply_loan_mutation(change_id))
		.unwrap();

	// Now the loan is no longer overdue and can be borrowed again
	env.submit_now(BORROWER, call::borrow_internal(loan_id))
		.unwrap();
}

crate::test_for_runtimes!(all, internal_priced);
crate::test_for_runtimes!(all, oracle_priced);
crate::test_for_runtimes!(all, update_maturity_extension);
