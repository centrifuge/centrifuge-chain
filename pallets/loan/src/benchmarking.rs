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
use crate::loan_type::BulletLoan;
use crate::test_utils::initialise_test_pool;
use crate::types::WriteOffGroup;
use crate::{Config as LoanConfig, Event as LoanEvent, Pallet as LoanPallet};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::sp_runtime::traits::Zero;
use frame_support::traits::tokens::fungibles::{Inspect, Unbalanced};
use frame_support::traits::{Currency, UnfilteredDispatchable};
use frame_system::RawOrigin;
use orml_tokens::{Config as ORMLConfig, Pallet as ORMLPallet};
use pallet_balances::Pallet as BalancePallet;
use pallet_pool::CurrencyIdOf;
use pallet_timestamp::{Config as TimestampConfig, Pallet as TimestampPallet};
use runtime_common::{Amount, Rate, CFG};
use test_utils::{
	assert_last_event, create_nft_class, create_pool, expect_asset_owner, mint_nft,
	GetUSDCurrencyId,
};

pub struct Pallet<T: Config>(LoanPallet<T>);

pub trait Config:
	LoanConfig<ClassId = <Self as pallet_uniques::Config>::ClassId>
	+ pallet_balances::Config
	+ pallet_uniques::Config
	+ pallet_pool::Config
	+ ORMLConfig
	+ TimestampConfig
{
}

fn make_free_cfg_balance<T>(account: T::AccountId)
where
	T: Config + pallet_balances::Config,
	<T as pallet_balances::Config>::Balance: From<u128>,
{
	let min_balance: <T as pallet_balances::Config>::Balance = (10u128 * CFG).into();
	let _ = BalancePallet::<T>::make_free_balance_be(&account, min_balance);
}

fn make_free_token_balance<T, GetCurrencyId>(
	account: &T::AccountId,
	balance: <T as ORMLConfig>::Balance,
) where
	T: Config + ORMLConfig,
	GetCurrencyId: Get<<T as ORMLConfig>::CurrencyId>,
{
	ORMLPallet::<T>::set_balance(GetCurrencyId::get(), account, balance)
		.expect("should not fail to set new token balance");
}

fn check_free_token_balance<T, GetCurrencyId>(
	account: &T::AccountId,
	balance: <T as ORMLConfig>::Balance,
) where
	T: Config + ORMLConfig,
	GetCurrencyId: Get<<T as ORMLConfig>::CurrencyId>,
{
	assert_eq!(
		ORMLPallet::<T>::balance(GetCurrencyId::get(), account),
		balance
	);
}

fn get_free_token_balance<T, GetCurrencyId>(account: &T::AccountId) -> <T as ORMLConfig>::Balance
where
	T: Config + ORMLConfig,
	GetCurrencyId: Get<<T as ORMLConfig>::CurrencyId>,
{
	ORMLPallet::<T>::balance(GetCurrencyId::get(), account)
}

fn whitelist_acc<T: frame_system::Config>(acc: &T::AccountId) {
	frame_benchmarking::benchmarking::add_to_whitelist(
		frame_system::Account::<T>::hashed_key_for(acc).into(),
	);
}

fn create_and_init_pool<T: Config>() -> (
	T::AccountId,
	PoolIdOf<T>,
	T::AccountId,
	<T as LoanConfig>::ClassId,
)
where
	<T as pallet_balances::Config>::Balance: From<u128>,
	CurrencyIdOf<T>: From<u32>,
	PoolIdOf<T>: From<<T as pallet_pool::Config>::PoolId>,
	<T as pallet_uniques::Config>::ClassId: From<u64>,
{
	// create pool
	let pool_owner = account::<T::AccountId>("owner", 0, 0);
	make_free_cfg_balance::<T>(pool_owner.clone());
	let pool_id: PoolIdOf<T> = create_pool::<T, GetUSDCurrencyId>(pool_owner.clone()).into();

	// initialise pool on loan
	let loan_account = LoanPallet::<T>::account_id();
	make_free_cfg_balance::<T>(loan_account.clone());
	let loan_class_id = initialise_test_pool::<T>(
		pool_id,
		1,
		T::AdminOrigin::successful_origin(),
		pool_owner.clone(),
		Some(loan_account.clone()),
	);

	whitelist_acc::<T>(&pool_owner);
	whitelist_acc::<T>(&loan_account);
	(pool_owner, pool_id, loan_account, loan_class_id)
}

fn create_asset<T: Config>() -> (T::AccountId, AssetOf<T>)
where
	<T as pallet_balances::Config>::Balance: From<u128>,
	<T as pallet_uniques::Config>::ClassId: From<u64>,
{
	// create asset
	let loan_owner = account::<T::AccountId>("caller", 0, 0);
	make_free_cfg_balance::<T>(loan_owner.clone());
	let asset_class_id = create_nft_class::<T>(2, loan_owner.clone(), None);
	let asset_instance_id = mint_nft::<T>(loan_owner.clone(), asset_class_id);
	let asset = Asset(asset_class_id, asset_instance_id);
	whitelist_acc::<T>(&loan_owner);
	(loan_owner, asset)
}

fn activate_test_loan_with_defaults<T: Config>(pool_id: PoolIdOf<T>, loan_id: T::LoanId)
where
	<T as LoanConfig>::Rate: From<Rate>,
	<T as LoanConfig>::Amount: From<Amount>,
{
	let loan_type = LoanType::BulletLoan(BulletLoan::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100).into(),
		// expected loss over asset maturity 0.15%
		Rate::saturating_from_rational(15, 10000).into(),
		// collateral value
		Amount::from_inner(125 * CFG).into(),
		// 4%
		math::rate_per_sec(Rate::saturating_from_rational(4, 100))
			.unwrap()
			.into(),
		// 2 years
		math::seconds_per_year() * 2,
	));
	// interest rate is 5%
	let rp: T::Rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100))
		.unwrap()
		.into();
	let origin = T::AdminOrigin::successful_origin();
	LoanPallet::<T>::activate_loan(origin, pool_id, loan_id, rp, loan_type)
		.expect("loan activation should not fail");
}

fn add_test_write_off_groups<T: Config>(pool_id: PoolIdOf<T>)
where
	<T as LoanConfig>::Rate: From<Rate>,
{
	for group in vec![(3, 10), (5, 15), (7, 20), (20, 30), (120, 100)] {
		LoanPallet::<T>::add_write_off_group_to_pool(
			T::AdminOrigin::successful_origin(),
			pool_id,
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(group.1, 100).into(),
				overdue_days: group.0,
			},
		)
		.expect("adding write off groups should not fail");
	}
}

benchmarks! {
	where_clause {
		where
		<T as pallet_uniques::Config>::ClassId: From<u64>,
		<T as pallet_balances::Config>::Balance: From<u128>,
		CurrencyIdOf<T>: From<u32>,
		<T as LoanConfig>::Rate: From<Rate>,
		<T as LoanConfig>::Amount: From<Amount>,
		PoolIdOf<T>: From<<T as pallet_pool::Config>::PoolId>,
		<T as ORMLConfig>::Balance: From<u128>,
		<T as ORMLConfig>::CurrencyId: From<u32>,
		<T as TimestampConfig>::Moment: From<u64> + Into<u64>,
	}

	initialise_pool {
		let origin = T::AdminOrigin::successful_origin();
		let pool_id: PoolIdOf<T> = Default::default();
		let class_id: <T as LoanConfig>::ClassId = Default::default();
		let call = Call::<T>::initialise_pool(pool_id, class_id);
	}:{ call.dispatch_bypass_filter(origin)? }
	verify {
		let got_class_id = PoolToLoanNftClass::<T>::get(pool_id).expect("pool must be initialised");
		assert_eq!(class_id, got_class_id);
		let got_pool_id = LoanNftClassToPool::<T>::get(got_class_id).expect("nft class id must be initialised");
		assert_eq!(pool_id, got_pool_id);
	}

	issue_loan {
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, asset)
	verify {
		// assert loan issue event
		let loan_id: T::LoanId = 1u128.into();
		assert_last_event::<T>(LoanEvent::LoanIssued(pool_id, loan_id, asset).into());

		// asset owner must be loan account
		expect_asset_owner::<T>(asset, loan_account);

		// loan owner must be caller
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, loan_owner);
	}

	activate_loan {
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_type = LoanType::BulletLoan(BulletLoan::new(
			// advance rate 80%
			Rate::saturating_from_rational(80, 100).into(),
			// expected loss over asset maturity 0.15%
			Rate::saturating_from_rational(15, 10000).into(),
			// collateral value
			Amount::from_inner(125 * CFG).into(),
			// 4%
			math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap().into(),
			// 2 years
			math::seconds_per_year() * 2,
		));
		// interest rate is 5%
		let rp: T::Rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap().into();
		let origin = T::AdminOrigin::successful_origin();
		let loan_id: T::LoanId = 1u128.into();
		let call = Call::<T>::activate_loan(pool_id, loan_id, rp, loan_type);
	}:{ call.dispatch_bypass_filter(origin)? }
	verify {
		assert_last_event::<T>(LoanEvent::LoanActivated(pool_id, loan_id).into());
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.loan_type, loan_type);
		assert_eq!(loan_info.status, LoanStatus::Active);
		assert_eq!(loan_info.rate_per_sec, rp);
	}

	add_write_off_group_to_pool {
		let origin = T::AdminOrigin::successful_origin();
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>();
		let write_off_group = WriteOffGroup {
			// 10%
			percentage: Rate::saturating_from_rational(10, 100).into(),
			overdue_days: 3
		};
		let call = Call::<T>::add_write_off_group_to_pool(pool_id, write_off_group);
	}:{ call.dispatch_bypass_filter(origin)? }
	verify {
		let index = 0u32;
		assert_last_event::<T>(LoanEvent::WriteOffGroupAdded(pool_id, index).into());
	}

	initial_borrow {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id);
		// add some balance to pool reserve
		let pool_reserve_account: T::AccountId = pallet_pool::Pallet::<T>::account_id();
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);
		let amount = Amount::from_inner(100 * CFG).into();
	}:borrow(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T>(LoanEvent::LoanAmountBorrowed(pool_id, loan_id, amount).into());
		// pool reserve should have 100 USD less = 900 USD
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (900 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);

		// loan owner should have 100 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (100 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&loan_owner, loan_owner_balance);
	}

	further_borrows {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id);
		// add some balance to pool reserve
		let pool_reserve_account: T::AccountId = pallet_pool::Pallet::<T>::account_id();
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);
		let amount = Amount::from_inner(50 * CFG).into();
		LoanPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 1 year
		let now = TimestampPallet::<T>::get().into();
		let after_one_year = now + math::seconds_per_year();
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_one_year.into()).expect("timestamp set should not fail");
	}:borrow(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T>(LoanEvent::LoanAmountBorrowed(pool_id, loan_id, amount).into());
		// pool reserve should have 100 USD less = 900 USD
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (900 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);

		// loan owner should have 100 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (100 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&loan_owner, loan_owner_balance);
	}

	repay_before_maturity {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id);
		// add some balance to pool reserve
		let pool_reserve_account: T::AccountId = pallet_pool::Pallet::<T>::account_id();
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);
		let amount = Amount::from_inner(100 * CFG).into();
		LoanPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 1 year
		let now = TimestampPallet::<T>::get().into();
		let after_one_year = now + math::seconds_per_year();
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_one_year.into()).expect("timestamp set should not fail");
		let amount = Amount::from_inner(100 * CFG).into();
	}:repay(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T>(LoanEvent::LoanAmountRepaid(pool_id, loan_id, amount).into());
		// pool reserve should have 1000 USD
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);

		// loan owner should have 0 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (0 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&loan_owner, loan_owner_balance);

		// current debt should not be zero
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.status, LoanStatus::Active);
		assert!(loan_info.present_value().unwrap() > Zero::zero());
	}

	repay_after_maturity {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id);
		// add some balance to pool reserve
		let pool_reserve_account: T::AccountId = pallet_pool::Pallet::<T>::account_id();
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);
		let amount = Amount::from_inner(100 * CFG).into();
		LoanPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + 2 * math::seconds_per_year() + math::seconds_per_day();
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
		let amount = Amount::from_inner(100 * CFG).into();
	}:repay(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T>(LoanEvent::LoanAmountRepaid(pool_id, loan_id, amount).into());
		// pool reserve should have 1000 USD
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);

		// loan owner should have 0 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (0 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&loan_owner, loan_owner_balance);

		// current debt should not be zero
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.status, LoanStatus::Active);
		assert!(loan_info.present_value().unwrap() > Zero::zero());
	}

	write_off_loan {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id);
		// add some balance to pool reserve
		let pool_reserve_account: T::AccountId = pallet_pool::Pallet::<T>::account_id();
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);
		let amount = Amount::from_inner(100 * CFG).into();
		LoanPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + 2 * math::seconds_per_year() + 130 * math::seconds_per_day();
		// add write off groups
		add_test_write_off_groups::<T>(pool_id);
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		let index = 4u32;
		assert_last_event::<T>(LoanEvent::LoanWrittenOff(pool_id, loan_id, index).into());
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.write_off_index, Some(index));
		assert!(!loan_info.admin_written_off);
	}

	admin_write_off_loan {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id);
		// add some balance to pool reserve
		let pool_reserve_account: T::AccountId = pallet_pool::Pallet::<T>::account_id();
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);
		let amount = Amount::from_inner(100 * CFG).into();
		LoanPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + 2 * math::seconds_per_year() + 130 * math::seconds_per_day();
		// add write off groups
		add_test_write_off_groups::<T>(pool_id);
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
		let index = 4u32;
		let call = Call::<T>::admin_write_off_loan(pool_id, loan_id, index);
		let origin = T::AdminOrigin::successful_origin();
	}:{ call.dispatch_bypass_filter(origin)? }
	verify {
		assert_last_event::<T>(LoanEvent::LoanWrittenOff(pool_id, loan_id, index).into());
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.write_off_index, Some(index));
		assert!(loan_info.admin_written_off);
	}

	repay_and_close {
		let (_pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id);
		// add some balance to pool reserve
		let pool_reserve_account: T::AccountId = pallet_pool::Pallet::<T>::account_id();
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);
		let amount = Amount::from_inner(100 * CFG).into();
		LoanPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2 year
		let now = TimestampPallet::<T>::get().into();
		let after_two_years = now + 2 * math::seconds_per_year();
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_two_years.into()).expect("timestamp set should not fail");
		// repay all. sent more than current debt
		let owner_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&loan_owner, owner_balance);
		let amount = Amount::from_inner(200 * CFG).into();
		LoanPallet::<T>::repay(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("repay should not fail");
	}:close_loan(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		assert_last_event::<T>(LoanEvent::LoanClosed(pool_id, loan_id, asset).into());
		// pool reserve should have more 1000 USD. this is with interest
		assert!(get_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account) > pool_reserve_balance);

		// Loan should be closed
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.status, LoanStatus::Closed);

		// asset owner must be loan owner
		expect_asset_owner::<T>(asset, loan_owner);

		// loan nft owner is loan account
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, loan_account);
	}

	write_off_and_close {
		let (_pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>();
		let (loan_owner, asset) = create_asset::<T>();
		LoanPallet::<T>::issue_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id);
		// add some balance to pool reserve
		let pool_reserve_account: T::AccountId = pallet_pool::Pallet::<T>::account_id();
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CFG).into();
		make_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);
		let amount = Amount::from_inner(100 * CFG).into();
		LoanPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2 year
		let now = TimestampPallet::<T>::get().into();
		let after_two_years = now + 2 * math::seconds_per_year() + 130 * math::seconds_per_day();
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_two_years.into()).expect("timestamp set should not fail");
		// add write off groups
		add_test_write_off_groups::<T>(pool_id);
		// write off loan. the loan will be moved to full write off after 120 days beyond maturity based on the test write off groups
		LoanPallet::<T>::write_off_loan(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id).expect("write off should not fail");
	}:close_loan(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		assert_last_event::<T>(LoanEvent::LoanClosed(pool_id, loan_id, asset).into());
		// pool reserve should have 900 USD since loan is written off 100%
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (900 * CFG).into();
		check_free_token_balance::<T, GetUSDCurrencyId>(&pool_reserve_account, pool_reserve_balance);

		// Loan should be closed
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.status, LoanStatus::Closed);

		// asset owner must be loan owner
		expect_asset_owner::<T>(asset, loan_owner);

		// loan nft owner is loan account
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, loan_account);
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::MockRuntime,
);
