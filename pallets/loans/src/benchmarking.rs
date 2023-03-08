// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Module provides benchmarking for Loan Pallet
use cfg_primitives::CFG as CURRENCY;
use cfg_traits::Investment;
use cfg_types::{
	fixed_point::Rate,
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use cfg_utils::set_block_number_timestamp;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{
	assert_ok,
	traits::{tokens::fungibles::Inspect, Currency, Hooks, IsType},
};
use frame_system::RawOrigin;
use orml_tokens::{Config as ORMLConfig, Pallet as ORMLPallet};
use orml_traits::{
	asset_registry::{Inspect as RegistryInspect, Mutate},
	MultiCurrency,
};
use pallet_balances::Pallet as BalancePallet;
use pallet_interest_accrual::{Config as InterestAccrualConfig, Pallet as InterestAccrualPallet};
use pallet_pool_system::pool_types::PoolLocator;
use pallet_timestamp::{Config as TimestampConfig, Pallet as TimestampPallet};
use sp_runtime::traits::{AccountIdConversion, CheckedDiv};
use test_utils::{
	assert_last_event, create as create_test_pool, create_nft_class_if_needed, expect_asset_owner,
	expect_asset_to_be_burned, get_tranche_id, mint_nft_of,
};

use super::*;
use crate::{
	loan_type::{BulletLoan, CreditLineWithMaturity},
	test_utils::{initialise_test_pool, FundsAccount},
	types::WriteOffGroupInput,
	Config as LoanConfig, Event as LoanEvent, Pallet as LoansPallet,
};

pub struct Pallet<T: Config>(LoansPallet<T>);

pub trait Config:
	LoanConfig<ClassId = <Self as pallet_uniques::Config>::CollectionId>
	+ pallet_aura::Config
	+ pallet_balances::Config
	+ pallet_uniques::Config
	+ pallet_pool_system::Config
	+ ORMLConfig
	+ TimestampConfig
	+ InterestAccrualConfig
{
	type IM: Investment<Self::AccountId, Amount = u128, InvestmentId = TrancheCurrency>;
}

#[cfg(test)]
impl Config for super::mock::Runtime {
	type IM = mock::OrderManager;
}

fn create_and_init_pool<T: Config>(
	init_pool: bool,
) -> (
	T::AccountId,
	PoolIdOf<T>,
	T::AccountId,
	<T as LoanConfig>::ClassId,
)
where
	<T as pallet_balances::Config>::Balance: From<u128>,
	<T as pallet_uniques::Config>::CollectionId: From<u64>,
	<T as pallet_pool_system::Config>::Balance: From<u128>,
	<T as pallet_pool_system::Config>::CurrencyId: From<CurrencyId>,
	<T as pallet_pool_system::Config>::TrancheId: Into<[u8; 16]>,
	<T as pallet_pool_system::Config>::EpochId: From<u32>,
	<T as pallet_pool_system::Config>::PoolId: Into<u64> + IsType<PoolIdOf<T>>,
	<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
	<T as ORMLConfig>::Balance: From<u128>,
	<T as pallet_uniques::Config>::CollectionId: Default,
{
	// create pool
	let pool_owner = account::<T::AccountId>("owner", 0, 0);
	make_free_cfg_balance::<T>(pool_owner.clone());
	make_free_token_balance::<T>(
		CurrencyId::AUSD,
		&FundsAccount::get().into_account_truncating(),
		(1000 * CURRENCY).into(),
	);
	let pool_id: PoolIdOf<T> = Default::default();
	let pool_account = pool_account::<T>(pool_id.into());
	let pal_pool_id: <T as pallet_pool_system::Config>::PoolId = pool_id.into();
	if T::AssetRegistry::metadata(&CurrencyId::AUSD.into()).is_none() {
		T::AssetRegistry::register_asset(
			Some(CurrencyId::AUSD.into()),
			orml_asset_registry::AssetMetadata {
				decimals: 18,
				name: "MOCK AUSD TOKEN".as_bytes().to_vec(),
				symbol: "MckAUSD".as_bytes().to_vec(),
				existential_deposit: 0.into(),
				location: None,
				additional: CustomMetadata::default(),
			},
		)
		.expect("Registering pool currency must work");
	}
	create_test_pool::<T, T::IM>(pool_id.into(), pool_owner.clone(), CurrencyId::AUSD);
	let tranche_id = get_tranche_id::<T>(pool_id.into(), 0);
	make_free_token_balance::<T>(
		CurrencyId::Tranche(pal_pool_id.into(), tranche_id.into()),
		&pool_account,
		(500 * CURRENCY).into(),
	);
	let tranche_id = get_tranche_id::<T>(pool_id.into(), 1);
	make_free_token_balance::<T>(
		CurrencyId::Tranche(pal_pool_id.into(), tranche_id.into()),
		&pool_account,
		(500 * CURRENCY).into(),
	);

	make_free_cfg_balance::<T>(borrower::<T>());
	assert_ok!(<T as pallet_pool_system::Config>::Permission::add(
		PermissionScope::Pool(pool_id.into()),
		borrower::<T>(),
		Role::PoolRole(PoolRole::Borrower)
	));
	assert_ok!(<T as pallet_pool_system::Config>::Permission::add(
		PermissionScope::Pool(pool_id.into()),
		borrower::<T>(),
		Role::PoolRole(PoolRole::PricingAdmin)
	));
	assert_ok!(<T as pallet_pool_system::Config>::Permission::add(
		PermissionScope::Pool(pool_id.into()),
		borrower::<T>(),
		Role::PoolRole(PoolRole::LoanAdmin)
	));

	make_free_cfg_balance::<T>(risk_admin::<T>());
	assert_ok!(<T as pallet_pool_system::Config>::Permission::add(
		PermissionScope::Pool(pool_id.into()),
		risk_admin::<T>(),
		Role::PoolRole(PoolRole::LoanAdmin)
	));

	// initialise pool on loan
	let loan_account = LoansPallet::<T>::account_id();
	make_free_cfg_balance::<T>(loan_account.clone());
	let mut loan_class_id = Default::default();
	if init_pool {
		loan_class_id =
			initialise_test_pool::<T>(pool_id, 1, pool_owner.clone(), Some(loan_account.clone()));
	}

	whitelist_acc::<T>(&pool_owner);
	whitelist_acc::<T>(&loan_account);
	(pool_owner, pool_id, loan_account, loan_class_id)
}

fn make_free_cfg_balance<T>(account: T::AccountId)
where
	T: Config + pallet_balances::Config,
	<T as pallet_balances::Config>::Balance: From<u128>,
{
	let min_balance: <T as pallet_balances::Config>::Balance = (100u128 * CURRENCY).into();
	let _ = BalancePallet::<T>::make_free_balance_be(&account, min_balance);
}

fn make_free_token_balance<T>(
	currency_id: CurrencyId,
	account: &T::AccountId,
	balance: <T as ORMLConfig>::Balance,
) where
	T: Config + ORMLConfig,
	<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
{
	<ORMLPallet<T> as MultiCurrency<T::AccountId>>::deposit(currency_id.into(), account, balance)
		.expect("should not fail to set new token balance");
}

fn check_free_token_balance<T>(
	currency_id: CurrencyId,
	account: &T::AccountId,
	balance: <T as ORMLConfig>::Balance,
) where
	T: Config + ORMLConfig,
	<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
{
	assert_eq!(
		ORMLPallet::<T>::balance(currency_id.into(), account),
		balance
	);
}

fn get_free_token_balance<T>(
	currency_id: CurrencyId,
	account: &T::AccountId,
) -> <T as ORMLConfig>::Balance
where
	T: Config + ORMLConfig,
	<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
{
	ORMLPallet::<T>::balance(currency_id.into(), account)
}

fn whitelist_acc<T: frame_system::Config>(acc: &T::AccountId) {
	frame_benchmarking::benchmarking::add_to_whitelist(
		frame_system::Account::<T>::hashed_key_for(acc).into(),
	);
}

fn risk_admin<T: frame_system::Config>() -> T::AccountId {
	let risk_admin = account::<T::AccountId>("risk_admin", 0, 0);
	whitelist_acc::<T>(&risk_admin);
	risk_admin
}

fn borrower<T: frame_system::Config>() -> T::AccountId {
	let borrower = account::<T::AccountId>("borrower", 0, 0);
	whitelist_acc::<T>(&borrower);
	borrower
}

fn create_asset<T: Config + frame_system::Config>(loan_id: T::LoanId) -> (T::AccountId, AssetOf<T>)
where
	<T as pallet_balances::Config>::Balance: From<u128>,
	<T as pallet_uniques::Config>::CollectionId: From<u64>,
{
	// create asset
	let loan_owner = borrower::<T>();
	make_free_cfg_balance::<T>(loan_owner.clone());
	let asset_class_id = create_nft_class_if_needed::<T>(2, loan_owner.clone(), None);
	let asset_instance_id = mint_nft_of::<T>(loan_owner.clone(), asset_class_id, loan_id);
	let asset = Asset(asset_class_id, asset_instance_id);
	whitelist_acc::<T>(&loan_owner);
	(loan_owner, asset)
}

fn activate_test_loan_with_rate<T: Config>(
	pool_id: PoolIdOf<T>,
	loan_id: T::LoanId,
	borrower: T::AccountId,
	rate: u32,
) where
	<T as LoanConfig>::Rate: From<Rate>,
	<T as LoanConfig>::Balance: From<u128>,
{
	let loan_type = LoanType::CreditLineWithMaturity(CreditLineWithMaturity::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100).into(),
		// probability of default is 4%
		Rate::saturating_from_rational(4, 100).into(),
		// loss given default is 50%
		Rate::saturating_from_rational(50, 100).into(),
		// collateral value
		(125 * CURRENCY).into(),
		// 4%
		math::interest_rate_per_sec(Rate::saturating_from_rational(4, 100))
			.unwrap()
			.into(),
		// 2 years
		math::seconds_per_year() * 2,
	));
	let rp: <T as pallet::Config>::Rate = interest_rate(rate).into();
	LoansPallet::<T>::price(
		RawOrigin::Signed(borrower).into(),
		pool_id,
		loan_id,
		rp,
		loan_type,
	)
	.expect("loan activation should not fail");
}

fn add_test_write_off_groups<T: Config>(pool_id: PoolIdOf<T>, risk_admin: T::AccountId)
where
	<T as LoanConfig>::Rate: From<Rate>,
{
	for group in &[(3, 10), (5, 15), (7, 20), (20, 30), (120, 100)] {
		LoansPallet::<T>::add_write_off_group(
			RawOrigin::Signed(risk_admin.clone()).into(),
			pool_id,
			WriteOffGroupInput {
				percentage: Rate::saturating_from_rational(group.1, 100).into(),
				penalty_interest_rate_per_year: Rate::saturating_from_rational(1, 100).into(),
				overdue_days: group.0,
			},
		)
		.expect("adding write off groups should not fail");
	}
}

fn pool_account<T: pallet_pool_system::Config>(pool_id: T::PoolId) -> T::AccountId {
	PoolLocator { pool_id }.into_account_truncating()
}

// Populate interest rates for worst-case lookup of our actual rate
fn populate_bench_storage<T: Config>(
	pool_id: PoolIdOf<T>,
	loans: u32,
	rates: u32,
) -> Option<AssetOf<T>>
where
	<T as LoanConfig>::Rate: From<Rate>,
	<T as LoanConfig>::LoanId: From<u32>,
	<T as InterestAccrualConfig>::InterestRate: From<Rate>,
	<T as pallet_balances::Config>::Balance: From<u128>,
	<T as pallet_uniques::Config>::CollectionId: From<u64>,
	<T as pallet_pool_system::Config>::PoolId: Into<u64> + IsType<PoolIdOf<T>>,
{
	let mut collateral = None;
	for idx in 1..rates {
		let rate = interest_rate(idx).into();
		InterestAccrualPallet::<T>::reference_yearly_rate(rate)
			.expect("Must be able to reference dummy interest rates");
	}
	for idx in 0..loans {
		let loan_id = (idx + 1).into();
		let (loan_owner, asset) = create_asset::<T>(loan_id);
		collateral = Some(asset);
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset)
			.expect("loan issue should not fail");
		activate_test_loan_with_rate::<T>(pool_id, loan_id, loan_owner, rates);
	}
	collateral
}

fn interest_rate(rate: u32) -> Rate {
	// denominator here needs to be greater than the total number of
	// rates, and not so large as to create fractions that are too
	// small for the interest-accrual validation.
	Rate::saturating_from_rational(rate, 5000)
}

benchmarks! {
	where_clause {
		where
		T: pallet_pool_system::Config<
			CurrencyId = CurrencyId,
			Balance = u128,
		>,
		<T as pallet_uniques::Config>::CollectionId: From<u64>,
		<T as pallet_balances::Config>::Balance: From<u128>,
		<T as LoanConfig>::Rate: From<Rate>,
		<T as LoanConfig>::LoanId: From<u32>,
		<T as LoanConfig>::Balance: From<u128>,
		<T as ORMLConfig>::Balance: From<u128>,
		<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
		<T as TimestampConfig>::Moment: From<u64> + Into<u64>,
		<T as InterestAccrualConfig>::InterestRate: From<Rate>,
		<T as pallet_pool_system::Config>::Balance: From<u128>,
		<T as pallet_pool_system::Config>::CurrencyId: From<CurrencyId>,
		<T as pallet_pool_system::Config>::TrancheId: Into<[u8; 16]>,
		<T as pallet_pool_system::Config>::EpochId: From<u32>,
		<T as pallet_pool_system::Config>::PoolId: Into<u64> + IsType<PoolIdOf<T>>,
		<T as pallet_uniques::Config>::CollectionId: Default,
		<T as pallet_uniques::Config>::CollectionId: Default,
	}

	initialise_pool {
		let (pool_owner, pool_id, _loan_account, class_id) = create_and_init_pool::<T>(false);
	}:_(RawOrigin::Signed(pool_owner.clone()), pool_id, class_id)
	verify {
		let got_class_id = PoolToLoanNftClass::<T>::get(pool_id).expect("pool must be initialised");
		assert_eq!(class_id, got_class_id);
		let got_pool_id = LoanNftClassToPool::<T>::get(got_class_id).expect("nft class id must be initialised");
		assert_eq!(pool_id, got_pool_id);
	}

	create {
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, collateral) = create_asset::<T>(1.into());
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, collateral)
	verify {
		// assert loan issue event
		let loan_id: T::LoanId = 1u128.into();
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::Created { pool_id, loan_id, collateral }.into());

		// collateral owner must be pool account
		let pool_account = pool_account::<T>(pool_id.into());
		expect_asset_owner::<T>(collateral, pool_account);

		// loan owner must be caller
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, loan_owner);
	}

	price {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..<T as InterestAccrualConfig>::MaxRateCount::get();
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		populate_bench_storage::<T>(pool_id, n, m);
		// Worst case here - an already-priced loan (which
		// needs to be removed from the active list) at the
		// very end of the list.
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let loan_type = LoanType::BulletLoan(BulletLoan::new(
			// advance rate 80%
			Rate::saturating_from_rational(80, 100).into(),
			// probability of default is 4%
			Rate::saturating_from_rational(4, 100).into(),
			// loss given default is 50%
			Rate::saturating_from_rational(50, 100).into(),
			// collateral value
			(125 * CURRENCY).into(),
			// 4%
			math::interest_rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap().into(),
			// 2 years
			math::seconds_per_year() * 2,
		));
		let interest_rate_per_year: <T as pallet::Config>::Rate = Rate::saturating_from_rational(m, 5000).into();
		let interest_rate_per_sec: <T as pallet::Config>::Rate = math::interest_rate_per_sec(interest_rate_per_year).unwrap().into();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, interest_rate_per_year, loan_type)
	verify {
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::Priced { pool_id, loan_id, interest_rate_per_sec, loan_type }.into());
		let loan = Loan::<T>::get(pool_id, loan_id).expect("loan info should be present");
		let active_loan = LoansPallet::<T>::get_active_loan(pool_id, loan_id).expect("Active loan info should be present");
		assert_eq!(active_loan.loan_type, loan_type);
		assert_eq!(loan.status, LoanStatus::Active);
		assert_eq!(active_loan.interest_rate_per_sec, interest_rate_per_sec);
	}

	add_write_off_group {
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let write_off_group = WriteOffGroupInput {
			// 10%
			percentage: Rate::saturating_from_rational(10, 100).into(),
			penalty_interest_rate_per_year: Rate::saturating_from_rational(1, 100).into(),
			overdue_days: 3
		};
	}:_(RawOrigin::Signed(risk_admin::<T>()), pool_id, write_off_group)
	verify {
		let write_off_group_index = 0u32;
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::WriteOffGroupAdded { pool_id, write_off_group_index }.into());
	}

	initial_borrow {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..<T as InterestAccrualConfig>::MaxRateCount::get();
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		populate_bench_storage::<T>(pool_id, n, m);
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
	}:borrow(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::Borrowed { pool_id, loan_id, amount }.into());
		// pool reserve should have 100 USD less = 900 USD
		let pool_reserve_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (900 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::AUSD, &pool_reserve_account, pool_reserve_balance);

		// loan owner should have 100 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (100 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::AUSD, &loan_owner, loan_owner_balance);
	}

	further_borrows {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..<T as InterestAccrualConfig>::MaxRateCount::get();
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		populate_bench_storage::<T>(pool_id, n, m);
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (50 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 1 year
		let now = TimestampPallet::<T>::get().into();
		let after_one_year = now + math::seconds_per_year() * 1000;
		let amount = (40 * CURRENCY).into();
		set_block_number_timestamp::<T>(One::one(), after_one_year.into());
		InterestAccrualPallet::<T>::on_initialize(0u32.into());
	}:borrow(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::Borrowed { pool_id, loan_id, amount }.into());
		// pool reserve should have 100 USD less = 900 USD
		let pool_reserve_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (910 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::AUSD, &pool_reserve_account, pool_reserve_balance);

		// loan owner should have 100 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (90 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::AUSD, &loan_owner, loan_owner_balance);
	}

	repay {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..<T as InterestAccrualConfig>::MaxRateCount::get();
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		populate_bench_storage::<T>(pool_id, n, m);
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + math::seconds_per_day()) * 1000;
		set_block_number_timestamp::<T>(One::one(), after_maturity.into());
		InterestAccrualPallet::<T>::on_initialize(0u32.into());
		let amount = (100 * CURRENCY).into();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::Repaid { pool_id, loan_id, amount }.into());
		// pool reserve should have 1000 USD
		let pool_reserve_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::AUSD, &pool_reserve_account, pool_reserve_balance);

		// loan owner should have 0 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (0 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::AUSD, &loan_owner, loan_owner_balance);

		// current debt should not be zero
		let loan = Loan::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan.status, LoanStatus::Active);
	}

	write_off {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..T::MaxWriteOffGroups::get();
		let o in 1..(<T as InterestAccrualConfig>::MaxRateCount::get() - 1);
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		populate_bench_storage::<T>(pool_id, n, o);
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let risk_admin = risk_admin::<T>();
		for i in 0..m {
			let percentage: <T as pallet::Config>::Rate = Rate::saturating_from_rational(i+1, m).into();
			let penalty_interest_rate_per_year = Rate::saturating_from_rational((2*i + 1) * 10000, 2*m)
				.trunc()
				.checked_div(&Rate::saturating_from_integer(10000))
				.expect("Rate is an integer after `trunc`. div by 10000 is safe")
				.into();
			let overdue_days = percentage.checked_mul_int(120).unwrap();
			let write_off_group = WriteOffGroupInput {
				percentage, penalty_interest_rate_per_year, overdue_days
			};
			LoansPallet::<T>::add_write_off_group(RawOrigin::Signed(risk_admin.clone()).into(), pool_id, write_off_group).expect("adding write off groups should not fail");
		}
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + 130 * math::seconds_per_day()) * 1000;
		// add write off groups
		set_block_number_timestamp::<T>(One::one(), after_maturity.into());
		InterestAccrualPallet::<T>::on_initialize(0u32.into());
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		let index = (m-1).into();
		let percentage = Rate::saturating_from_rational(100, 100).into();
		let penalty_interest_rate_per_year = Rate::saturating_from_rational((2*m - 1) * 10000, 2*m)
			.trunc()
			.checked_div(&Rate::saturating_from_integer(10000))
			.expect("Rate is an integer after `trunc`. div by 10000 is safe")
			.into();
		let penalty_interest_rate_per_sec = math::penalty_interest_rate_per_sec(penalty_interest_rate_per_year).expect("Rate should be convertible to per-sec");
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::WrittenOff { pool_id, loan_id, percentage, penalty_interest_rate_per_sec, write_off_group_index: Some(index) }.into());
		let active_loan = LoansPallet::<T>::get_active_loan(pool_id, loan_id).unwrap();
		assert_eq!(active_loan.write_off_status, WriteOffStatus::WrittenOff{write_off_index: index})
	}

	admin_write_off {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..(<T as InterestAccrualConfig>::MaxRateCount::get() - 1);
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		populate_bench_storage::<T>(pool_id, n, m);
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + 130 * math::seconds_per_day()) * 1000;
		// add write off groups
		set_block_number_timestamp::<T>(One::one(), after_maturity.into());
		InterestAccrualPallet::<T>::on_initialize(0u32.into());
		let percentage = Rate::saturating_from_rational(100, 100).into();
		let penalty_interest_rate_per_year = Rate::saturating_from_rational(1, 100).into();
		let penalty_interest_rate_per_sec = math::penalty_interest_rate_per_sec(penalty_interest_rate_per_year).expect("Rate should be convertible to per-second");
	}:_(RawOrigin::Signed(risk_admin::<T>()), pool_id, loan_id, percentage, penalty_interest_rate_per_year)
	verify {
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::WrittenOff { pool_id, loan_id, percentage, penalty_interest_rate_per_sec, write_off_group_index: None }.into());
		let active_loan = LoansPallet::<T>::get_active_loan(pool_id, loan_id).unwrap();
		assert_eq!(active_loan.write_off_status, WriteOffStatus::WrittenOffByAdmin{percentage, penalty_interest_rate_per_sec});
	}

	repay_and_close {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..<T as InterestAccrualConfig>::MaxRateCount::get();
		let (_pool_owner, pool_id, _loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let collateral = populate_bench_storage::<T>(pool_id, n, m).unwrap();
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2 year
		let now = TimestampPallet::<T>::get().into();
		let after_two_years = now + 2 * math::seconds_per_year() * 1000;
		set_block_number_timestamp::<T>(One::one(), after_two_years.into());
		InterestAccrualPallet::<T>::on_initialize(0u32.into());
		// repay all. sent more than current debt
		let owner_balance: <T as ORMLConfig>::Balance = (1000 * CURRENCY).into();
		make_free_token_balance::<T>(CurrencyId::AUSD, &loan_owner, owner_balance);
		let amount = (200 * CURRENCY).into();
		LoansPallet::<T>::repay(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("repay should not fail");
	}:close(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::Closed { pool_id, loan_id, collateral }.into());
		// pool reserve should have more 1000 USD. this is with interest
		let pool_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CURRENCY).into();
		assert!(get_free_token_balance::<T>(CurrencyId::AUSD, &pool_account) > pool_reserve_balance);

		// Loan should be closed
		let loan = Loan::<T>::get(pool_id, loan_id).expect("loan info should be present");
		match loan.status {
			LoanStatus::Closed { closed_at: _ } => (),
			_ => assert!(false, "Loan status should be Closed"),
		}

		// asset owner must be loan owner
		expect_asset_owner::<T>(collateral, loan_owner);

		// loan nft owner is pool account
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_to_be_burned::<T>(loan_asset);
	}

	write_off_and_close {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..(<T as InterestAccrualConfig>::MaxRateCount::get() - 1);
		let (_pool_owner, pool_id, _loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let collateral = populate_bench_storage::<T>(pool_id, n, m).unwrap();
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2 year
		let now = TimestampPallet::<T>::get().into();
		let after_two_years = now + (2 * math::seconds_per_year() + 130 * math::seconds_per_day()) * 1000;
		set_block_number_timestamp::<T>(One::one(), after_two_years.into());
		InterestAccrualPallet::<T>::on_initialize(0u32.into());
		// add write off groups
		add_test_write_off_groups::<T>(pool_id, risk_admin::<T>());
		// write off loan. the loan will be moved to full write off after 120 days beyond maturity based on the test write off groups
		LoansPallet::<T>::write_off(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id).expect("write off should not fail");
	}:close(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::Closed { pool_id, loan_id, collateral }.into());
		// pool reserve should have 900 USD since loan is written off 100%
		let pool_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (900 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::AUSD, &pool_account, pool_reserve_balance);

		// Loan should be closed
		let loan = Loan::<T>::get(pool_id, loan_id).expect("loan info should be present");
		match loan.status {
			LoanStatus::Closed { closed_at: _ } => (),
			_ => assert!(false, "Loan status should be Closed"),
		}

		// asset owner must be loan owner
		expect_asset_owner::<T>(collateral, loan_owner);

		// loan nft owner is pool account
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_to_be_burned::<T>(loan_asset);
	}

	update_nav {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let m in 1..<T as InterestAccrualConfig>::MaxRateCount::get();
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		// Populate interest rates for worst-case lookup of our actual rate
		for idx in 1..m {
			let rate = Rate::saturating_from_rational(idx+1, 5000).into();
			InterestAccrualPallet::<T>::reference_yearly_rate(rate)
				.expect("Must be able to reference dummy interest rates");
		}
		let amount = (CURRENCY / 4).into();

		// Special case compared to the normal setup fn.
		// we need to borrow each loan so we have a non-zero NAV amount.
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, asset) = create_asset::<T>(loan_id);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
			activate_test_loan_with_rate::<T>(pool_id, loan_id, loan_owner.clone(), m);
			LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		}
		let loan_owner = borrower::<T>();

		// set timestamp to around 1 year
		let now = TimestampPallet::<T>::get().into();
		let after_one_month = now + math::seconds_per_day() * 30 * 1000;
		set_block_number_timestamp::<T>(One::one(), after_one_month.into());
		InterestAccrualPallet::<T>::on_initialize(0u32.into());
		// add write off groups
		add_test_write_off_groups::<T>(pool_id, risk_admin::<T>());
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id)
	verify {
		let pool_nav = PoolNAV::<T>::get(pool_id).expect("pool nav should be present");
		// updated time should be after_one_years
		assert_eq!(pool_nav.last_updated, after_one_month/1000);
		assert_last_event::<T, <T as LoanConfig>::RuntimeEvent>(LoanEvent::NAVUpdated { pool_id, nav: pool_nav.latest, update_type: NAVUpdateType::Exact }.into());
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::Runtime,
);
