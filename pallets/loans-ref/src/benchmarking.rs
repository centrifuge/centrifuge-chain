use std::time::Duration;

use cfg_traits::{InterestAccrual, PoolInspect};
use cfg_types::adjustments::Adjustment;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::traits::{
	tokens::nonfungibles::{Create, Mutate},
	UnixTime,
};
use frame_system::RawOrigin;
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::{Get, One, Zero};

use super::{
	pallet::*,
	types::{LoanInfo, MaxBorrowAmount},
	valuation::{DiscountedCashFlow, ValuationMethod},
};

const OFFSET: Duration = Duration::from_secs(100);
const LOAN_BENCHMARK_COLLECTION_ID: u16 = 29482;

#[cfg(test)]
fn config_mocks() {
	use crate::mock::{MockPermissions, MockPools};

	MockPermissions::mock_has(|_, _, _| true);
	MockPools::mock_pool_exists(|_| true);
	MockPools::mock_account_for(|_| 0);
	MockPools::mock_withdraw(|_, _, _| Ok(()));
	MockPools::mock_deposit(|_, _, _| Ok(()));
}

fn create_loan<T: Config>(
	caller: &T::AccountId,
	pool_id: PoolIdOf<T>,
	asset: (T::CollectionId, T::ItemId),
) -> T::LoanId
where
	T::Balance: From<u32>,
{
	Pallet::<T>::create(
		RawOrigin::Signed(caller.clone()).into(),
		pool_id,
		LoanInfo::new(asset)
			.maturity(T::Time::now() + OFFSET)
			.interest_rate(T::Rate::saturating_from_rational(1, 2))
			.collateral_value(10000.into())
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

fn borrow_loan<T: Config>(caller: &T::AccountId, pool_id: PoolIdOf<T>, loan_id: T::LoanId)
where
	T::Balance: From<u32>,
{
	Pallet::<T>::borrow(
		RawOrigin::Signed(caller.clone()).into(),
		pool_id,
		loan_id,
		100.into(),
	)
	.unwrap();
}

benchmarks! {
	where_clause {
		where
		<T::Pool as PoolInspect<T::AccountId, T::CurrencyId>>::PoolId: From<u32>,
		T::Balance: From<u32>,
		T::NonFungible: Create<T::AccountId> + Mutate<T::AccountId>,
		T::CollectionId: From<u16>,
		T::ItemId: From<u16>,
	}

	update_portfolio_valuation {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..<T::InterestAccrual as InterestAccrual<T::Rate, T::Balance, Adjustment<T::Balance>>>::MaxRateCount::get();

		#[cfg(test)]
		config_mocks();

		let caller = whitelisted_caller();
		let pool_id = 1.into();

		let collection_id = LOAN_BENCHMARK_COLLECTION_ID.into();
		T::NonFungible::create_collection(&collection_id, &caller, &caller).unwrap();

		for i in 0..n {
			let item_id = (i as u16).into();
			T::NonFungible::mint_into(&collection_id, &item_id, &caller).unwrap();

			let loan_id = create_loan::<T>(&caller, pool_id, (collection_id, item_id));
			borrow_loan::<T>(&caller, pool_id, loan_id);
		}
	}: _(RawOrigin::Signed(caller), pool_id)
	verify {
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
