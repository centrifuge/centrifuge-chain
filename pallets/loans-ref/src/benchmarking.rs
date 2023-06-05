// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::CFG;
use cfg_traits::{
	data::{DataCollection, DataRegistry},
	InterestAccrual, Permissions, PoolBenchmarkHelper,
};
use cfg_types::{
	adjustments::Adjustment,
	permissions::{PermissionScope, PoolRole, Role},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{
	storage::bounded_vec::BoundedVec,
	traits::{
		tokens::nonfungibles::{Create, Mutate},
		UnixTime,
	},
};
use frame_system::RawOrigin;
use orml_traits::DataFeeder;
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::{Get, One, Zero};
use sp_std::{time::Duration, vec};

use super::{
	loan::LoanInfo,
	pallet::*,
	pricing::{
		internal::{InternalPricing, MaxBorrowAmount},
		Pricing,
	},
	types::{
		policy::{WriteOffRule, WriteOffTrigger},
		valuation::{DiscountedCashFlow, ValuationMethod},
		BorrowRestrictions, InterestPayments, LoanRestrictions, Maturity, PayDownSchedule,
		RepayRestrictions, RepaymentSchedule,
	},
};

const OFFSET: Duration = Duration::from_secs(120);
const COLLECION_ID: u16 = 42;
const COLLATERAL_VALUE: u128 = 1_000_000;
const FUNDS: u128 = 1_000_000_000;

type MaxRateCountOf<T> = <<T as Config>::InterestAccrual as InterestAccrual<
	<T as Config>::Rate,
	<T as Config>::Balance,
	Adjustment<<T as Config>::Balance>,
>>::MaxRateCount;

type MaxCollectionSizeOf<T> = <<T as Config>::PriceRegistry as DataRegistry<
	<T as Config>::PriceId,
	PoolIdOf<T>,
>>::MaxCollectionSize;

struct Helper<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Helper<T>
where
	T::Balance: From<u128>,
	T::NonFungible: Create<T::AccountId> + Mutate<T::AccountId>,
	T::CollectionId: From<u16>,
	T::ItemId: From<u16>,
	T::PriceId: From<u32>,
	T::Pool:
		PoolBenchmarkHelper<PoolId = PoolIdOf<T>, AccountId = T::AccountId, Balance = T::Balance>,
	PriceCollectionOf<T>: DataCollection<T::PriceId, Data = PriceResultOf<T>>,
	T::PriceRegistry: DataFeeder<T::PriceId, T::Balance, T::AccountId>,
{
	#[cfg(test)]
	fn config_mocks() {
		use cfg_mocks::pallet_mock_data::util::MockDataCollection;

		use crate::mock::{MockPermissions, MockPools, MockPrices};

		MockPermissions::mock_add(|_, _, _| Ok(()));
		MockPermissions::mock_has(|_, _, _| true);
		MockPools::mock_pool_exists(|_| true);
		MockPools::mock_account_for(|_| 0);
		MockPools::mock_withdraw(|_, _, _| Ok(()));
		MockPools::mock_deposit(|_, _, _| Ok(()));
		MockPools::mock_benchmark_create_pool(|_, _| {});
		MockPools::mock_benchmark_give_ausd(|_, _| {});
		MockPrices::mock_feed_value(|_, _, _| Ok(()));
		MockPrices::mock_register_id(|_, _| Ok(()));
		MockPrices::mock_collection(|_| MockDataCollection::new(|_| Ok((0, 0))));
	}

	fn prepare_benchmark() -> PoolIdOf<T> {
		#[cfg(test)]
		Self::config_mocks();

		let pool_id = Default::default();

		let pool_admin = account("pool_admin", 0, 0);
		T::Pool::benchmark_create_pool(pool_id, &pool_admin);

		let loan_admin = account("loan_admin", 0, 0);
		T::Permissions::add(
			PermissionScope::Pool(pool_id),
			loan_admin,
			Role::PoolRole(PoolRole::LoanAdmin),
		)
		.unwrap();

		let borrower = account::<T::AccountId>("borrower", 0, 0);
		T::Pool::benchmark_give_ausd(&borrower, (FUNDS * CFG).into());
		T::NonFungible::create_collection(&COLLECION_ID.into(), &borrower, &borrower).unwrap();
		T::Permissions::add(
			PermissionScope::Pool(pool_id),
			borrower,
			Role::PoolRole(PoolRole::Borrower),
		)
		.unwrap();

		pool_id
	}

	fn base_loan(item_id: T::ItemId) -> LoanInfo<T> {
		LoanInfo {
			schedule: RepaymentSchedule {
				maturity: Maturity::Fixed((T::Time::now() + OFFSET).as_secs()),
				interest_payments: InterestPayments::None,
				pay_down_schedule: PayDownSchedule::None,
			},
			collateral: (COLLECION_ID.into(), item_id),
			pricing: Pricing::Internal(InternalPricing {
				collateral_value: COLLATERAL_VALUE.into(),
				interest_rate: T::Rate::saturating_from_rational(1, 5000),
				max_borrow_amount: MaxBorrowAmount::UpToOutstandingDebt {
					advance_rate: T::Rate::one(),
				},
				valuation_method: ValuationMethod::DiscountedCashFlow(DiscountedCashFlow {
					probability_of_default: T::Rate::zero(),
					loss_given_default: T::Rate::zero(),
					discount_rate: T::Rate::one(),
				}),
			}),
			restrictions: LoanRestrictions {
				borrows: BorrowRestrictions::NotWrittenOff,
				repayments: RepayRestrictions::None,
			},
		}
	}

	fn create_loan(pool_id: PoolIdOf<T>, item_id: T::ItemId) -> T::LoanId {
		let borrower = account("borrower", 0, 0);

		T::NonFungible::mint_into(&COLLECION_ID.into(), &item_id, &borrower).unwrap();

		Pallet::<T>::create(
			RawOrigin::Signed(borrower).into(),
			pool_id,
			Self::base_loan(item_id),
		)
		.unwrap();

		LastLoanId::<T>::get(pool_id)
	}

	fn borrow_loan(pool_id: PoolIdOf<T>, loan_id: T::LoanId) {
		let borrower = account("borrower", 0, 0);
		Pallet::<T>::borrow(
			RawOrigin::Signed(borrower).into(),
			pool_id,
			loan_id,
			10.into(),
		)
		.unwrap();
	}

	fn fully_repay_loan(pool_id: PoolIdOf<T>, loan_id: T::LoanId) {
		let borrower = account("borrower", 0, 0);
		Pallet::<T>::repay(
			RawOrigin::Signed(borrower).into(),
			pool_id,
			loan_id,
			COLLATERAL_VALUE.into(),
			0.into(),
		)
		.unwrap();
	}

	// Worst case policy where you need to iterate for the whole policy.
	fn create_policy() -> BoundedVec<WriteOffRule<T::Rate>, T::MaxWriteOffPolicySize> {
		vec![
			WriteOffRule::new(
				[WriteOffTrigger::PrincipalOverdueDays(0)],
				T::Rate::zero(),
				T::Rate::zero(),
			);
			T::MaxWriteOffPolicySize::get() as usize
		]
		.try_into()
		.unwrap()
	}

	fn set_policy(pool_id: PoolIdOf<T>) {
		let pool_admin = account::<T::AccountId>("pool_admin", 0, 0);

		Pallet::<T>::update_write_off_policy(
			RawOrigin::Signed(pool_admin).into(),
			pool_id,
			Self::create_policy(),
		)
		.unwrap();
	}

	fn expire_loan(pool_id: PoolIdOf<T>, loan_id: T::LoanId) {
		Pallet::<T>::expire(pool_id, loan_id).unwrap();
	}

	fn initialize_active_state(n: u32) -> PoolIdOf<T> {
		let pool_id = Self::prepare_benchmark();

		for i in 1..MaxRateCountOf::<T>::get() {
			// First `i` (i=0) used by the loan's interest rate.
			let rate = T::Rate::saturating_from_rational(i + 1, 5000);
			T::InterestAccrual::reference_rate(rate).unwrap();
		}

		for i in 0..MaxCollectionSizeOf::<T>::get() {
			let price_id = i.into();
			// This account is different in each iteration because of how oracles works.
			// This restriction no longer exists once
			// https://github.com/open-web3-stack/open-runtime-module-library/pull/920 is merged
			let feeder = account("feeder", i, 0);
			T::PriceRegistry::feed_value(feeder, price_id, 0.into()).unwrap();
			T::PriceRegistry::register_id(&price_id, &pool_id).unwrap();
		}

		for i in 0..n {
			let item_id = (i as u16).into();
			let loan_id = Self::create_loan(pool_id, item_id);
			Self::borrow_loan(pool_id, loan_id);
		}

		pool_id
	}

	fn max_active_loans() -> u32 {
		T::MaxActiveLoansPerPool::get().min(10)
	}
}

benchmarks! {
	where_clause {
	where
		T::Balance: From<u128>,
		T::NonFungible: Create<T::AccountId> + Mutate<T::AccountId>,
		T::CollectionId: From<u16>,
		T::ItemId: From<u16>,
		T::PriceId: From<u32>,
		T::Pool: PoolBenchmarkHelper<PoolId = PoolIdOf<T>, AccountId = T::AccountId, Balance = T::Balance>,
		PriceCollectionOf<T>: DataCollection<T::PriceId, Data = PriceResultOf<T>>,
		T::PriceRegistry: DataFeeder<T::PriceId, T::Balance, T::AccountId>,
	}

	create {
		let borrower = account("borrower", 0, 0);
		let pool_id = Helper::<T>::prepare_benchmark();

		let (collection_id, item_id) = (COLLECION_ID.into(), 1.into());
		T::NonFungible::mint_into(&collection_id, &item_id, &borrower).unwrap();
		let loan_info = Helper::<T>::base_loan(item_id);

	}: _(RawOrigin::Signed(borrower), pool_id, loan_info)

	borrow {
		let n in 1..Helper::<T>::max_active_loans() - 1;

		let borrower = account("borrower", 0, 0);
		let pool_id = Helper::<T>::initialize_active_state(n);
		let loan_id = Helper::<T>::create_loan(pool_id, u16::MAX.into());

	}: _(RawOrigin::Signed(borrower), pool_id, loan_id, 10.into())

	repay {
		let n in 1..Helper::<T>::max_active_loans() - 1;

		let borrower = account("borrower", 0, 0);
		let pool_id = Helper::<T>::initialize_active_state(n);
		let loan_id = Helper::<T>::create_loan(pool_id, u16::MAX.into());
		Helper::<T>::borrow_loan(pool_id, loan_id);

	}: _(RawOrigin::Signed(borrower), pool_id, loan_id, 10.into(), 0.into())

	write_off {
		let n in 1..Helper::<T>::max_active_loans() - 1;

		let borrower = account("borrower", 0, 0);
		let pool_id = Helper::<T>::initialize_active_state(n);
		let loan_id = Helper::<T>::create_loan(pool_id, u16::MAX.into());
		Helper::<T>::borrow_loan(pool_id, loan_id);
		Helper::<T>::set_policy(pool_id);
		Helper::<T>::expire_loan(pool_id, loan_id);

	}: _(RawOrigin::Signed(borrower), pool_id, loan_id)

	admin_write_off {
		let n in 1..Helper::<T>::max_active_loans() - 1;

		let loan_admin = account("loan_admin", 0, 0);
		let pool_id = Helper::<T>::initialize_active_state(n);
		let loan_id = Helper::<T>::create_loan(pool_id, u16::MAX.into());
		Helper::<T>::borrow_loan(pool_id, loan_id);
		Helper::<T>::set_policy(pool_id);

	}: _(RawOrigin::Signed(loan_admin), pool_id, loan_id, T::Rate::zero(), T::Rate::zero())

	close {
		let n in 1..Helper::<T>::max_active_loans() - 1;

		let borrower = account("borrower", 0, 0);
		let pool_id = Helper::<T>::initialize_active_state(n);
		let loan_id = Helper::<T>::create_loan(pool_id, u16::MAX.into());
		Helper::<T>::borrow_loan(pool_id, loan_id);
		Helper::<T>::fully_repay_loan(pool_id, loan_id);

	}: _(RawOrigin::Signed(borrower), pool_id, loan_id)

	update_write_off_policy {
		let pool_admin = account("pool_admin", 0, 0);
		let pool_id = Helper::<T>::prepare_benchmark();
		let policy = Helper::<T>::create_policy();

	}: _(RawOrigin::Signed(pool_admin), pool_id, policy)

	update_portfolio_valuation {
		let n in 1..Helper::<T>::max_active_loans();

		let borrower = account("borrower", 0, 0);
		let pool_id = Helper::<T>::initialize_active_state(n);

	}: _(RawOrigin::Signed(borrower), pool_id)
	verify {
		assert!(Pallet::<T>::portfolio_valuation(pool_id).value() > Zero::zero());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
