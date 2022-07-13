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
use super::*;
use crate::loan_type::{BulletLoan, CreditLineWithMaturity};
use crate::test_utils::initialise_test_pool;
use crate::types::WriteOffGroup;
use crate::{Config as LoanConfig, Event as LoanEvent, Pallet as LoansPallet};
use common_types::{CurrencyId, PoolLocator};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::assert_ok;
use frame_support::traits::tokens::fungibles::Inspect;
use frame_support::traits::{Currency, IsType};
use frame_system::RawOrigin;
use orml_tokens::{Config as ORMLConfig, Pallet as ORMLPallet};
use orml_traits::MultiCurrency;
use pallet_balances::Pallet as BalancePallet;
use pallet_timestamp::{Config as TimestampConfig, Pallet as TimestampPallet};
use runtime_common::{Rate, CFG as CURRENCY};
use test_utils::{
	assert_last_event, create as create_test_pool, create_nft_class_if_needed, expect_asset_owner,
	expect_asset_to_be_burned, get_tranche_id, mint_nft_of,
};

pub struct Pallet<T: Config>(LoansPallet<T>);

pub trait Config:
	LoanConfig<ClassId = <Self as pallet_uniques::Config>::CollectionId>
	+ pallet_balances::Config
	+ pallet_uniques::Config
	+ pallet_pools::Config
	+ ORMLConfig
	+ TimestampConfig
{
}

#[cfg(test)]
impl Config for super::mock::MockRuntime {}

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

// return white listed senior and junior tranche investors
fn investors<T: frame_system::Config>() -> (T::AccountId, T::AccountId) {
	let senior_investor = account::<T::AccountId>("senior", 0, 0);
	let junior_investor = account::<T::AccountId>("junior", 0, 0);
	whitelist_acc::<T>(&senior_investor);
	whitelist_acc::<T>(&junior_investor);
	(senior_investor, junior_investor)
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
	<T as pallet_pools::Config>::Balance: From<u128>,
	<T as pallet_pools::Config>::CurrencyId: From<CurrencyId>,
	<T as pallet_pools::Config>::TrancheId: Into<[u8; 16]>,
	<T as pallet_pools::Config>::EpochId: From<u32>,
	<T as pallet_pools::Config>::PoolId: Into<u64> + IsType<PoolIdOf<T>>,
	<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
	<T as ORMLConfig>::Balance: From<u128>,
	<T as pallet_uniques::Config>::CollectionId: Default,
{
	// create pool
	let pool_owner = account::<T::AccountId>("owner", 0, 0);
	make_free_cfg_balance::<T>(pool_owner.clone());
	let (senior_inv, junior_inv) = investors::<T>();
	make_free_cfg_balance::<T>(senior_inv.clone());
	make_free_cfg_balance::<T>(junior_inv.clone());
	make_free_token_balance::<T>(CurrencyId::AUSD, &senior_inv, (500 * CURRENCY).into());
	make_free_token_balance::<T>(CurrencyId::AUSD, &junior_inv, (500 * CURRENCY).into());
	let pool_id: PoolIdOf<T> = Default::default();
	let pool_account = pool_account::<T>(pool_id.into());
	let pal_pool_id: T::PoolId = pool_id.into();
	create_test_pool::<T>(
		pool_id.into(),
		pool_owner.clone(),
		junior_inv,
		senior_inv,
		CurrencyId::AUSD,
	);
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

	// add borrower role and price admin and risk admin role
	make_free_cfg_balance::<T>(borrower::<T>());
	make_free_cfg_balance::<T>(risk_admin::<T>());
	assert_ok!(<T as pallet_pools::Config>::Permission::add(
		PermissionScope::Pool(pool_id.into()),
		borrower::<T>(),
		Role::PoolRole(PoolRole::Borrower)
	));
	assert_ok!(<T as pallet_pools::Config>::Permission::add(
		PermissionScope::Pool(pool_id.into()),
		borrower::<T>(),
		Role::PoolRole(PoolRole::PricingAdmin)
	));
	assert_ok!(<T as pallet_pools::Config>::Permission::add(
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

fn activate_test_loan_with_defaults<T: Config>(
	pool_id: PoolIdOf<T>,
	loan_id: T::LoanId,
	borrower: T::AccountId,
) where
	<T as LoanConfig>::Rate: From<Rate>,
	<T as LoanConfig>::Balance: From<u128>,
{
	// Note: Originally this was 5%. The with_rate version uses 5000
	// as the denominator, so our numerator is 250
	activate_test_loan_with_rate::<T>(pool_id, loan_id, borrower, 250)
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
	let rp: T::Rate = math::interest_rate_per_sec(Rate::saturating_from_rational(rate, 5000))
		.unwrap()
		.into();
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
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(group.1, 100).into(),
				penalty_interest_rate_per_sec: Rate::saturating_from_rational(1, 100).into(),
				overdue_days: group.0,
			},
		)
		.expect("adding write off groups should not fail");
	}
}

fn pool_account<T: pallet_pools::Config>(pool_id: T::PoolId) -> T::AccountId {
	PoolLocator { pool_id }.into_account_truncating()
}

benchmarks! {
	where_clause {
		where
		<T as pallet_uniques::Config>::CollectionId: From<u64>,
		<T as pallet_balances::Config>::Balance: From<u128>,
		<T as LoanConfig>::Rate: From<Rate>,
		<T as LoanConfig>::LoanId: From<u32>,
		<T as LoanConfig>::Balance: From<u128>,
		<T as ORMLConfig>::Balance: From<u128>,
		<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
		<T as TimestampConfig>::Moment: From<u64> + Into<u64>,
		<T as pallet_pools::Config>::Balance: From<u128>,
		<T as pallet_pools::Config>::CurrencyId: From<CurrencyId>,
		<T as pallet_pools::Config>::TrancheId: Into<[u8; 16]>,
		<T as pallet_pools::Config>::EpochId: From<u32>,
		<T as pallet_pools::Config>::PoolId: Into<u64> + IsType<PoolIdOf<T>>,
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
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Created { pool_id, loan_id, collateral }.into());

		// collateral owner must be pool account
		let pool_account = pool_account::<T>(pool_id.into());
		expect_asset_owner::<T>(collateral, pool_account);

		// loan owner must be caller
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, loan_owner);
	}

	price {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, asset) = create_asset::<T>(loan_id);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
			activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner);
		}
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
		// interest rate is 5%
		let interest_rate_per_sec: T::Rate = math::interest_rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap().into();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, interest_rate_per_sec, loan_type)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Priced { pool_id, loan_id, interest_rate_per_sec, loan_type }.into());
		let loan = Loan::<T>::get(pool_id, loan_id).expect("loan info should be present");
		let active_loan = LoansPallet::<T>::get_active_loan(pool_id, loan_id).expect("Active loan info should be present");
		assert_eq!(active_loan.loan_type, loan_type);
		assert_eq!(loan.status, LoanStatus::Active);
		assert_eq!(active_loan.interest_rate_per_sec, interest_rate_per_sec);
	}

	add_write_off_group {
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let write_off_group = WriteOffGroup {
			// 10%
			percentage: Rate::saturating_from_rational(10, 100).into(),
			penalty_interest_rate_per_sec: Rate::saturating_from_rational(1, 100).into(),
			overdue_days: 3
		};
	}:_(RawOrigin::Signed(risk_admin::<T>()), pool_id, write_off_group)
	verify {
		let write_off_group_index = 0u32;
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::WriteOffGroupAdded { pool_id, write_off_group_index }.into());
	}

	initial_borrow {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, asset) = create_asset::<T>(loan_id);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
			activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner);
		}
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
	}:borrow(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Borrowed { pool_id, loan_id, amount }.into());
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
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, asset) = create_asset::<T>(loan_id);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
			activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner);
		}
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (50 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 1 year
		let now = TimestampPallet::<T>::get().into();
		let after_one_year = now + math::seconds_per_year() * 1000;
		let amount = (40 * CURRENCY).into();
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_one_year.into()).expect("timestamp set should not fail");
	}:borrow(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Borrowed { pool_id, loan_id, amount }.into());
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
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, asset) = create_asset::<T>(loan_id);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
			activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner);
		}
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + math::seconds_per_day()) * 1000;
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
		let amount = (100 * CURRENCY).into();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Repaid { pool_id, loan_id, amount }.into());
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
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, asset) = create_asset::<T>(loan_id);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
			activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner);
		}
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let risk_admin = risk_admin::<T>();
		for i in 0..m {
			let percentage: T::Rate = Rate::saturating_from_rational(i+1, m).into();
			let penalty_interest_rate_per_sec = Rate::saturating_from_rational(i+1, m).into();
			let overdue_days = percentage.checked_mul_int(120).unwrap();
			let write_off_group = WriteOffGroup {
				percentage, penalty_interest_rate_per_sec, overdue_days
			};
			LoansPallet::<T>::add_write_off_group(RawOrigin::Signed(risk_admin.clone()).into(), pool_id, write_off_group).expect("adding write off groups should not fail");
		}
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + 130 * math::seconds_per_day()) * 1000;
		// add write off groups
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		let index = (m-1).into();
		let percentage = Rate::saturating_from_rational(100, 100).into();
		let penalty_interest_rate_per_sec = Rate::saturating_from_rational(100, 100).into();
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::WrittenOff { pool_id, loan_id, percentage, penalty_interest_rate_per_sec, write_off_group_index: Some(index) }.into());
		let active_loan = LoansPallet::<T>::get_active_loan(pool_id, loan_id).unwrap();
		assert_eq!(active_loan.write_off_status, WriteOffStatus::WrittenOff{write_off_index: index})
	}

	admin_write_off {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, asset) = create_asset::<T>(loan_id);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
			activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner);
		}
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + 130 * math::seconds_per_day()) * 1000;
		// add write off groups
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
		let percentage = Rate::saturating_from_rational(100, 100).into();
		let penalty_interest_rate_per_sec = Rate::saturating_from_rational(1, 100).into();
	}:_(RawOrigin::Signed(risk_admin::<T>()), pool_id, loan_id, percentage, penalty_interest_rate_per_sec)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::WrittenOff { pool_id, loan_id, percentage, penalty_interest_rate_per_sec, write_off_group_index: None }.into());
		let active_loan = LoansPallet::<T>::get_active_loan(pool_id, loan_id).unwrap();
		assert_eq!(active_loan.write_off_status, WriteOffStatus::WrittenOffByAdmin{percentage, penalty_interest_rate_per_sec});
	}

	repay_and_close {
		let n in 1..T::MaxActiveLoansPerPool::get();
		let (_pool_owner, pool_id, _loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let mut collateral = None;
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, new_asset) = create_asset::<T>(loan_id);
			collateral = Some(new_asset);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, new_asset).expect("loan issue should not fail");
			activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner);

		}
		let collateral = collateral.unwrap();
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2 year
		let now = TimestampPallet::<T>::get().into();
		let after_two_years = now + 2 * math::seconds_per_year() * 1000;
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_two_years.into()).expect("timestamp set should not fail");
		// repay all. sent more than current debt
		let owner_balance: <T as ORMLConfig>::Balance = (1000 * CURRENCY).into();
		make_free_token_balance::<T>(CurrencyId::AUSD, &loan_owner, owner_balance);
		let amount = (200 * CURRENCY).into();
		LoansPallet::<T>::repay(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("repay should not fail");
	}:close(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Closed { pool_id, loan_id, collateral }.into());
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
		let (_pool_owner, pool_id, _loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let mut collateral = None;
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, new_collateral) = create_asset::<T>(loan_id);
			collateral = Some(new_collateral);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, new_collateral).expect("loan issue should not fail");
			activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner);
		}
		let collateral = collateral.unwrap();
		let loan_owner = borrower::<T>();
		let loan_id = n.into();
		let amount = (100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2 year
		let now = TimestampPallet::<T>::get().into();
		let after_two_years = now + (2 * math::seconds_per_year() + 130 * math::seconds_per_day()) * 1000;
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_two_years.into()).expect("timestamp set should not fail");
		// add write off groups
		add_test_write_off_groups::<T>(pool_id, risk_admin::<T>());
		// write off loan. the loan will be moved to full write off after 120 days beyond maturity based on the test write off groups
		LoansPallet::<T>::write_off(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id).expect("write off should not fail");
	}:close(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Closed { pool_id, loan_id, collateral }.into());
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
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		let amount = (CURRENCY / 4).into();
		for idx in 0..n {
			let loan_id = (idx + 1).into();
			let (loan_owner, asset) = create_asset::<T>(loan_id);
			LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
			activate_test_loan_with_rate::<T>(pool_id, loan_id, loan_owner.clone(), idx+1);
			LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		}
		let loan_owner = borrower::<T>();

		// set timestamp to around 1 year
		let now = TimestampPallet::<T>::get().into();
		let after_one_month = now + math::seconds_per_day() * 30 * 1000;
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_one_month.into()).expect("timestamp set should not fail");
		// add write off groups
		add_test_write_off_groups::<T>(pool_id, risk_admin::<T>());
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id)
	verify {
		let pool_nav = PoolNAV::<T>::get(pool_id).expect("pool nav should be present");
		// updated time should be after_one_years
		assert_eq!(pool_nav.last_updated, after_one_month/1000);
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::NAVUpdated { pool_id, nav: pool_nav.latest, update_type: NAVUpdateType::Exact }.into());
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::MockRuntime,
);
