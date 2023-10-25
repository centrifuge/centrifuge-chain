use cfg_primitives::{conversion, Balance, CollectionId, ItemId, LoanId, PoolId, SECONDS_PER_HOUR};
use cfg_traits::{
	interest::{CompoundingSchedule, InterestRate},
	Seconds, TimeAsSecs,
};
use cfg_types::{
	fixed_point::Quantity, oracles::OracleKey, permissions::PoolRole, tokens::CurrencyId,
};
use frame_support::traits::Get;
use pallet_loans::{
	entities::{
		input::{PrincipalInput, RepaidInput},
		loans::LoanInfo,
		pricing::{
			external::{ExternalPricing, MaxBorrowAmount as ExtMaxBorrowAmount},
			internal::{InternalPricing, MaxBorrowAmount as IntMaxBorrowAmount},
			Pricing,
		},
	},
	types::{
		valuation::ValuationMethod, BorrowRestrictions, InterestPayments, LoanRestrictions,
		Maturity, PayDownSchedule, RepayRestrictions, RepaymentSchedule,
	},
};
use runtime_common::apis::{
	runtime_decl_for_LoansApi::LoansApiV1, runtime_decl_for_PoolsApi::PoolsApiV1,
};

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::runtime_env::RuntimeEnv,
		utils::{
			self,
			genesis::{
				self,
				currency::{self, cfg, usd6, CurrencyInfo, Usd6},
				Genesis,
			},
			POOL_MIN_EPOCH_TIME,
		},
	},
	utils::{accounts::Keyring, tokens::rate_from_percent},
};

const POOL_ADMIN: Keyring = Keyring::Admin;
const INVESTOR: Keyring = Keyring::Alice;
const BORROWER: Keyring = Keyring::Bob;

const POOL_A: PoolId = 23;
const NFT_A: (CollectionId, ItemId) = (1, ItemId(10));
const PRICE_A: OracleKey = OracleKey::Isin(*b"INE123456AB1");
const PRICE_A_VALUE: Quantity = Quantity::from_integer(1_000);

const FOR_FEES: Balance = cfg(1);
const EXPECTED_POOL_BALANCE: Balance = usd6(1_000_000);
const COLLATERAL_VALUE: Balance = usd6(100_000);
const QUANTITY: Quantity = Quantity::from_integer(100);

mod common {
	use super::*;

	pub fn initialize_state_for_loans<E: Env<T>, T: Runtime>() -> E {
		let mut env = E::from_storage(
			Genesis::<T>::default()
				.add(genesis::balances(T::ExistentialDeposit::get() + FOR_FEES))
				.add(genesis::assets(vec![Usd6::ID]))
				.add(genesis::tokens(vec![(Usd6::ID, Usd6::ED)]))
				.storage(),
		);

		env.state_mut(|| {
			// Creating a pool
			utils::give_balance::<T>(POOL_ADMIN.id(), T::PoolDeposit::get());
			utils::create_empty_pool::<T>(POOL_ADMIN.id(), POOL_A, Usd6::ID);

			// Funding a pool
			let tranche_id = T::Api::tranche_id(POOL_A, 0).unwrap();
			let tranche_investor = PoolRole::TrancheInvestor(tranche_id, Seconds::MAX);
			utils::give_pool_role::<T>(INVESTOR.id(), POOL_A, tranche_investor);
			utils::give_tokens::<T>(INVESTOR.id(), Usd6::ID, EXPECTED_POOL_BALANCE);
			utils::invest::<T>(INVESTOR.id(), POOL_A, tranche_id, EXPECTED_POOL_BALANCE);
		});

		env.pass(Blocks::BySeconds(POOL_MIN_EPOCH_TIME));

		env.state_mut(|| {
			// New epoch with the investor funds available
			utils::close_pool_epoch::<T>(POOL_ADMIN.id(), POOL_A);

			// Preparing borrower
			utils::give_pool_role::<T>(BORROWER.id(), POOL_A, PoolRole::Borrower);
			utils::give_nft::<T>(BORROWER.id(), NFT_A);
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

	pub fn default_loan<T: Runtime>(now: Seconds, pricing: Pricing<T>) -> LoanInfo<T> {
		LoanInfo {
			schedule: RepaymentSchedule {
				maturity: Maturity::Fixed {
					date: now + SECONDS_PER_HOUR,
					extension: SECONDS_PER_HOUR / 2,
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

	pub fn default_external_pricing<T: Runtime>(curency_id: CurrencyId) -> Pricing<T> {
		Pricing::External(ExternalPricing {
			price_id: PRICE_A,
			max_borrow_amount: ExtMaxBorrowAmount::Quantity(QUANTITY),
			notional: conversion::fixed_point_to_balance(
				PRICE_A_VALUE,
				currency::find_metadata(curency_id).decimals as usize,
			)
			.unwrap(),
			max_price_variation: rate_from_percent(0),
		})
	}
}

/// Test the basic loan flow, which consist in:
/// - create a loan
/// - borrow from the loan
/// - fully repay the loan until
/// - close the loan
fn basic_loan_flow<T: Runtime>() {
	let mut env = common::initialize_state_for_loans::<RuntimeEnv<T>, T>();

	let info = env.state(|| {
		let now = <pallet_timestamp::Pallet<T> as TimeAsSecs>::now();
		common::default_loan::<T>(now, common::default_internal_pricing())
	});

	env.submit_now(
		BORROWER,
		pallet_loans::Call::create {
			pool_id: POOL_A,
			info,
		},
	)
	.unwrap();

	let loan_id = common::last_loan_id(&env);

	env.submit_now(
		BORROWER,
		pallet_loans::Call::borrow {
			pool_id: POOL_A,
			loan_id,
			amount: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
		},
	)
	.unwrap();

	env.pass(Blocks::BySeconds(SECONDS_PER_HOUR / 2));

	let loan_portfolio = env.state(|| T::Api::portfolio_loan(POOL_A, loan_id).unwrap());

	env.state_mut(|| {
		// Give required tokens to the borrower to be able to repay the interest accrued
		// until this moment
		utils::give_tokens::<T>(BORROWER.id(), Usd6::ID, loan_portfolio.outstanding_interest);
	});

	env.submit_now(
		BORROWER,
		pallet_loans::Call::repay {
			pool_id: POOL_A,
			loan_id,
			amount: RepaidInput {
				principal: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
				interest: loan_portfolio.outstanding_interest,
				unscheduled: 0,
			},
		},
	)
	.unwrap();

	env.submit_now(
		BORROWER,
		pallet_loans::Call::close {
			pool_id: POOL_A,
			loan_id,
		},
	)
	.unwrap();
}

/// Test using oracles to price the loan
fn oracle_priced_loan<T: Runtime>() {
	let mut env = common::initialize_state_for_loans::<RuntimeEnv<T>, T>();

	env.state_mut(|| utils::feed_oracle::<T>(vec![(PRICE_A, PRICE_A_VALUE)]));

	let info = env.state(|| {
		let now = <pallet_timestamp::Pallet<T> as TimeAsSecs>::now();
		common::default_loan::<T>(now, common::default_external_pricing(Usd6::ID))
	});

	env.submit_now(
		BORROWER,
		pallet_loans::Call::create {
			pool_id: POOL_A,
			info,
		},
	)
	.unwrap();

	let loan_id = common::last_loan_id(&env);

	// TODO: in progress
}

crate::test_for_runtimes!(all, basic_loan_flow);
crate::test_for_runtimes!(all, oracle_priced_loan);
