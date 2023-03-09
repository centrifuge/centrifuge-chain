use cfg_traits::{InterestAccrual, PoolBenchmarkHelper};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::{
	tokens::nonfungibles::{Create, Mutate},
	UnixTime,
};
use frame_system::RawOrigin;
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::{Get, One, Zero};
use sp_std::time::Duration;

use super::{
	pallet::*,
	types::{LoanInfo, MaxBorrowAmount},
	valuation::{DiscountedCashFlow, ValuationMethod},
};

const OFFSET: Duration = Duration::from_secs(100);
const COLLECION_ID: u16 = 42;

fn prepare_benchmark<T: Config>(admin: &T::AccountId, borrower: &T::AccountId) -> PoolIdOf<T>
where
	T::Balance: From<u128>,
	T::Pool:
		PoolBenchmarkHelper<PoolId = PoolIdOf<T>, AccountId = T::AccountId, Balance = T::Balance>,
{
	#[cfg(test)]
	{
		let _ = (admin, borrower);

		use crate::mock::{MockPermissions, MockPools};

		MockPermissions::mock_has(|_, _, _| true);
		MockPools::mock_pool_exists(|_| true);
		MockPools::mock_account_for(|_| 0);
		MockPools::mock_withdraw(|_, _, _| Ok(()));
		MockPools::mock_deposit(|_, _, _| Ok(()));

		Default::default()
	}

	#[cfg(not(test))]
	{
		use cfg_primitives::CFG;
		use cfg_traits::Permissions;
		use cfg_types::permissions::{PermissionScope, PoolRole, Role};

		let pool_id = Default::default();
		T::Pool::benchmark_create_pool(pool_id, admin);

		T::Permissions::add(
			PermissionScope::Pool(pool_id),
			borrower.clone(),
			Role::PoolRole(PoolRole::Borrower),
		)
		.unwrap();

		let funds = 1_000_000_000;
		T::Pool::benchmark_give_ausd(borrower, (funds * CFG).into());

		pool_id
	}
}

fn create_loan<T: Config>(
	borrower: &T::AccountId,
	pool_id: PoolIdOf<T>,
	asset: (T::CollectionId, T::ItemId),
) -> T::LoanId
where
	T::Balance: From<u128>,
{
	Pallet::<T>::create(
		RawOrigin::Signed(borrower.clone()).into(),
		pool_id,
		LoanInfo::new(asset)
			.maturity(T::Time::now() + OFFSET)
			.interest_rate(T::Rate::saturating_from_rational(1, 5000))
			.collateral_value((1_000_000).into())
			.max_borrow_amount(MaxBorrowAmount::UpToOutstandingDebt {
				advance_rate: T::Rate::one(),
			})
			.valuation_method(ValuationMethod::DiscountedCashFlow(DiscountedCashFlow {
				probability_of_default: T::Rate::zero(),
				loss_given_default: T::Rate::zero(),
				discount_rate: T::Rate::one(),
			})),
	)
	.unwrap();

	LastLoanId::<T>::get(pool_id)
}

fn borrow_loan<T: Config>(borrower: &T::AccountId, pool_id: PoolIdOf<T>, loan_id: T::LoanId)
where
	T::Balance: From<u128>,
{
	Pallet::<T>::borrow(
		RawOrigin::Signed(borrower.clone()).into(),
		pool_id,
		loan_id,
		10.into(),
	)
	.unwrap();
}

benchmarks! {
	where_clause {
		where
		PoolIdOf<T>: From<u32>,
		T::Balance: From<u128>,
		T::NonFungible: Create<T::AccountId> + Mutate<T::AccountId>,
		T::CollectionId: From<u16>,
		T::ItemId: From<u16>,
		T::Pool: PoolBenchmarkHelper<PoolId = PoolIdOf<T>, AccountId = T::AccountId, Balance = T::Balance>,
	}

	update_portfolio_valuation {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..MaxRateCountOf::<T>::get();

		for i in 1..m {
			// First `i` (with value 0) used by the loan's interest rate.
			let rate = T::Rate::saturating_from_rational(i + 1, 5000);
			T::InterestAccrual::reference_yearly_rate(rate).unwrap();
		}

		let pool_admin = account::<T::AccountId>("pool_admin", 0, 0);
		let borrower = account::<T::AccountId>("borrower", 0, 0);
		let pool_id = prepare_benchmark::<T>(&pool_admin, &borrower);

		let collection_id = COLLECION_ID.into();
		T::NonFungible::create_collection(&collection_id, &borrower, &borrower).unwrap();

		for i in 0..n {
			let item_id = (i as u16).into();
			T::NonFungible::mint_into(&collection_id, &item_id, &borrower).unwrap();

			let loan_id = create_loan::<T>(&borrower, pool_id, (collection_id, item_id));
			borrow_loan::<T>(&borrower, pool_id, loan_id);
		}
	}: _(RawOrigin::Signed(borrower), pool_id)
	verify {
		assert!(Pallet::<T>::portfolio_valuation(pool_id).value() > Zero::zero());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
