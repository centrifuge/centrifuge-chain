use cfg_primitives::{Balance, CollectionId, ItemId, PoolId, SECONDS_PER_YEAR};
use cfg_traits::{
	interest::{CompoundingSchedule, InterestRate},
	Seconds, TimeAsSecs,
};
use cfg_types::permissions::PoolRole;
use frame_support::traits::Get;
use pallet_loans::{
	entities::{
		input::PrincipalInput,
		loans::LoanInfo,
		pricing::{
			internal::{InternalPricing, MaxBorrowAmount as IntMaxBorrowAmount},
			Pricing,
		},
	},
	types::{
		valuation::ValuationMethod, BorrowRestrictions, InterestPayments, LoanRestrictions,
		Maturity, PayDownSchedule, RepayRestrictions, RepaymentSchedule,
	},
};
use runtime_common::apis::runtime_decl_for_PoolsApi::PoolsApiV1;

use crate::{
	generic::{
		environment::{Blocks, Env},
		envs::runtime_env::RuntimeEnv,
		runtime::Runtime,
		utils::{
			self,
			genesis::{
				self,
				currency::{cfg, usd6, CurrencyInfo, Usd6},
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

const FOR_FEES: Balance = cfg(1);
const EXPECTED_POOL_BALANCE: Balance = usd6(1_000_000);
const COLLATERAL_VALUE: Balance = usd6(100_000);

fn initialize_state_for_loans<Environment: Env<T>, T: Runtime>() -> Environment {
	let mut env = Environment::from_storage(
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

fn internal_priced_loan<T: Runtime>(now: Seconds) -> LoanInfo<T> {
	LoanInfo {
		schedule: RepaymentSchedule {
			maturity: Maturity::Fixed {
				date: now + SECONDS_PER_YEAR,
				extension: SECONDS_PER_YEAR / 2,
			},
			interest_payments: InterestPayments::None,
			pay_down_schedule: PayDownSchedule::None,
		},
		interest_rate: InterestRate::Fixed {
			rate_per_year: rate_from_percent(20),
			compounding: CompoundingSchedule::Secondly,
		},
		collateral: NFT_A,
		pricing: Pricing::Internal(InternalPricing {
			collateral_value: COLLATERAL_VALUE,
			max_borrow_amount: IntMaxBorrowAmount::UpToTotalBorrowed {
				advance_rate: rate_from_percent(100),
			},
			valuation_method: ValuationMethod::OutstandingDebt,
		}),
		restrictions: LoanRestrictions {
			borrows: BorrowRestrictions::NotWrittenOff,
			repayments: RepayRestrictions::None,
		},
	}
}

fn borrow<T: Runtime>() {
	let mut env = initialize_state_for_loans::<RuntimeEnv<T>, T>();

	let info = env.state(|| {
		let now = <pallet_timestamp::Pallet<T> as TimeAsSecs>::now();
		internal_priced_loan::<T>(now)
	});

	env.submit_now(
		BORROWER,
		pallet_loans::Call::create {
			pool_id: POOL_A,
			info,
		},
	)
	.unwrap();

	let loan_id = env
		.find_event(|e| match e {
			pallet_loans::Event::<T>::Created { loan_id, .. } => Some(loan_id),
			_ => None,
		})
		.unwrap();

	env.submit_now(
		BORROWER,
		pallet_loans::Call::borrow {
			pool_id: POOL_A,
			loan_id,
			amount: PrincipalInput::Internal(COLLATERAL_VALUE / 2),
		},
	)
	.unwrap();
}

crate::test_for_runtimes!(all, borrow);
