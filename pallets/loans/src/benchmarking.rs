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
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::assert_ok;
use frame_support::sp_runtime::traits::Zero;
use frame_support::traits::tokens::fungibles::Inspect;
use frame_support::traits::{Currency, IsType};
use frame_system::RawOrigin;
use orml_tokens::{Config as ORMLConfig, Pallet as ORMLPallet};
use orml_traits::MultiCurrency;
use pallet_balances::Pallet as BalancePallet;
use pallet_pools::PoolLocator;
use pallet_timestamp::{Config as TimestampConfig, Pallet as TimestampPallet};
use primitives_tokens::CurrencyId;
use runtime_common::{Amount, Rate, CFG as CURRENCY};
use sp_runtime::traits::StaticLookup;
use sp_std::vec;
use test_utils::{assert_last_event, create, create_nft_class, expect_asset_owner, mint_nft};

pub struct Pallet<T: Config>(LoansPallet<T>);

pub trait Config:
	LoanConfig<ClassId = <Self as pallet_uniques::Config>::ClassId>
	+ pallet_balances::Config
	+ pallet_uniques::Config
	+ pallet_pools::Config
	+ ORMLConfig
	+ TimestampConfig
{
}

fn make_free_cfg_balance<T>(account: T::AccountId)
where
	T: Config + pallet_balances::Config,
	<T as pallet_balances::Config>::Balance: From<u128>,
{
	let min_balance: <T as pallet_balances::Config>::Balance = (10u128 * CURRENCY).into();
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
	<T as pallet_uniques::Config>::ClassId: From<u64>,
	<T as pallet_pools::Config>::Balance: From<u128>,
	<T as pallet_pools::Config>::CurrencyId: From<CurrencyId>,
	<T as pallet_pools::Config>::TrancheId: From<u8>,
	<T as pallet_pools::Config>::EpochId: From<u32>,
	<T as pallet_pools::Config>::PoolId: Into<u64> + IsType<PoolIdOf<T>>,
	<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
	<T as ORMLConfig>::Balance: From<u128>,
{
	// create pool
	let pool_owner = account::<T::AccountId>("owner", 0, 0);
	make_free_cfg_balance::<T>(pool_owner.clone());
	let (senior_inv, junior_inv) = investors::<T>();
	make_free_cfg_balance::<T>(senior_inv.clone());
	make_free_cfg_balance::<T>(junior_inv.clone());
	make_free_token_balance::<T>(CurrencyId::Usd, &senior_inv, (500 * CURRENCY).into());
	make_free_token_balance::<T>(CurrencyId::Usd, &junior_inv, (500 * CURRENCY).into());
	let pool_id: PoolIdOf<T> = Default::default();
	let pool_account = pool_account::<T>(pool_id.into());
	let pal_pool_id: T::PoolId = pool_id.into();
	make_free_token_balance::<T>(
		CurrencyId::Tranche(pal_pool_id.into(), 0u8.into()),
		&pool_account,
		(500 * CURRENCY).into(),
	);
	make_free_token_balance::<T>(
		CurrencyId::Tranche(pal_pool_id.into(), 1u8.into()),
		&pool_account,
		(500 * CURRENCY).into(),
	);
	create::<T>(
		pool_id.into(),
		pool_owner.clone(),
		junior_inv.clone(),
		senior_inv.clone(),
		CurrencyId::Usd,
	);

	// add borrower role and price admin and risk admin role
	make_free_cfg_balance::<T>(borrower::<T>());
	make_free_cfg_balance::<T>(risk_admin::<T>());
	assert_ok!(pallet_pools::Pallet::<T>::approve_role_for(
		RawOrigin::Signed(pool_owner.clone()).into(),
		pool_id.into(),
		PoolRole::Borrower,
		vec![<T::Lookup as StaticLookup>::unlookup(borrower::<T>())]
	));
	assert_ok!(pallet_pools::Pallet::<T>::approve_role_for(
		RawOrigin::Signed(pool_owner.clone()).into(),
		pool_id.into(),
		PoolRole::PricingAdmin,
		vec![<T::Lookup as StaticLookup>::unlookup(borrower::<T>())]
	));
	assert_ok!(pallet_pools::Pallet::<T>::approve_role_for(
		RawOrigin::Signed(pool_owner.clone()).into(),
		pool_id.into(),
		PoolRole::RiskAdmin,
		vec![<T::Lookup as StaticLookup>::unlookup(risk_admin::<T>())]
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

fn create_asset<T: Config + frame_system::Config>() -> (T::AccountId, AssetOf<T>)
where
	<T as pallet_balances::Config>::Balance: From<u128>,
	<T as pallet_uniques::Config>::ClassId: From<u64>,
{
	// create asset
	let loan_owner = borrower::<T>();
	make_free_cfg_balance::<T>(loan_owner.clone());
	let asset_class_id = create_nft_class::<T>(2, loan_owner.clone(), None);
	let asset_instance_id = mint_nft::<T>(loan_owner.clone(), asset_class_id);
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
	<T as LoanConfig>::Amount: From<Amount>,
{
	let loan_type = LoanType::CreditLineWithMaturity(CreditLineWithMaturity::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100).into(),
		// probability of default is 4%
		Rate::saturating_from_rational(4, 100).into(),
		// loss given default is 50%
		Rate::saturating_from_rational(50, 100).into(),
		// collateral value
		Amount::from_inner(125 * CURRENCY).into(),
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
	for group in vec![(3, 10), (5, 15), (7, 20), (20, 30), (120, 100)] {
		LoansPallet::<T>::add_write_off_group(
			RawOrigin::Signed(risk_admin.clone()).into(),
			pool_id,
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(group.1, 100).into(),
				overdue_days: group.0,
			},
		)
		.expect("adding write off groups should not fail");
	}
}

fn pool_account<T: pallet_pools::Config>(pool_id: T::PoolId) -> T::AccountId {
	PoolLocator { pool_id }.into_account()
}

benchmarks! {
	where_clause {
		where
		<T as pallet_uniques::Config>::ClassId: From<u64>,
		<T as pallet_balances::Config>::Balance: From<u128>,
		<T as LoanConfig>::Rate: From<Rate>,
		<T as LoanConfig>::Amount: From<Amount>,
		<T as ORMLConfig>::Balance: From<u128>,
		<T as ORMLConfig>::CurrencyId: From<CurrencyId>,
		<T as TimestampConfig>::Moment: From<u64> + Into<u64>,
		<T as pallet_pools::Config>::Balance: From<u128>,
		<T as pallet_pools::Config>::CurrencyId: From<CurrencyId>,
		<T as pallet_pools::Config>::TrancheId: From<u8>,
		<T as pallet_pools::Config>::EpochId: From<u32>,
		<T as pallet_pools::Config>::PoolId: Into<u64> + IsType<PoolIdOf<T>>,
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

	create_loan {
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, asset)
	verify {
		// assert loan issue event
		let loan_id: T::LoanId = 1u128.into();
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Created(pool_id, loan_id, asset).into());

		// asset owner must be loan account
		expect_asset_owner::<T>(asset, loan_account);

		// loan owner must be caller
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, loan_owner);
	}

	price_loan {
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_type = LoanType::BulletLoan(BulletLoan::new(
			// advance rate 80%
			Rate::saturating_from_rational(80, 100).into(),
			// probability of default is 4%
			Rate::saturating_from_rational(4, 100).into(),
			// loss given default is 50%
			Rate::saturating_from_rational(50, 100).into(),
			// collateral value
			Amount::from_inner(125 * CURRENCY).into(),
			// 4%
			math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap().into(),
			// 2 years
			math::seconds_per_year() * 2,
		));
		// interest rate is 5%
		let rp: T::Rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap().into();
		let loan_id: T::LoanId = 1u128.into();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, rp, loan_type)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Priced(pool_id, loan_id).into());
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.loan_type, loan_type);
		assert_eq!(loan_info.status, LoanStatus::Active);
		assert_eq!(loan_info.rate_per_sec, rp);
	}

	add_write_off_group {
		let (pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let write_off_group = WriteOffGroup {
			// 10%
			percentage: Rate::saturating_from_rational(10, 100).into(),
			overdue_days: 3
		};
	}:_(RawOrigin::Signed(risk_admin::<T>()), pool_id, write_off_group)
	verify {
		let index = 0u32;
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::WriteOffGroupAdded(pool_id, index).into());
	}

	initial_borrow {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner.clone());
		let amount = Amount::from_inner(100 * CURRENCY).into();
	}:borrow(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Borrowed(pool_id, loan_id, amount).into());
		// pool reserve should have 100 USD less = 900 USD
		let pool_reserve_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (900 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::Usd, &pool_reserve_account, pool_reserve_balance);

		// loan owner should have 100 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (100 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::Usd, &loan_owner, loan_owner_balance);
	}

	further_borrows {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner.clone());
		let amount = Amount::from_inner(50 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 1 year
		let now = TimestampPallet::<T>::get().into();
		let after_one_year = now + math::seconds_per_year() * 1000;
		let amount = Amount::from_inner(40 * CURRENCY).into();
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_one_year.into()).expect("timestamp set should not fail");
	}:borrow(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Borrowed(pool_id, loan_id, amount).into());
		// pool reserve should have 100 USD less = 900 USD
		let pool_reserve_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (910 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::Usd, &pool_reserve_account, pool_reserve_balance);

		// loan owner should have 100 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (90 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::Usd, &loan_owner, loan_owner_balance);
	}

	repay {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner.clone());
		let amount = Amount::from_inner(100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + math::seconds_per_day()) * 1000;
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
		let amount = Amount::from_inner(100 * CURRENCY).into();
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id, amount)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Repaid(pool_id, loan_id, amount).into());
		// pool reserve should have 1000 USD
		let pool_reserve_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::Usd, &pool_reserve_account, pool_reserve_balance);

		// loan owner should have 0 USD
		let loan_owner_balance: <T as ORMLConfig>::Balance = (0 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::Usd, &loan_owner, loan_owner_balance);

		// current debt should not be zero
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.status, LoanStatus::Active);
		assert!(loan_info.present_value(&vec![]).unwrap() > Zero::zero());
	}

	write_off_loan {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner.clone());
		let amount = Amount::from_inner(100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + 130 * math::seconds_per_day()) * 1000;
		// add write off groups
		add_test_write_off_groups::<T>(pool_id, risk_admin::<T>());
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
	}:_(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		let index = 4u32;
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::WrittenOff(pool_id, loan_id, index).into());
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.write_off_index, Some(index));
		assert!(!loan_info.admin_written_off);
	}

	admin_write_off_loan {
		let (_pool_owner, pool_id, _loan_account, _loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner.clone());
		let amount = Amount::from_inner(100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2+ years
		let now = TimestampPallet::<T>::get().into();
		let after_maturity = now + (2 * math::seconds_per_year() + 130 * math::seconds_per_day()) * 1000;
		// add write off groups
		add_test_write_off_groups::<T>(pool_id, risk_admin::<T>());
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_maturity.into()).expect("timestamp set should not fail");
		let index = 4u32;
	}:_(RawOrigin::Signed(risk_admin::<T>()), pool_id, loan_id, index)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::WrittenOff(pool_id, loan_id, index).into());
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.write_off_index, Some(index));
		assert!(loan_info.admin_written_off);
	}

	repay_and_close {
		let (_pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner.clone());
		let amount = Amount::from_inner(100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 2 year
		let now = TimestampPallet::<T>::get().into();
		let after_two_years = now + 2 * math::seconds_per_year() * 1000;
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_two_years.into()).expect("timestamp set should not fail");
		// repay all. sent more than current debt
		let owner_balance: <T as ORMLConfig>::Balance = (1000 * CURRENCY).into();
		make_free_token_balance::<T>(CurrencyId::Usd, &loan_owner, owner_balance);
		let amount = Amount::from_inner(200 * CURRENCY).into();
		LoansPallet::<T>::repay(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("repay should not fail");
	}:close(RawOrigin::Signed(loan_owner.clone()), pool_id, loan_id)
	verify {
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Closed(pool_id, loan_id, asset).into());
		// pool reserve should have more 1000 USD. this is with interest
		let pool_reserve_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (1000 * CURRENCY).into();
		assert!(get_free_token_balance::<T>(CurrencyId::Usd, &pool_reserve_account) > pool_reserve_balance);

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
		let (_pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner.clone());
		let amount = Amount::from_inner(100 * CURRENCY).into();
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
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::Closed(pool_id, loan_id, asset).into());
		// pool reserve should have 900 USD since loan is written off 100%
		let pool_reserve_account = pool_account::<T>(pool_id.into());
		let pool_reserve_balance: <T as ORMLConfig>::Balance = (900 * CURRENCY).into();
		check_free_token_balance::<T>(CurrencyId::Usd, &pool_reserve_account, pool_reserve_balance);

		// Loan should be closed
		let loan_info = LoanInfo::<T>::get(pool_id, loan_id).expect("loan info should be present");
		assert_eq!(loan_info.status, LoanStatus::Closed);

		// asset owner must be loan owner
		expect_asset_owner::<T>(asset, loan_owner);

		// loan nft owner is loan account
		let loan_asset = Asset(loan_class_id, loan_id);
		expect_asset_owner::<T>(loan_asset, loan_account);
	}

	nav_update_single_loan {
		let (_pool_owner, pool_id, loan_account, loan_class_id) = create_and_init_pool::<T>(true);
		let (loan_owner, asset) = create_asset::<T>();
		LoansPallet::<T>::create(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, asset).expect("loan issue should not fail");
		let loan_id: T::LoanId = 1u128.into();
		activate_test_loan_with_defaults::<T>(pool_id, loan_id, loan_owner.clone());
		let amount = Amount::from_inner(100 * CURRENCY).into();
		LoansPallet::<T>::borrow(RawOrigin::Signed(loan_owner.clone()).into(), pool_id, loan_id, amount).expect("borrow should not fail");
		// set timestamp to around 1 year
		let now = TimestampPallet::<T>::get().into();
		let after_one_year = now + 1 * math::seconds_per_year() * 1000;
		TimestampPallet::<T>::set(RawOrigin::None.into(), after_one_year.into()).expect("timestamp set should not fail");
		// add write off groups
		add_test_write_off_groups::<T>(pool_id, risk_admin::<T>());
	}:update_nav(RawOrigin::Signed(loan_owner.clone()), pool_id)
	verify {
		let pool_nav = PoolNAV::<T>::get(pool_id).expect("pool nav should be present");
		// pool nav should more than 100 USD(due to interest)
		assert!(pool_nav.latest_nav > amount);
		// updated time should be after_one_years
		assert_eq!(pool_nav.last_updated, after_one_year/1000);
		assert_last_event::<T, <T as LoanConfig>::Event>(LoanEvent::NAVUpdated(pool_id, pool_nav.latest_nav).into());
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::MockRuntime,
);
