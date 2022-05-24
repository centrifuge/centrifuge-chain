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

//! Unit test cases for Loan pallet
use super::*;
use crate as pallet_loans;
use crate::loan_type::{CreditLine, CreditLineWithMaturity};
use crate::mock::{
	Borrower, Event, InterestAccrual, JuniorInvestor, Loans, MockRuntime, Origin, RiskAdmin,
	SeniorInvestor, Timestamp, Tokens,
};
use crate::mock::{PoolAdmin, TestExternalitiesBuilder};
use crate::test_utils::{
	assert_last_event, create, create_nft_class, expect_asset_owner, expect_asset_to_be_burned,
	initialise_test_pool, mint_nft,
};
use common_types::{CurrencyId, PoolId, PoolLocator};
use frame_support::traits::fungibles::Inspect;
use frame_support::{assert_err, assert_ok};
use loan_type::{BulletLoan, LoanType};
use pallet_loans::Event as LoanEvent;
use runtime_common::{Balance, ClassId, InstanceId, Rate, CFG as USD};
use sp_arithmetic::traits::{checked_pow, CheckedMul};
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::StaticLookup;
use sp_runtime::ArithmeticError;

// Return last triggered event
fn last_event() -> Event {
	frame_system::Pallet::<MockRuntime>::events()
		.pop()
		.map(|item| item.event)
		.expect("Event expected")
}

fn fetch_loan_event(event: Event) -> Option<LoanEvent<MockRuntime>> {
	match event {
		Event::Loans(loan_event) => Some(loan_event),
		_ => None,
	}
}

type MultiCurrencyBalanceOf<T> = <T as pallet_pools::Config>::Balance;

fn balance_of<T>(currency_id: T::CurrencyId, account: &T::AccountId) -> MultiCurrencyBalanceOf<T>
where
	T: pallet_pools::Config + frame_system::Config,
{
	<T as pallet_pools::Config>::Tokens::balance(currency_id, account)
}

fn issue_test_loan<T>(pool_id: u64, borrower: T::AccountId) -> (T::PoolId, AssetOf<T>, AssetOf<T>)
where
	T: pallet_pools::Config<
			CurrencyId = CurrencyId,
			Balance = u128,
			PoolId = PoolId,
			TrancheId = [u8; 16],
			EpochId = u32,
		> + pallet_loans::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64, Origin = Origin>
		+ pallet_uniques::Config<ClassId = ClassId, InstanceId = InstanceId>
		+ pallet_permissions::Config<Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
	PoolIdOf<T>: From<<T as pallet_pools::Config>::PoolId>,
{
	let pool_admin = PoolAdmin::get();

	create::<T>(
		pool_id,
		pool_admin,
		JuniorInvestor::get(),
		SeniorInvestor::get(),
		CurrencyId::Usd,
	);
	// add borrower role and price admin role
	assert_ok!(pallet_permissions::Pallet::<T>::add(
		Origin::signed(pool_admin),
		Role::PoolRole(PoolRole::PoolAdmin),
		borrower,
		PermissionScope::Pool(pool_id),
		Role::PoolRole(PoolRole::Borrower),
	));
	assert_ok!(pallet_permissions::Pallet::<T>::add(
		Origin::signed(pool_admin),
		Role::PoolRole(PoolRole::PoolAdmin),
		borrower,
		PermissionScope::Pool(pool_id),
		Role::PoolRole(PoolRole::PricingAdmin),
	));
	let pr_pool_id: PoolIdOf<T> = pool_id.into();
	let loan_nft_class_id =
		initialise_test_pool::<T>(pr_pool_id, 1, pool_admin, Some(Loans::account_id()));
	let collateral_class = create_nft_class::<T>(2, borrower.clone(), None);
	let instance_id = mint_nft::<T>(borrower.clone(), collateral_class);
	let collateral = Asset(collateral_class, instance_id);
	let res = Loans::create(Origin::signed(borrower), pool_id, collateral);
	assert_ok!(res);

	// post issue checks
	// next loan id should 2
	assert_eq!(NextLoanId::<T>::get(pr_pool_id), 2u128.into());

	// loanId should be 1
	let loan_id = 1u128.into();

	// event should be emitted
	assert_last_event::<MockRuntime, <MockRuntime as pallet_loans::Config>::Event>(
		LoanEvent::Created(pool_id, loan_id, collateral).into(),
	);
	let loan = Loans::get_loan(pool_id, loan_id).expect("LoanDetails should be present");

	// collateral is same as we sent before
	assert_eq!(loan.collateral, collateral);
	assert_eq!(loan.status, LoanStatus::Created);

	// collateral nft owner is pool pallet
	let pool_account = PoolLocator { pool_id }.into_account();
	expect_asset_owner::<T>(collateral, pool_account);

	// pool should be initialised
	assert_eq!(
		loan_nft_class_id,
		Loans::get_loan_nft_class(pool_id).expect("Loan class should be created")
	);
	(pool_id, Asset(loan_nft_class_id, loan_id), collateral)
}

fn default_bullet_loan_params() -> LoanType<Rate, Balance> {
	LoanType::BulletLoan(BulletLoan::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100),
		// probability of default is 4%
		Rate::saturating_from_rational(4, 100),
		// loss given default is 50%
		Rate::saturating_from_rational(50, 100),
		// collateral value
		125 * USD,
		// 4%
		math::interest_rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
		// 2 years
		math::seconds_per_year() * 2,
	))
}

fn default_credit_line_params() -> LoanType<Rate, Balance> {
	LoanType::CreditLine(CreditLine::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100),
		// collateral value
		125 * USD,
	))
}

fn default_credit_line_with_maturity_params() -> LoanType<Rate, Balance> {
	LoanType::CreditLineWithMaturity(CreditLineWithMaturity::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100),
		// probability of default is 4%
		Rate::saturating_from_rational(4, 100),
		// loss given default is 50%
		Rate::saturating_from_rational(50, 100),
		// collateral value
		125 * USD,
		// 4%
		math::interest_rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
		// 2 years
		math::seconds_per_year() * 2,
	))
}

fn price_test_loan<T>(
	admin: T::AccountId,
	pool_id: T::PoolId,
	loan_id: T::LoanId,
	rp: Rate,
	loan_type: LoanType<Rate, Balance>,
) where
	T: pallet_pools::Config<PoolId = PoolId>
		+ pallet_loans::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let res = Loans::price(Origin::signed(admin), pool_id, loan_id, rp, loan_type);
	assert_ok!(res);
	let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
	let (got_pool_id, got_loan_id) = match loan_event {
		LoanEvent::Priced(pool_id, loan_id, _, _) => Some((pool_id, loan_id)),
		_ => None,
	}
	.expect("must be a Loan issue priced event");
	assert_eq!(pool_id, got_pool_id);
	assert_eq!(loan_id, got_loan_id);

	// check loan status as Active
	let loan = Loan::<MockRuntime>::get(pool_id, loan_id).expect("LoanDetails should be present");
	let active_loan =
		Loans::get_active_loan(pool_id, loan_id).expect("ActiveLoanDetails should be present");
	assert_eq!(loan.status, LoanStatus::Active);
	assert_eq!(active_loan.interest_rate_per_sec, rp);
	assert_eq!(active_loan.loan_type, loan_type);
	assert_eq!(active_loan.max_borrow_amount(0), 100 * USD);
	assert_eq!(active_loan.write_off_status, WriteOffStatus::None);
}

fn price_bullet_loan<T>(
	admin: T::AccountId,
	pool_id: T::PoolId,
	loan_id: T::LoanId,
) -> (Rate, LoanType<Rate, Balance>)
where
	T: pallet_pools::Config<PoolId = PoolId>
		+ pallet_loans::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let loan_type = default_bullet_loan_params();
	// interest rate is 5%
	let rp = math::interest_rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
	price_test_loan::<T>(admin, pool_id, loan_id, rp, loan_type);
	(rp, loan_type)
}

fn price_credit_line_loan<T>(
	admin: T::AccountId,
	pool_id: T::PoolId,
	loan_id: T::LoanId,
) -> (Rate, LoanType<Rate, Balance>)
where
	T: pallet_pools::Config<PoolId = PoolId>
		+ pallet_loans::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let loan_type = default_credit_line_params();
	// interest rate is 5%
	let rp = math::interest_rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
	price_test_loan::<T>(admin, pool_id, loan_id, rp, loan_type);
	(rp, loan_type)
}

fn price_credit_line_with_maturity_loan<T>(
	admin: T::AccountId,
	pool_id: T::PoolId,
	loan_id: T::LoanId,
) -> (Rate, LoanType<Rate, Balance>)
where
	T: pallet_pools::Config<PoolId = PoolId>
		+ pallet_loans::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let loan_type = default_credit_line_with_maturity_params();
	// interest rate is 5%
	let rp = math::interest_rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
	price_test_loan::<T>(admin, pool_id, loan_id, rp, loan_type);
	(rp, loan_type)
}

fn close_test_loan<T>(
	owner: T::AccountId,
	pool_id: T::PoolId,
	loan: AssetOf<T>,
	collateral: AssetOf<T>,
) where
	T: pallet_pools::Config<PoolId = PoolId>
		+ pallet_loans::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let loan_id = loan.1;

	// close the loan
	let res = Loans::close(Origin::signed(owner), pool_id, loan_id);
	assert_ok!(res);

	let (got_pool_id, got_loan_id, got_collateral) =
		match fetch_loan_event(last_event()).expect("should be a loan event") {
			LoanEvent::Closed(pool_id, loan_id, collateral) => Some((pool_id, loan_id, collateral)),
			_ => None,
		}
		.expect("must be a Loan close event");
	assert_eq!(pool_id, got_pool_id);
	assert_eq!(loan_id, got_loan_id);
	assert_eq!(collateral, got_collateral);

	// check that collateral nft was returned
	expect_asset_owner::<T>(collateral, owner);

	// check that loan nft was burned
	expect_asset_to_be_burned::<T>(loan);

	// check loan status as Closed
	let loan = Loan::<MockRuntime>::get(pool_id, loan_id).expect("LoanDetails should be present");
	assert_eq!(loan.status, LoanStatus::Closed);
}

#[test]
fn test_create() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();

			// successful issue
			let (pool_id, loan, collateral) = issue_test_loan::<MockRuntime>(0, borrower);

			// wrong owner
			let owner2 = 2;
			let res = Loans::create(Origin::signed(owner2), pool_id, collateral);
			assert_err!(res, Error::<MockRuntime>::NotAssetOwner);

			// missing owner
			let instance_id = 100u128.into();
			let res = Loans::create(
				Origin::signed(owner2),
				pool_id,
				Asset(collateral.0, instance_id),
			);
			assert_err!(res, Error::<MockRuntime>::NFTOwnerNotFound);

			// trying to issue a loan with loan nft
			let res = Loans::create(Origin::signed(borrower), pool_id, loan);
			assert_err!(res, Error::<MockRuntime>::NotAValidAsset)
		});
}

#[test]
fn test_price_bullet_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();

			// successful issue
			let (pool_id, loan, _collateral) = issue_test_loan::<MockRuntime>(0, borrower);

			let loan_id = loan.1;

			// maturity date in the past
			let loan_type = LoanType::BulletLoan(BulletLoan::new(
				// advance rate 80%
				Rate::saturating_from_rational(80, 100),
				// probability of default is 4%
				Rate::saturating_from_rational(4, 100),
				// loss given default is 50%
				Rate::saturating_from_rational(50, 100),
				// collateral value
				125 * USD,
				// 4%
				math::interest_rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
				// maturity date in the past
				1,
			));
			let rp = math::interest_rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
			Timestamp::set_timestamp(100 * 1000);
			let res = Loans::price(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::LoanValueInvalid);

			// interest_rate_per_sec is invalid
			let loan_type = LoanType::BulletLoan(BulletLoan::new(
				// advance rate 80%
				Rate::saturating_from_rational(80, 100),
				// probability of default is 4%
				Rate::saturating_from_rational(4, 100),
				// loss given default is 50%
				Rate::saturating_from_rational(50, 100),
				// collateral value
				125 * USD,
				// 4%
				math::interest_rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
				// maturity in 2 years
				math::seconds_per_year() * 2,
			));
			let rp = Zero::zero();
			let res = Loans::price(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::LoanValueInvalid);

			// successful pricing
			let (rate, loan_type) = price_bullet_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// pricing an active but non borrowed against loan should be possible
			let res = Loans::price(Origin::signed(borrower), pool_id, loan.1, rate, loan_type);
			assert_ok!(res);
		})
}

#[test]
fn test_price_credit_line_with_maturity_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();

			// successful issue
			let (pool_id, loan, _collateral) = issue_test_loan::<MockRuntime>(0, borrower);

			let loan_id = loan.1;

			// maturity date in the past
			let loan_type = LoanType::CreditLineWithMaturity(CreditLineWithMaturity::new(
				// advance rate 80%
				Rate::saturating_from_rational(80, 100),
				// probability of default is 4%
				Rate::saturating_from_rational(4, 100),
				// loss given default is 50%
				Rate::saturating_from_rational(50, 100),
				// collateral value
				125 * USD,
				// 4%
				math::interest_rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
				// maturity date in the past
				1,
			));
			let rp = math::interest_rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
			Timestamp::set_timestamp(100 * 1000);
			let res = Loans::price(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::LoanValueInvalid);

			// interest_rate_per_sec is invalid
			let loan_type = LoanType::CreditLineWithMaturity(CreditLineWithMaturity::new(
				// advance rate 80%
				Rate::saturating_from_rational(80, 100),
				// probability of default is 4%
				Rate::saturating_from_rational(4, 100),
				// loss given default is 50%
				Rate::saturating_from_rational(50, 100),
				// collateral value
				125 * USD,
				// 4%
				math::interest_rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
				// maturity in 2 years
				math::seconds_per_year() * 2,
			));
			let rp = Zero::zero();
			let res = Loans::price(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::LoanValueInvalid);

			// successful pricing
			let (rate, loan_type) = price_bullet_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// pricing an active but non borrowed against loan should be possible
			let res = Loans::price(Origin::signed(borrower), pool_id, loan.1, rate, loan_type);
			assert_ok!(res);
		})
}

#[test]
fn test_price_credit_line_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();

			// successful issue
			let (pool_id, loan, _collateral) = issue_test_loan::<MockRuntime>(0, borrower);

			let loan_id = loan.1;

			// interest_rate_per_sec is invalid
			let loan_type = LoanType::CreditLine(CreditLine::new(
				// advance rate 80%
				Rate::saturating_from_rational(80, 100),
				// collateral value
				125 * USD,
			));
			let rp = Zero::zero();
			let res = Loans::price(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::LoanValueInvalid);

			// successful pricing
			let (rate, loan_type) = price_bullet_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// pricing an active but non borrowed against loan should be possible
			let res = Loans::price(Origin::signed(borrower), pool_id, loan.1, rate, loan_type);
			assert_ok!(res);
		})
}

macro_rules! test_close_loan {
	($price_loan:ident) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let borrower = Borrower::get();
				// successful issue
				let (pool_id, loan, collateral) = issue_test_loan::<MockRuntime>(0, borrower);

				// successful pricing
				$price_loan::<MockRuntime>(borrower, pool_id, loan.1);

				// successful close of loan
				close_test_loan::<MockRuntime>(borrower, pool_id, loan, collateral);
			})
	};
}

#[test]
fn test_close_bullet_loan() {
	test_close_loan!(price_bullet_loan)
}

#[test]
fn test_close_credit_line_loan() {
	test_close_loan!(price_credit_line_loan)
}

#[test]
fn test_close_credit_line_with_maturity_loan() {
	test_close_loan!(price_credit_line_with_maturity_loan)
}

macro_rules! test_borrow_loan {
	($price_loan:ident, $maturity_check:expr) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let pool_admin = PoolAdmin::get();
				let borrower = Borrower::get();
				// successful issue
				let (pool_id, loan, _collateral) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());

				// successful pricing
				let loan_id = loan.1;
				let (rate, loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// borrow 50 first
				Timestamp::set_timestamp(1 * 1000);
				let borrow_amount = 50 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// pricing an active loan with non zero borrowed amount should not be possible
				let res = Loans::price(
					Origin::signed(borrower),
					pool_id,
					loan_id,
					math::interest_rate_per_sec(Rate::saturating_from_rational::<u64, u64>(5, 100))
						.unwrap(),
					default_credit_line_params(),
				);
				assert_err!(res, Error::<MockRuntime>::LoanIsActive);

				// check loan data
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				let rate_info = InterestAccrual::get_rate(active_loan.interest_rate_per_sec)
					.expect("Rate information should be present");

				// accumulated rate is now rate per sec
				assert_eq!(active_loan.interest_rate_per_sec, rate);
				assert_eq!(rate_info.last_updated, 1);
				assert_eq!(active_loan.total_borrowed, 50 * USD);
				let inverse_rate = rate_info.accumulated_rate.reciprocal().unwrap();
				let p_debt = inverse_rate.checked_mul_int(borrow_amount).unwrap();
				assert_eq!(active_loan.normalized_debt, p_debt);
				// pool should have 50 less token
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 950 * USD);
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 50 * USD);
				// nav should be updated to latest present value
				let current_nav = <Loans as TPoolNav<PoolId, Balance>>::nav(pool_id)
					.unwrap()
					.0;
				let now = Loans::now();
				let pv = active_loan.present_value(p_debt, &vec![], now).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// borrow another 20 after 1000 seconds
				Timestamp::set_timestamp(1001 * 1000);
				let borrow_amount = 20 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);
				// check loan data
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				let rate_info = InterestAccrual::get_rate(active_loan.interest_rate_per_sec)
					.expect("Rate information should be present");
				assert_eq!(active_loan.total_borrowed, 70 * USD);
				let c_debt = math::debt(p_debt, rate_info.accumulated_rate)
					.unwrap()
					.checked_add(borrow_amount)
					.unwrap();
				let inverse_rate = rate_info.accumulated_rate.reciprocal().unwrap();
				let p_debt = inverse_rate.checked_mul_int(c_debt).unwrap();
				assert_eq!(active_loan.normalized_debt, p_debt);

				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 930 * USD);
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 70 * USD);
				let current_nav = <Loans as TPoolNav<PoolId, Balance>>::nav(pool_id)
					.unwrap()
					.0;
				let now = Loans::now();
				let pv = active_loan.present_value(p_debt, &vec![], now).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// try to borrow more than max_borrow_amount
				// borrow another 40 after 1000 seconds
				Timestamp::set_timestamp(2001 * 1000);
				let borrow_amount = 40 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_err!(res, Error::<MockRuntime>::MaxBorrowAmountExceeded);

				// try to borrow after maturity date
				let mut now = 2001;
				if $maturity_check {
					now = loan_type.maturity_date().unwrap() + 1;
					Timestamp::set_timestamp(now * 1000);
					let res =
						Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
					assert_err!(res, Error::<MockRuntime>::LoanMaturityDatePassed);
				}

				// written off loan cannot borrow
				// add write off groups
				let risk_admin = RiskAdmin::get();
				assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(pool_admin),
					Role::PoolRole(PoolRole::PoolAdmin),
					risk_admin,
					PermissionScope::Pool(pool_id),
					Role::PoolRole(PoolRole::RiskAdmin),
				));
				for group in vec![
					(3, 0, 1),
					(5, 15, 2),
					(7, 20, 3),
					(20, 30, 4),
					(120, 100, 5),
				] {
					let res = Loans::add_write_off_group(
						Origin::signed(risk_admin),
						pool_id,
						WriteOffGroup {
							percentage: Rate::saturating_from_rational::<u64, u64>(group.1, 100),
							overdue_days: group.0,
							penalty_interest_rate_per_sec: Rate::saturating_from_rational::<u64, u64>(
								group.2, 100,
							),
						},
					);
					assert_ok!(res);
				}

				let res = Loans::admin_write_off(
					Origin::signed(risk_admin),
					pool_id,
					loan_id,
					Rate::saturating_from_rational::<u64, u64>(3, 100),
					Rate::saturating_from_rational::<u64, u64>(1, 100),
				);
				assert_ok!(res);

				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_err!(res, Error::<MockRuntime>::WrittenOffByAdmin);

				// update nav
				let updated_nav =
					<Loans as TPoolNav<PoolId, Balance>>::update_nav(pool_id).unwrap();
				// check loan data
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				// after maturity should be current outstanding
				let debt = InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)
				.expect("Interest should accrue");
				assert_eq!(
					updated_nav, debt,
					"should be equal to outstanding debt post maturity"
				);
			})
	};
}

#[test]
fn test_borrow_bullet_loan() {
	test_borrow_loan!(price_bullet_loan, true)
}

#[test]
fn test_borrow_credit_line_with_maturity_loan() {
	test_borrow_loan!(price_credit_line_with_maturity_loan, true)
}
#[test]
fn test_borrow_credit_line_loan() {
	test_borrow_loan!(price_credit_line_loan, false)
}

macro_rules! test_repay_loan {
	($price_loan:ident) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let borrower: u64 = Borrower::get();
				// successful issue
				let (pool_id, loan_nft, collateral_nft) =
					issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());
				let loan_id = loan_nft.1;

				// successful pricing
				let (rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// borrow 50
				Timestamp::set_timestamp(1 * 1000);
				let borrow_amount = 50 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// check loan data
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				let rate_info = InterestAccrual::get_rate(active_loan.interest_rate_per_sec)
					.expect("Rate information should be present");
				// accumulated rate is now rate per sec
				assert_eq!(active_loan.total_borrowed, 50 * USD);
				let p_debt = rate_info
					.accumulated_rate
					.reciprocal()
					.unwrap()
					.checked_mul_int(borrow_amount)
					.unwrap();
				assert_eq!(active_loan.normalized_debt, p_debt);
				// pool should have 50 less token
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 950 * USD);
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 50 * USD);
				// nav should be updated to latest present value
				let current_nav = <Loans as TPoolNav<PoolId, Balance>>::nav(pool_id)
					.unwrap()
					.0;
				let now = Loans::now();
				let debt = InterestAccrual::current_debt(active_loan.interest_rate_per_sec, p_debt)
					.unwrap();
				let pv = active_loan.present_value(debt, &vec![], now).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// repay 20 after 1000 seconds
				Timestamp::set_timestamp(1001 * 1000);
				let repay_amount = 20 * USD;
				assert_eq!(active_loan.total_repaid, 0);
				let res = Loans::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
				assert_ok!(res);

				// check loan data
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				let rate_info = InterestAccrual::get_rate(active_loan.interest_rate_per_sec)
					.expect("Rate information should be present");
				// accumulated rate is now rate per sec
				assert_eq!(
					rate_info.accumulated_rate,
					checked_pow(rate, 1000).unwrap().checked_mul(&rate).unwrap()
				);
				assert_eq!(rate_info.last_updated, 1001);
				assert_eq!(active_loan.total_borrowed, 50 * USD);
				assert_eq!(active_loan.total_repaid, 20 * USD);
				// principal debt should still be more than 30 due to interest
				assert!(active_loan.normalized_debt > 30 * USD);
				// pool should have 30 less token
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 970 * USD);
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 30 * USD);
				// nav should be updated to latest present value
				let current_nav = <Loans as TPoolNav<PoolId, Balance>>::nav(pool_id)
					.unwrap()
					.0;
				let now = Loans::now();
				let debt = InterestAccrual::current_debt(active_loan.interest_rate_per_sec, p_debt)
					.unwrap();
				let pv = active_loan.present_value(debt, &vec![], now).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// repay 30 more after another 1000 seconds
				Timestamp::set_timestamp(2001 * 1000);
				let repay_amount = 30 * USD;
				let res = Loans::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
				assert_ok!(res);

				// try and close the loan
				let res = Loans::close(Origin::signed(borrower), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::LoanNotRepaid);

				// check loan data
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				// nav should be updated to latest present value
				let current_nav = <Loans as TPoolNav<PoolId, Balance>>::nav(pool_id)
					.unwrap()
					.0;
				let now = Loans::now();
				let old_debt = InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)
				.expect("Debt should be calculatable");
				let pv = active_loan.present_value(old_debt, &vec![], now).unwrap();
				assert_eq!(active_loan.total_repaid, 50 * USD);
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// repay the interest
				// 50 for 1000 seconds
				let amount = 50 * USD;
				let p_debt = active_loan
					.interest_rate_per_sec
					.reciprocal()
					.unwrap()
					.checked_mul_int(amount)
					.unwrap();
				let rate_after_1000 = checked_pow(active_loan.interest_rate_per_sec, 1001).unwrap();
				let debt_after_1000 = rate_after_1000.checked_mul_int(p_debt).unwrap();

				// 30 for 1000 seconds
				let p_debt = rate_after_1000
					.reciprocal()
					.unwrap()
					.checked_mul_int(debt_after_1000 - (20 * USD))
					.unwrap();
				let rate_after_2000 = checked_pow(active_loan.interest_rate_per_sec, 2001).unwrap();
				let debt_after_2000 = rate_after_2000.checked_mul_int(p_debt).unwrap();
				let p_debt = rate_after_2000
					.reciprocal()
					.unwrap()
					.checked_mul_int(debt_after_2000 - (30 * USD))
					.unwrap();
				assert_eq!(active_loan.normalized_debt, p_debt);

				// debt after 3000 seconds
				Timestamp::set_timestamp(3001 * 1000);
				let rate_after_3000 = checked_pow(active_loan.interest_rate_per_sec, 3001).unwrap();
				let debt = rate_after_3000.checked_mul_int(p_debt).unwrap();

				// transfer exact interest amount to owner account from dummy account 2
				let dummy: u64 = 7;
				let dest =
					<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(
						borrower,
					);
				let transfer_amount = debt;
				let res = Tokens::transfer(
					Origin::signed(dummy),
					dest,
					CurrencyId::Usd,
					transfer_amount,
				);
				assert_ok!(res);

				// repay more than the interest
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				let total_repaid_pre = active_loan.total_repaid;

				let repay_amount = debt + 10 * USD;
				let res = Loans::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
				assert_ok!(res);

				// only the debt should have been repaid
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				assert_eq!(active_loan.total_repaid - total_repaid_pre, debt);

				// close loan
				let res = Loans::close(Origin::signed(borrower), pool_id, loan_id);
				assert_ok!(res);

				// check loan data
				let loan = Loan::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanDetails should be present");
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				let rate_info = InterestAccrual::get_rate(active_loan.interest_rate_per_sec)
					.expect("Rate information should be present");
				assert_eq!(loan.status, LoanStatus::Closed);
				assert_eq!(active_loan.normalized_debt, Zero::zero());
				assert_eq!(active_loan.total_borrowed, 50 * USD);
				assert_eq!(rate_info.last_updated, 3001);
				// nav should be updated to latest present value and should be zero
				let current_nav = <Loans as TPoolNav<PoolId, Balance>>::nav(pool_id)
					.unwrap()
					.0;
				let now = Loans::now();
				let old_debt = InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)
				.expect("Debt should be calculatable");
				let pv = active_loan.present_value(old_debt, &vec![], now).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");
				assert_eq!(current_nav, Zero::zero());

				// pool balance should be 1000 + interest
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				let expected_balance = 1000 * USD + transfer_amount;
				assert_eq!(pool_balance, expected_balance);

				// owner balance should be zero
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());

				// owner account should own the collateral NFT
				expect_asset_owner::<MockRuntime>(collateral_nft, borrower);

				// pool account should own the loan NFT
				expect_asset_to_be_burned::<MockRuntime>(loan_nft);

				// check nav
				let res = Loans::update_nav_of_pool(pool_id);
				assert_ok!(res);
				let (nav, loans_updated) = res.unwrap();
				assert_eq!(nav, Zero::zero());
				assert_eq!(loans_updated, 1);
			})
	};
}

#[test]
fn test_repay_bullet_loan() {
	test_repay_loan!(price_bullet_loan)
}

#[test]
fn test_repay_credit_line_with_maturity_loan() {
	test_repay_loan!(price_credit_line_with_maturity_loan)
}

#[test]
fn test_repay_credit_line_loan() {
	test_repay_loan!(price_credit_line_loan)
}

macro_rules! test_pool_nav {
	($price_loan:ident,$moving_max_borrow_amount:expr,$admin_write_off:expr,$pv_1:expr,$pv_200:expr) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let pool_admin: u64 = PoolAdmin::get();
				let borrower: u64 = Borrower::get();
				// successful issue
				let (pool_id, loan, _collateral) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());
				let loan_id = loan.1;

				// successful pricing
				let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// present value should still be zero
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				let now = Loans::now();
				let old_debt = InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)
				.expect("Debt should be calculatable");
				let pv = active_loan.present_value(old_debt, &vec![], now).unwrap();
				assert_eq!(pv, Zero::zero());

				// borrow 50 amount at the instant
				let borrow_amount = 50 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// check present value
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				let now = Loans::now();
				let old_debt = InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)
				.expect("Debt should be calculatable");
				let pv = active_loan.present_value(old_debt, &vec![], now).unwrap();
				assert_eq!(pv, $pv_1);

				// pass some time. maybe 200 days
				let after_200_days = 3600 * 24 * 200;
				Timestamp::set_timestamp(after_200_days * 1000);
				let res = Loans::update_nav_of_pool(pool_id);
				assert_ok!(res);
				let (nav, ..) = res.unwrap();
				// present value should be 50.05
				assert_eq!(nav, $pv_200);

				if $moving_max_borrow_amount {
					// can borrow upto max_borrow_amount
					// max_borrow_amount = 125 * 0.8 - debt
					// check present value
					let active_loan = Loans::get_active_loan(pool_id, loan_id)
						.expect("ActiveLoanDetails should be present");
					let debt = InterestAccrual::current_debt(
						active_loan.interest_rate_per_sec,
						active_loan.normalized_debt,
					)
					.unwrap();
					let borrow_amount = (100 * USD).checked_sub(debt).unwrap();
					let res =
						Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
					assert_ok!(res);

					// cannot borrow more than max_borrow_amount, 1
					let borrow_amount = 1 * USD;
					let res =
						Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
					assert_err!(res, Error::<MockRuntime>::MaxBorrowAmountExceeded);

					// payback 50 and borrow more later
					let repay_amount = 50 * USD;
					let res =
						Loans::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
					assert_ok!(res);

					// pass some time. maybe 500 days
					let after_500_days = 3600 * 24 * 300;
					Timestamp::set_timestamp(after_500_days * 1000);

					// you cannot borrow more than 50 since the debt is more than 50 by now
					let borrow_amount = 50 * USD;
					let res =
						Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
					assert_err!(res, Error::<MockRuntime>::MaxBorrowAmountExceeded);

					// borrow 40 maybe
					let borrow_amount = 40 * USD;
					let res =
						Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
					assert_ok!(res);
				} else {
					// borrow another 50 and
					let borrow_amount = 50 * USD;
					let res =
						Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
					assert_ok!(res);

					// cannot borrow more than max_borrow_amount, 1
					let borrow_amount = 1 * USD;
					let res =
						Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
					assert_err!(res, Error::<MockRuntime>::MaxBorrowAmountExceeded);
				}

				// let the maturity has passed 2 years + 10 day
				let after_2_years = (math::seconds_per_year() * 2) + math::seconds_per_day() * 10;
				let active_loan = Loans::get_active_loan(pool_id, loan_id)
					.expect("ActiveLoanDetails should be present");
				Timestamp::set_timestamp(after_2_years * 1000);
				let debt = InterestAccrual::current_debt(
					active_loan.interest_rate_per_sec,
					active_loan.normalized_debt,
				)
				.unwrap();
				let res = Loans::update_nav_of_pool(pool_id);
				assert_ok!(res);
				let (pv, ..) = res.unwrap();
				// present value should be equal to current outstanding debt
				assert_eq!(pv, debt);
				let (nav, ..) = res.unwrap();
				assert_eq!(pv, nav);

				// call update nav extrinsic and check for event
				let res = Loans::update_nav(Origin::signed(borrower), pool_id);
				assert_ok!(res);
				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
				let (got_pool_id, updated_nav, exact) = match loan_event {
					LoanEvent::NAVUpdated(pool_id, update_nav, exact) => {
						Some((pool_id, update_nav, exact))
					}
					_ => None,
				}
				.expect("must be a Nav updated event");
				assert_eq!(pool_id, got_pool_id);
				assert_eq!(updated_nav, nav);
				assert_eq!(exact, NAVUpdateType::Exact);

				let risk_admin = RiskAdmin::get();
				assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(pool_admin),
					Role::PoolRole(PoolRole::PoolAdmin),
					risk_admin,
					PermissionScope::Pool(pool_id),
					Role::PoolRole(PoolRole::RiskAdmin),
				));
				// write off the loan and check for updated nav
				for group in vec![(3, 10, 1), (5, 15, 2), (7, 20, 3), (20, 30, 4)] {
					let group = WriteOffGroup {
						percentage: Rate::saturating_from_rational::<u128, u128>(group.1, 100),
						overdue_days: group.0,
						penalty_interest_rate_per_sec: Rate::saturating_from_rational::<u64, u64>(
							group.2, 100,
						),
					};
					let res =
						Loans::add_write_off_group(Origin::signed(risk_admin), pool_id, group);
					assert_ok!(res);
				}

				if $admin_write_off {
					let res = Loans::admin_write_off(
						Origin::signed(risk_admin),
						pool_id,
						loan_id,
						Rate::saturating_from_rational::<u64, u64>(7, 100),
						Rate::saturating_from_rational::<u64, u64>(3, 100),
					);
					assert_ok!(res);
				} else {
					// write off loan. someone calls write off
					let res = Loans::write_off(Origin::signed(100), pool_id, loan_id);
					assert_ok!(res);
				}
				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
				let (_pool_id, _loan_id, write_off_index) = match loan_event {
					LoanEvent::WrittenOff(pool_id, loan_id, .., write_off_index) => {
						Some((pool_id, loan_id, write_off_index))
					}
					_ => None,
				}
				.expect("must be a loan written off event");
				// it must be 2 with overdue days as 7 and write off percentage as 20%
				assert_eq!(write_off_index, Some(2));

				// update nav
				let res = Loans::update_nav(Origin::signed(borrower), pool_id);
				assert_ok!(res);
				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
				let (_pool_id, updated_nav, exact) = match loan_event {
					LoanEvent::NAVUpdated(pool_id, update_nav, exact) => {
						Some((pool_id, update_nav, exact))
					}
					_ => None,
				}
				.expect("must be a Nav updated event");

				// updated nav should be (1-20%) outstanding debt
				let expected_nav = debt
					- Rate::saturating_from_rational(20, 100)
						.checked_mul_int(debt)
						.unwrap();
				assert_eq!(expected_nav, updated_nav);
				assert_eq!(exact, NAVUpdateType::Exact);
			})
	};
}

#[test]
fn test_pool_nav_bullet_loan() {
	test_pool_nav!(
		price_bullet_loan,
		// not a credit line
		false,
		// anyone can write off after maturity
		false,
		// present value at the instant of origination
		48969664319886742807u128,
		// present value after 200 days
		50054820713981957086u128
	)
}

#[test]
fn test_pool_nav_credit_line_with_maturity_loan() {
	test_pool_nav!(
		price_credit_line_with_maturity_loan,
		// credit line
		true,
		// anyone can write off after maturity
		false,
		// present value at the instant of origination
		48969664319886742807u128,
		// present value after 200 days
		50054820713981957086u128
	)
}

#[test]
fn test_pool_nav_credit_line_loan() {
	test_pool_nav!(
		price_credit_line_loan,
		// credit line
		true,
		// only admin can write off
		true,
		// present value at the instant of origination
		50000000000000000000u128,
		// present value after 200 days
		51388800811704851015u128
	)
}

#[test]
fn test_add_write_off_groups() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_admin = PoolAdmin::get();
			let risk_admin: u64 = RiskAdmin::get();
			let pool_id = 0;
			create::<MockRuntime>(
				pool_id,
				pool_admin,
				JuniorInvestor::get(),
				SeniorInvestor::get(),
				CurrencyId::Usd,
			);
			let pr_pool_id: PoolIdOf<MockRuntime> = pool_id.into();
			initialise_test_pool::<MockRuntime>(pr_pool_id, 1, pool_admin, None);
			assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
				Origin::signed(pool_admin),
				Role::PoolRole(PoolRole::PoolAdmin),
				risk_admin,
				PermissionScope::Pool(pool_id),
				Role::PoolRole(PoolRole::RiskAdmin),
			));

			// fetch write off groups
			let groups = PoolWriteOffGroups::<MockRuntime>::get(pool_id);
			assert_eq!(groups, vec![]);

			for percentage in vec![10, 20, 30, 40, 30, 50, 70, 100] {
				// add a new write off group
				let group = WriteOffGroup {
					percentage: Rate::saturating_from_rational(percentage, 100),
					overdue_days: 3,
					penalty_interest_rate_per_sec: Rate::saturating_from_rational::<u64, u64>(
						5, 100,
					),
				};
				let res = Loans::add_write_off_group(Origin::signed(risk_admin), pool_id, group);
				assert_ok!(res);
				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
				let (_pool_id, index) = match loan_event {
					LoanEvent::WriteOffGroupAdded(pool_id, index) => Some((pool_id, index)),
					_ => None,
				}
				.expect("must be a write off group added event");

				// check if the write off group is added
				let groups = PoolWriteOffGroups::<MockRuntime>::get(pool_id);
				assert_eq!(groups[index as usize], group);
				assert_eq!(groups.len() - 1, index as usize);
			}

			// invalid write off group
			let group = WriteOffGroup {
				percentage: Rate::saturating_from_rational(110, 100),
				overdue_days: 3,
				penalty_interest_rate_per_sec: Rate::saturating_from_rational::<u64, u64>(5, 100),
			};
			let res = Loans::add_write_off_group(Origin::signed(risk_admin), pool_id, group);
			assert_err!(res, Error::<MockRuntime>::InvalidWriteOffGroup);
		})
}

macro_rules! test_write_off_maturity_loan {
	($price_loan:ident) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let pool_admin = PoolAdmin::get();
				let borrower: u64 = Borrower::get();
				// successful issue
				let (pool_id, loan, _collateral) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());
				let loan_id = loan.1;

				// successful pricing
				let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// borrow 50
				Timestamp::set_timestamp(1 * 1000);
				let borrow_amount = 50 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// after one year
				// anyone can trigger the call
				let caller = 100;
				Timestamp::set_timestamp(math::seconds_per_year() * 1000);
				let res = Loans::write_off(Origin::signed(caller), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::LoanHealthy);

				// let the maturity date passes + 1 day
				let t = math::seconds_per_year() * 2 + math::seconds_per_day();
				Timestamp::set_timestamp(t * 1000);
				let res = Loans::write_off(Origin::signed(caller), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::NoValidWriteOffGroup);

				// add write off groups
				let risk_admin = RiskAdmin::get();
				assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(pool_admin),
					Role::PoolRole(PoolRole::PoolAdmin),
					risk_admin,
					PermissionScope::Pool(pool_id),
					Role::PoolRole(PoolRole::RiskAdmin),
				));
				for group in vec![(3, 10, 1), (5, 15, 2), (7, 20, 3), (20, 30, 4)] {
					let res = Loans::add_write_off_group(
						Origin::signed(risk_admin),
						pool_id,
						WriteOffGroup {
							percentage: Rate::saturating_from_rational(group.1, 100),
							overdue_days: group.0,
							penalty_interest_rate_per_sec: Rate::saturating_from_rational::<u64, u64>(
								group.2, 100,
							),
						},
					);
					assert_ok!(res);
				}

				// same since write off group is missing
				let t = math::seconds_per_year() * 2 + math::seconds_per_day();
				Timestamp::set_timestamp(t * 1000);
				let res = Loans::write_off(Origin::signed(caller), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::NoValidWriteOffGroup);

				// days, index
				for days_index in vec![(3, 0), (5, 1), (7, 2), (20, 3)] {
					// move to more than 3 days
					let t = math::seconds_per_year() * 2 + math::seconds_per_day() * days_index.0;
					Timestamp::set_timestamp(t * 1000);
					let res = Loans::write_off(Origin::signed(caller), pool_id, loan_id);
					assert_ok!(res);

					let loan_event =
						fetch_loan_event(last_event()).expect("should be a loan event");
					let (_pool_id, _loan_id, write_off_index) = match loan_event {
						LoanEvent::WrittenOff(pool_id, loan_id, .., write_off_index) => {
							Some((pool_id, loan_id, write_off_index))
						}
						_ => None,
					}
					.expect("must be a Loan issue event");
					assert_eq!(write_off_index, Some(days_index.1));
					let active_loan = Loans::get_active_loan(pool_id, loan_id)
						.expect("ActiveLoanDetails should be present");
					assert_eq!(
						active_loan.write_off_status,
						WriteOffStatus::WrittenOff {
							write_off_index: days_index.1
						}
					);
				}
			})
	};
}

#[test]
fn test_write_off_bullet_loan() {
	test_write_off_maturity_loan!(price_bullet_loan)
}

#[test]
fn test_write_off_credit_line_with_maturity_loan() {
	test_write_off_maturity_loan!(price_credit_line_with_maturity_loan)
}

macro_rules! test_admin_write_off_loan_type {
	($price_loan:ident) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let pool_admin = PoolAdmin::get();
				let borrower: u64 = Borrower::get();
				// successful issue
				let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());

				let loan_id = loan.1;

				// successful pricing
				let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// borrow 50
				Timestamp::set_timestamp(1 * 1000);
				let borrow_amount = 50 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// after one year
				// caller should be admin, can write off before maturity
				let risk_admin = RiskAdmin::get();
				assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(pool_admin),
					Role::PoolRole(PoolRole::PoolAdmin),
					risk_admin,
					PermissionScope::Pool(pool_id),
					Role::PoolRole(PoolRole::RiskAdmin),
				));

				// add write off groups
				let groups = vec![(3, 10, 1), (5, 15, 2), (7, 20, 3), (20, 30, 4)];
				for group in groups.clone() {
					let res = Loans::add_write_off_group(
						Origin::signed(risk_admin),
						pool_id,
						WriteOffGroup {
							percentage: Rate::saturating_from_rational(group.1 as u64, 100u64),
							overdue_days: group.0,
							penalty_interest_rate_per_sec: Rate::saturating_from_rational::<u64, u64>(
								group.2 as u64,
								100u64,
							),
						},
					);
					assert_ok!(res);
				}

				// verify and check before and after maturity
				for time in vec![
					math::seconds_per_year(),
					math::seconds_per_year() * 2 + math::seconds_per_day() * 3,
				] {
					Timestamp::set_timestamp(time * 1000);
					for index in vec![0, 3, 2, 1, 0] {
						let res = Loans::admin_write_off(
							Origin::signed(risk_admin),
							pool_id,
							loan_id,
							Rate::saturating_from_rational(groups.clone()[index].0, 100u64),
							Rate::saturating_from_rational(groups.clone()[index].2, 100u64),
						);
						assert_ok!(res);

						let loan_event =
							fetch_loan_event(last_event()).expect("should be a loan event");
						let (_pool_id, _loan_id, write_off_index) = match loan_event {
							LoanEvent::WrittenOff(pool_id, loan_id, .., write_off_index) => {
								Some((pool_id, loan_id, write_off_index))
							}
							_ => None,
						}
						.expect("must be a Loan issue event");
						assert_eq!(write_off_index, Some(index as u32));
						let active_loan = Loans::get_active_loan(pool_id, loan_id)
							.expect("ActiveLoanDetails should be present");
						assert_eq!(
							active_loan.write_off_status,
							WriteOffStatus::WrittenOff {
								write_off_index: index as u32
							}
						);
					}
				}

				// permission less write off should not work once written off by admin
				let res = Loans::write_off(Origin::signed(100), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::WrittenOffByAdmin)
			})
	};
}

#[test]
fn test_admin_write_off_bullet_loan() {
	test_admin_write_off_loan_type!(price_bullet_loan)
}

#[test]
fn test_admin_write_off_credit_line_with_maturity_loan() {
	test_admin_write_off_loan_type!(price_credit_line_with_maturity_loan)
}

#[test]
fn test_admin_write_off_credit_line_loan() {
	test_admin_write_off_loan_type!(price_credit_line_loan)
}

macro_rules! test_close_written_off_loan_type {
	($price_loan:ident, $maturity_checks:expr) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let pool_admin = PoolAdmin::get();
				let borrower: u64 = Borrower::get();
				// successful issue
				let (pool_id, loan, asset) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());

				let loan_id = loan.1;

				// successful pricing
				let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// borrow 50
				Timestamp::set_timestamp(1 * 1000);
				let borrow_amount = 50 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// let the maturity pass and closing loan should not work
				Timestamp::set_timestamp(
					(math::seconds_per_year() * 2 + 5 * math::seconds_per_day()) * 1000,
				);
				let res = Loans::close(Origin::signed(borrower), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::LoanNotRepaid);

				// add write off groups
				let risk_admin = RiskAdmin::get();
				assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(pool_admin),
					Role::PoolRole(PoolRole::PoolAdmin),
					risk_admin,
					PermissionScope::Pool(pool_id),
					Role::PoolRole(PoolRole::RiskAdmin),
				));
				for group in vec![
					(3, 10, 1),
					(5, 15, 2),
					(7, 20, 3),
					(20, 30, 4),
					(120, 100, 5),
				] {
					let res = Loans::add_write_off_group(
						Origin::signed(risk_admin),
						pool_id,
						WriteOffGroup {
							percentage: Rate::saturating_from_rational(group.1, 100),
							overdue_days: group.0,
							penalty_interest_rate_per_sec: Rate::saturating_from_rational::<u64, u64>(
								group.2, 100,
							),
						},
					);
					assert_ok!(res);
				}

				if $maturity_checks {
					// write off loan but should not be able to close since its not 100% write off
					let res = Loans::write_off(Origin::signed(200), pool_id, loan_id);
					assert_ok!(res);
					let loan_event =
						fetch_loan_event(last_event()).expect("should be a loan event");
					let (_pool_id, _loan_id, write_off_index) = match loan_event {
						LoanEvent::WrittenOff(pool_id, loan_id, .., write_off_index) => {
							Some((pool_id, loan_id, write_off_index))
						}
						_ => None,
					}
					.expect("must be a Loan issue event");
					assert_eq!(write_off_index, Some(1));
					let res = Loans::close(Origin::signed(borrower), pool_id, loan_id);
					assert_err!(res, Error::<MockRuntime>::LoanNotRepaid);

					// let it be 120 days beyond maturity, we write off 100% now
					Timestamp::set_timestamp(
						(math::seconds_per_year() * 2 + 120 * math::seconds_per_day()) * 1000,
					);
					let res = Loans::write_off(Origin::signed(200), pool_id, loan_id);
					assert_ok!(res);
				} else {
					// write off as admin
					let res = Loans::admin_write_off(
						Origin::signed(risk_admin),
						pool_id,
						loan_id,
						Rate::saturating_from_rational::<u64, u64>(120, 100),
						Rate::saturating_from_rational::<u64, u64>(5, 100),
					);
					assert_ok!(res);
				}

				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
				let (_pool_id, _loan_id, write_off_index) = match loan_event {
					LoanEvent::WrittenOff(pool_id, loan_id, .., write_off_index) => {
						Some((pool_id, loan_id, write_off_index))
					}
					_ => None,
				}
				.expect("must be a Loan written off event");
				assert_eq!(write_off_index, Some(4));

				// nav should be zero
				let res = Loans::update_nav(Origin::signed(borrower), pool_id);
				assert_ok!(res);
				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
				let (got_pool_id, updated_nav, exact) = match loan_event {
					LoanEvent::NAVUpdated(pool_id, update_nav, exact) => {
						Some((pool_id, update_nav, exact))
					}
					_ => None,
				}
				.expect("must be a Nav updated event");
				assert_eq!(pool_id, got_pool_id);
				assert_eq!(updated_nav, Zero::zero());
				assert_eq!(exact, NAVUpdateType::Exact);

				// close loan now
				close_test_loan::<MockRuntime>(borrower, pool_id, loan, asset);
			})
	};
}

#[test]
fn test_close_written_off_bullet_loan() {
	test_close_written_off_loan_type!(price_bullet_loan, true)
}

#[test]
fn test_close_written_off_credit_line_with_maturity_loan() {
	test_close_written_off_loan_type!(price_credit_line_with_maturity_loan, true)
}

#[test]
fn test_close_written_off_credit_line_loan() {
	test_close_written_off_loan_type!(price_credit_line_loan, false)
}

macro_rules! repay_too_early {
	($price_loan:ident) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				Timestamp::set_timestamp(1 * 1000);
				let borrower: u64 = Borrower::get();
				// successful issue
				let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());

				let loan_id = loan.1;

				// successful pricing
				let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// borrow amount
				let borrow_amount = 100 * USD;
				let res = Loans::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// check balances
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 900 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 100 * USD);

				// repay in the same instant
				let res = Loans::repay(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_err!(res, Error::<MockRuntime>::RepayTooEarly);

				// after origination date
				Timestamp::set_timestamp(2 * 1000);
				let res = Loans::repay(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// check balances
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());

				// close loan
				let res = Loans::close(Origin::signed(borrower), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::LoanNotRepaid)
			})
	};
}

#[test]
fn test_repay_too_early() {
	repay_too_early!(price_bullet_loan);
	repay_too_early!(price_credit_line_loan);
	repay_too_early!(price_credit_line_with_maturity_loan);
}

macro_rules! write_off_overflow {
	($price_loan:ident) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let pool_admin = PoolAdmin::get();
				let borrower: u64 = Borrower::get();
				// successful issue
				let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());
				let loan_id = loan.1;

				// successful pricing
				let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);
				// after one year
				// anyone can trigger the call
				let caller = 42;
				// add write off groups
				let risk_admin = RiskAdmin::get();
				assert_ok!(pallet_permissions::Pallet::<MockRuntime>::add(
					Origin::signed(pool_admin),
					Role::PoolRole(PoolRole::PoolAdmin),
					risk_admin,
					PermissionScope::Pool(pool_id),
					Role::PoolRole(PoolRole::RiskAdmin),
				));
				//for group in vec![(3, 10), (313503982334601, 20)] {
				for group in vec![
					(3, 10, 1),
					(313503982334601, 15, 2),
					(10, 20, 3),
					(10, 30, 4),
				] {
					let res = Loans::add_write_off_group(
						Origin::signed(risk_admin),
						pool_id,
						WriteOffGroup {
							percentage: Rate::saturating_from_rational(group.1, 100),
							overdue_days: group.0,
							penalty_interest_rate_per_sec: Rate::saturating_from_rational::<u64, u64>(
								group.2, 100,
							),
						},
					);
					assert_ok!(res);
				}

				// same since write off group is missing
				let t = math::seconds_per_year() * 2 + math::seconds_per_day() * 1337;
				Timestamp::set_timestamp(t * 1000);

				let res = Loans::write_off(Origin::signed(caller), pool_id, loan_id);
				assert_err!(res, ArithmeticError::Overflow)
			})
	};
}

#[test]
fn test_write_off_overflow() {
	write_off_overflow!(price_bullet_loan);
	write_off_overflow!(price_credit_line_with_maturity_loan);
}
