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
use crate as pallet_loan;
use crate::loan_type::{CreditLine, CreditLineWithMaturity};
use crate::mock::{
	Borrower, Event, JuniorInvestor, Loan, MockRuntime, Origin, RiskAdmin, SeniorInvestor,
	Timestamp, Tokens,
};
use crate::mock::{PoolAdmin, TestExternalitiesBuilder};
use crate::test_utils::{
	assert_last_event, create_nft_class, create_pool, expect_asset_owner, initialise_test_pool,
	mint_nft,
};
use frame_support::{assert_err, assert_ok};
use frame_system::RawOrigin;
use loan_type::{BulletLoan, LoanType};
use orml_traits::MultiCurrency;
use pallet_loan::Event as LoanEvent;
use pallet_tinlake_investor_pool::PoolLocator;
use primitives_tokens::CurrencyId;
use runtime_common::{Amount, Balance, ClassId, InstanceId, PoolId, Rate, CFG as USD};
use sp_arithmetic::traits::{checked_pow, CheckedDiv, CheckedMul, CheckedSub};
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::StaticLookup;

// Return last triggered event
fn last_event() -> Event {
	frame_system::Pallet::<MockRuntime>::events()
		.pop()
		.map(|item| item.event)
		.expect("Event expected")
}

fn fetch_loan_event(event: Event) -> Option<LoanEvent<MockRuntime>> {
	match event {
		Event::Loan(loan_event) => Some(loan_event),
		_ => None,
	}
}

type MultiCurrencyBalanceOf<T> = <T as pallet_tinlake_investor_pool::Config>::Balance;

fn balance_of<T>(currency_id: T::CurrencyId, account: &T::AccountId) -> MultiCurrencyBalanceOf<T>
where
	T: pallet_tinlake_investor_pool::Config + frame_system::Config,
{
	<T as pallet_tinlake_investor_pool::Config>::Tokens::total_balance(currency_id, account)
}

fn issue_test_loan<T>(pool_id: u64, borrower: T::AccountId) -> (T::PoolId, AssetOf<T>, AssetOf<T>)
where
	T: pallet_tinlake_investor_pool::Config<
			CurrencyId = CurrencyId,
			Balance = u128,
			PoolId = PoolId,
			TrancheId = u8,
			EpochId = u32,
		> + pallet_loan::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>
		+ pallet_uniques::Config<ClassId = ClassId, InstanceId = InstanceId>,
	PoolIdOf<T>: From<<T as pallet_tinlake_investor_pool::Config>::PoolId>,
{
	let pool_admin = PoolAdmin::get();
	create_pool::<T>(
		pool_id,
		pool_admin,
		JuniorInvestor::get(),
		SeniorInvestor::get(),
		CurrencyId::Usd,
	);
	// add borrower role and price admin role
	assert_ok!(pallet_tinlake_investor_pool::Pallet::<T>::approve_role_for(
		RawOrigin::Signed(pool_admin).into(),
		pool_id,
		PoolRole::Borrower,
		vec![<T::Lookup as StaticLookup>::unlookup(borrower)]
	));
	assert_ok!(pallet_tinlake_investor_pool::Pallet::<T>::approve_role_for(
		RawOrigin::Signed(pool_admin).into(),
		pool_id,
		PoolRole::PricingAdmin,
		vec![<T::Lookup as StaticLookup>::unlookup(borrower)]
	));
	let pr_pool_id: PoolIdOf<T> = pool_id.into();
	let loan_nft_class_id =
		initialise_test_pool::<T>(pr_pool_id, 1, pool_admin, Some(Loan::account_id()));
	let asset_class = create_nft_class::<T>(2, borrower.clone(), None);
	let instance_id = mint_nft::<T>(borrower.clone(), asset_class);
	let asset = Asset(asset_class, instance_id);
	let res = Loan::issue_loan(Origin::signed(borrower), pool_id, asset);
	assert_ok!(res);

	// post issue checks
	// next loan id should 2
	assert_eq!(NextLoanId::<T>::get(), 2u128.into());

	// loanId should be 1
	let loan_id = 1u128.into();

	// event should be emitted
	assert_last_event::<MockRuntime, <MockRuntime as pallet_loan::Config>::Event>(
		LoanEvent::LoanIssued(pool_id, loan_id, asset).into(),
	);
	let loan_data = Loan::get_loan_info(pool_id, loan_id).expect("LoanData should be present");

	// asset is same as we sent before
	assert_eq!(loan_data.asset, asset);
	assert_eq!(loan_data.status, LoanStatus::Issued);

	// asset owner is loan pallet
	expect_asset_owner::<T>(asset, Loan::account_id());

	// pool should be initialised
	assert_eq!(
		loan_nft_class_id,
		Loan::get_loan_nft_class(pool_id).expect("Loan class should be created")
	);
	(pool_id, Asset(loan_nft_class_id, loan_id), asset)
}

fn default_bullet_loan_params() -> LoanType<Rate, Amount> {
	LoanType::BulletLoan(BulletLoan::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100),
		// probability of default is 4%
		Rate::saturating_from_rational(4, 100),
		// loss given default is 50%
		Rate::saturating_from_rational(50, 100),
		// collateral value
		Amount::from_inner(125 * USD),
		// 4%
		math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
		// 2 years
		math::seconds_per_year() * 2,
	))
}

fn default_credit_line_params() -> LoanType<Rate, Amount> {
	LoanType::CreditLine(CreditLine::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100),
		// collateral value
		Amount::from_inner(125 * USD),
	))
}

fn default_credit_line_with_maturity_params() -> LoanType<Rate, Amount> {
	LoanType::CreditLineWithMaturity(CreditLineWithMaturity::new(
		// advance rate 80%
		Rate::saturating_from_rational(80, 100),
		// probability of default is 4%
		Rate::saturating_from_rational(4, 100),
		// loss given default is 50%
		Rate::saturating_from_rational(50, 100),
		// collateral value
		Amount::from_inner(125 * USD),
		// 4%
		math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
		// 2 years
		math::seconds_per_year() * 2,
	))
}

fn price_test_loan<T>(
	admin: T::AccountId,
	pool_id: T::PoolId,
	loan_id: T::LoanId,
	rp: Rate,
	loan_type: LoanType<Rate, Amount>,
) where
	T: pallet_tinlake_investor_pool::Config<PoolId = PoolId>
		+ pallet_loan::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let res = Loan::price_loan(Origin::signed(admin), pool_id, loan_id, rp, loan_type);
	assert_ok!(res);
	let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
	let (got_pool_id, got_loan_id) = match loan_event {
		LoanEvent::LoanPriceSet(pool_id, loan_id) => Some((pool_id, loan_id)),
		_ => None,
	}
	.expect("must be a Loan issue activated event");
	assert_eq!(pool_id, got_pool_id);
	assert_eq!(loan_id, got_loan_id);

	// check loan status as Activated
	let loan_data =
		LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
	assert_eq!(loan_data.status, LoanStatus::Active);
	assert_eq!(loan_data.rate_per_sec, rp);
	assert_eq!(loan_data.loan_type, loan_type);
	assert_eq!(loan_data.ceiling(0), Amount::from_inner(100 * USD));
	assert_eq!(loan_data.write_off_index, None);
	assert!(!loan_data.admin_written_off);
}

fn price_bullet_loan<T>(
	admin: T::AccountId,
	pool_id: T::PoolId,
	loan_id: T::LoanId,
) -> (Rate, LoanType<Rate, Amount>)
where
	T: pallet_tinlake_investor_pool::Config<PoolId = PoolId>
		+ pallet_loan::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let loan_type = default_bullet_loan_params();
	// interest rate is 5%
	let rp = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
	price_test_loan::<T>(admin, pool_id, loan_id, rp, loan_type);
	(rp, loan_type)
}

fn price_credit_line_loan<T>(
	admin: T::AccountId,
	pool_id: T::PoolId,
	loan_id: T::LoanId,
) -> (Rate, LoanType<Rate, Amount>)
where
	T: pallet_tinlake_investor_pool::Config<PoolId = PoolId>
		+ pallet_loan::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let loan_type = default_credit_line_params();
	// interest rate is 5%
	let rp = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
	price_test_loan::<T>(admin, pool_id, loan_id, rp, loan_type);
	(rp, loan_type)
}

fn price_credit_line_with_maturity_loan<T>(
	admin: T::AccountId,
	pool_id: T::PoolId,
	loan_id: T::LoanId,
) -> (Rate, LoanType<Rate, Amount>)
where
	T: pallet_tinlake_investor_pool::Config<PoolId = PoolId>
		+ pallet_loan::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let loan_type = default_credit_line_with_maturity_params();
	// interest rate is 5%
	let rp = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
	price_test_loan::<T>(admin, pool_id, loan_id, rp, loan_type);
	(rp, loan_type)
}

fn close_test_loan<T>(owner: T::AccountId, pool_id: T::PoolId, loan: AssetOf<T>, asset: AssetOf<T>)
where
	T: pallet_tinlake_investor_pool::Config<PoolId = PoolId>
		+ pallet_loan::Config<ClassId = ClassId, LoanId = InstanceId>
		+ frame_system::Config<AccountId = u64>,
{
	let loan_id = loan.1;

	// close the loan
	let res = Loan::close_loan(Origin::signed(owner), pool_id, loan_id);
	assert_ok!(res);

	let (got_pool_id, got_loan_id, got_asset) =
		match fetch_loan_event(last_event()).expect("should be a loan event") {
			LoanEvent::LoanClosed(pool_id, loan_id, asset) => Some((pool_id, loan_id, asset)),
			_ => None,
		}
		.expect("must be a Loan close event");
	assert_eq!(pool_id, got_pool_id);
	assert_eq!(loan_id, got_loan_id);
	assert_eq!(asset, got_asset);

	// check asset owner
	expect_asset_owner::<T>(asset, owner);

	// check loan owner
	expect_asset_owner::<T>(loan, Loan::account_id());

	// check loan status as Closed
	let loan_data =
		LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
	assert_eq!(loan_data.status, LoanStatus::Closed);
}

#[test]
fn test_issue_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();

			// successful issue
			let (pool_id, loan, asset) = issue_test_loan::<MockRuntime>(0, borrower);

			// wrong owner
			let owner2 = 2;
			let res = Loan::issue_loan(Origin::signed(owner2), pool_id, asset);
			assert_err!(res, Error::<MockRuntime>::ErrNotAssetOwner);

			// missing owner
			let instance_id = 100u128.into();
			let res =
				Loan::issue_loan(Origin::signed(owner2), pool_id, Asset(asset.0, instance_id));
			assert_err!(res, Error::<MockRuntime>::ErrNFTOwnerNotFound);

			// trying to issue a loan with loan nft
			let res = Loan::issue_loan(Origin::signed(borrower), pool_id, loan);
			assert_err!(res, Error::<MockRuntime>::ErrNotAValidAsset)
		});
}

#[test]
fn test_price_bullet_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();

			// successful issue
			let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);

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
				Amount::from_inner(125 * USD),
				// 4%
				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
				// maturity date in the past
				1,
			));
			let rp = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
			Timestamp::set_timestamp(100);
			let res = Loan::price_loan(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::ErrLoanValueInvalid);

			// rate_per_sec is invalid
			let loan_type = LoanType::BulletLoan(BulletLoan::new(
				// advance rate 80%
				Rate::saturating_from_rational(80, 100),
				// probability of default is 4%
				Rate::saturating_from_rational(4, 100),
				// loss given default is 50%
				Rate::saturating_from_rational(50, 100),
				// collateral value
				Amount::from_inner(125 * USD),
				// 4%
				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
				// maturity in 2 years
				math::seconds_per_year() * 2,
			));
			let rp = Zero::zero();
			let res = Loan::price_loan(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::ErrLoanValueInvalid);

			// successful activation
			let (rate, loan_type) = price_bullet_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// cannot activate an already activated loan
			let res = Loan::price_loan(Origin::signed(borrower), pool_id, loan.1, rate, loan_type);
			assert_err!(res, Error::<MockRuntime>::ErrLoanIsActive);
		})
}

#[test]
fn test_price_credit_line_with_maturity_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();

			// successful issue
			let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);

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
				Amount::from_inner(125 * USD),
				// 4%
				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
				// maturity date in the past
				1,
			));
			let rp = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
			Timestamp::set_timestamp(100);
			let res = Loan::price_loan(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::ErrLoanValueInvalid);

			// rate_per_sec is invalid
			let loan_type = LoanType::CreditLineWithMaturity(CreditLineWithMaturity::new(
				// advance rate 80%
				Rate::saturating_from_rational(80, 100),
				// probability of default is 4%
				Rate::saturating_from_rational(4, 100),
				// loss given default is 50%
				Rate::saturating_from_rational(50, 100),
				// collateral value
				Amount::from_inner(125 * USD),
				// 4%
				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
				// maturity in 2 years
				math::seconds_per_year() * 2,
			));
			let rp = Zero::zero();
			let res = Loan::price_loan(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::ErrLoanValueInvalid);

			// successful activation
			let (rate, loan_type) = price_bullet_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// cannot activate an already activated loan
			let res = Loan::price_loan(Origin::signed(borrower), pool_id, loan.1, rate, loan_type);
			assert_err!(res, Error::<MockRuntime>::ErrLoanIsActive);
		})
}

#[test]
fn test_price_credit_line_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();

			// successful issue
			let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);

			let loan_id = loan.1;

			// rate_per_sec is invalid
			let loan_type = LoanType::CreditLine(CreditLine::new(
				// advance rate 80%
				Rate::saturating_from_rational(80, 100),
				// collateral value
				Amount::from_inner(125 * USD),
			));
			let rp = Zero::zero();
			let res = Loan::price_loan(Origin::signed(borrower), pool_id, loan_id, rp, loan_type);
			assert_err!(res, Error::<MockRuntime>::ErrLoanValueInvalid);

			// successful activation
			let (rate, loan_type) = price_bullet_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// cannot activate an already activated loan
			let res = Loan::price_loan(Origin::signed(borrower), pool_id, loan.1, rate, loan_type);
			assert_err!(res, Error::<MockRuntime>::ErrLoanIsActive);
		})
}

macro_rules! test_close_loan {
	($price_loan:ident) => {
		TestExternalitiesBuilder::default()
			.build()
			.execute_with(|| {
				let borrower = Borrower::get();
				// successful issue
				let (pool_id, loan, asset) = issue_test_loan::<MockRuntime>(0, borrower);

				// successful activation
				$price_loan::<MockRuntime>(borrower, pool_id, loan.1);

				// successful close of loan
				close_test_loan::<MockRuntime>(borrower, pool_id, loan, asset);
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
				let borrower = Borrower::get();
				// successful issue
				let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());

				// successful activation
				let loan_id = loan.1;
				let (rate, loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// borrow 50 first
				Timestamp::set_timestamp(1);
				let borrow_amount = Amount::from_inner(50 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// check loan data
				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanData should be present");
				// accumulated rate is now rate per sec
				assert_eq!(loan_data.rate_per_sec, rate);
				assert_eq!(loan_data.accumulated_rate, rate);
				assert_eq!(loan_data.last_updated, 1);
				assert_eq!(loan_data.borrowed_amount, Amount::from_inner(50 * USD));
				let p_debt = borrow_amount
					.checked_div(
						&math::convert::<Rate, Amount>(loan_data.accumulated_rate).unwrap(),
					)
					.unwrap();
				assert_eq!(loan_data.principal_debt, p_debt);
				// pool should have 50 less token
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 950 * USD);
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 50 * USD);
				// nav should be updated to latest present value
				let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
				let pv = loan_data.present_value(&vec![]).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// borrow another 20 after 1000 seconds
				Timestamp::set_timestamp(1001);
				let borrow_amount = Amount::from_inner(20 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);
				// check loan data
				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanData should be present");
				// accumulated rate is rate*rate^1000
				assert_eq!(
					loan_data.accumulated_rate,
					checked_pow(rate, 1000).unwrap().checked_mul(&rate).unwrap()
				);
				assert_eq!(loan_data.last_updated, 1001);
				assert_eq!(loan_data.borrowed_amount, Amount::from_inner(70 * USD));
				let c_debt = math::debt(p_debt, loan_data.accumulated_rate).unwrap();
				let p_debt = c_debt
					.checked_add(&borrow_amount)
					.unwrap()
					.checked_div(
						&math::convert::<Rate, Amount>(loan_data.accumulated_rate).unwrap(),
					)
					.unwrap();
				assert_eq!(loan_data.principal_debt, p_debt);

				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 930 * USD);
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 70 * USD);
				let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
				let pv = loan_data.present_value(&vec![]).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// try to borrow more than ceiling
				// borrow another 40 after 1000 seconds
				Timestamp::set_timestamp(2001);
				let borrow_amount = Amount::from_inner(40 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_err!(res, Error::<MockRuntime>::ErrLoanCeilingReached);

				// try to borrow after maturity date
				let mut now = 2001;
				if $maturity_check {
					now = loan_type.maturity_date().unwrap() + 1;
					Timestamp::set_timestamp(now);
					let res =
						Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
					assert_err!(res, Error::<MockRuntime>::ErrLoanMaturityDatePassed);
				}

				// written off loan cannot borrow
				// add write off groups
				let risk_admin = RiskAdmin::get();
				assert_ok!(
					pallet_tinlake_investor_pool::Pallet::<MockRuntime>::approve_role_for(
						RawOrigin::Signed(PoolAdmin::get()).into(),
						pool_id,
						PoolRole::RiskAdmin,
						vec![
							<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(
								risk_admin
							)
						]
					)
				);
				for group in vec![(3, 0), (5, 15), (7, 20), (20, 30), (120, 100)] {
					let res = Loan::add_write_off_group_to_pool(
						Origin::signed(risk_admin),
						pool_id,
						WriteOffGroup {
							percentage: Rate::saturating_from_rational(group.1, 100),
							overdue_days: group.0,
						},
					);
					assert_ok!(res);
				}

				let res = Loan::admin_write_off_loan(Origin::signed(risk_admin), pool_id, loan_id, 0);
				assert_ok!(res);

				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_err!(res, Error::<MockRuntime>::ErrLoanWrittenOffByAdmin);

				// update nav
				let updated_nav = <Loan as TPoolNav<PoolId, Amount>>::update_nav(pool_id).unwrap();
				// check loan data
				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanData should be present");
				// after maturity should be current outstanding
				let (_acc_rate, debt) = loan_data.accrue(now).unwrap();
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
				let (pool_id, loan, asset) = issue_test_loan::<MockRuntime>(0, borrower);
				let pool_account = PoolLocator { pool_id }.into_account();
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 1000 * USD);

				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());
				let loan_id = loan.1;

				// successful activation
				let (rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

				// try repay without any borrowed
				let repay_amount = Amount::from_inner(20 * USD);
				let res = Loan::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
				assert_ok!(res);

				// borrow 50
				Timestamp::set_timestamp(1);
				let borrow_amount = Amount::from_inner(50 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// check loan data
				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanData should be present");
				// accumulated rate is now rate per sec
				assert_eq!(loan_data.accumulated_rate, rate);
				assert_eq!(loan_data.last_updated, 1);
				assert_eq!(loan_data.borrowed_amount, Amount::from_inner(50 * USD));
				let p_debt = borrow_amount
					.checked_div(
						&math::convert::<Rate, Amount>(loan_data.accumulated_rate).unwrap(),
					)
					.unwrap();
				assert_eq!(loan_data.principal_debt, p_debt);
				// pool should have 50 less token
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 950 * USD);
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 50 * USD);
				// nav should be updated to latest present value
				let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
				let pv = loan_data.present_value(&vec![]).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// repay 20 after 1000 seconds
				Timestamp::set_timestamp(1001);
				let repay_amount = Amount::from_inner(20 * USD);
				let res = Loan::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
				assert_ok!(res);

				// check loan data
				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanData should be present");
				// accumulated rate is now rate per sec
				assert_eq!(
					loan_data.accumulated_rate,
					checked_pow(rate, 1000).unwrap().checked_mul(&rate).unwrap()
				);
				assert_eq!(loan_data.last_updated, 1001);
				assert_eq!(loan_data.borrowed_amount, Amount::from_inner(50 * USD));
				// principal debt should still be more than 30 due to interest
				assert!(loan_data.principal_debt > Amount::from_inner(30 * USD));
				// pool should have 30 less token
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				assert_eq!(pool_balance, 970 * USD);
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, 30 * USD);
				// nav should be updated to latest present value
				let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
				let pv = loan_data.present_value(&vec![]).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// repay 30 more after another 1000 seconds
				Timestamp::set_timestamp(2001);
				let repay_amount = Amount::from_inner(30 * USD);
				let res = Loan::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
				assert_ok!(res);

				// try and close the loan
				let res = Loan::close_loan(Origin::signed(borrower), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::ErrLoanNotRepaid);

				// check loan data
				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanData should be present");
				// nav should be updated to latest present value
				let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
				let pv = loan_data.present_value(&vec![]).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");

				// repay the interest
				// 50 for 1000 seconds
				let amount = Amount::from_inner(50 * USD);
				let p_debt = amount
					.checked_div(&math::convert::<Rate, Amount>(loan_data.rate_per_sec).unwrap())
					.unwrap();
				let rate_after_1000 = checked_pow(loan_data.rate_per_sec, 1001).unwrap();
				let debt_after_1000 = p_debt
					.checked_mul(&math::convert::<Rate, Amount>(rate_after_1000).unwrap())
					.unwrap();

				// 30 for 1000 seconds
				let p_debt = debt_after_1000
					.checked_sub(&Amount::from_inner(20 * USD))
					.unwrap()
					.checked_div(&math::convert::<Rate, Amount>(rate_after_1000).unwrap())
					.unwrap();
				let rate_after_2000 = checked_pow(loan_data.rate_per_sec, 2001).unwrap();
				let debt_after_2000 = p_debt
					.checked_mul(&math::convert::<Rate, Amount>(rate_after_2000).unwrap())
					.unwrap();
				let p_debt = debt_after_2000
					.checked_sub(&Amount::from_inner(30 * USD))
					.unwrap()
					.checked_div(&math::convert::<Rate, Amount>(rate_after_2000).unwrap())
					.unwrap();
				assert_eq!(loan_data.principal_debt, p_debt);

				// debt after 3000 seconds
				Timestamp::set_timestamp(3001);
				let rate_after_3000 = checked_pow(loan_data.rate_per_sec, 3001).unwrap();
				let debt = p_debt
					.checked_mul(&math::convert::<Rate, Amount>(rate_after_3000).unwrap())
					.unwrap();

				// transfer exact interest amount to owner account from dummy account 2
				let dummy: u64 = 7;
				let transfer_amount: Balance = debt.into_inner().into();
				let dest =
					<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(
						borrower,
					);
				let res = Tokens::transfer(
					Origin::signed(dummy),
					dest,
					CurrencyId::Usd,
					transfer_amount,
				);
				assert_ok!(res);

				// repay the interest
				let repay_amount = debt;
				let res = Loan::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
				assert_ok!(res);

				// close loan
				let res = Loan::close_loan(Origin::signed(borrower), pool_id, loan_id);
				assert_ok!(res);

				// check loan data
				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanData should be present");
				assert_eq!(loan_data.status, LoanStatus::Closed);
				assert_eq!(loan_data.principal_debt, Zero::zero());
				assert_eq!(loan_data.borrowed_amount, Amount::from_inner(50 * USD));
				assert_eq!(loan_data.last_updated, 3001);
				// nav should be updated to latest present value and should be zero
				let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
				let pv = loan_data.present_value(&vec![]).unwrap();
				assert_eq!(current_nav, pv, "should be same due to single loan");
				assert_eq!(current_nav, Zero::zero());

				// pool balance should be 1000 + interest
				let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
				let expected_balance = 1000 * USD + transfer_amount;
				assert_eq!(pool_balance, expected_balance);

				// owner balance should be zero
				let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
				assert_eq!(owner_balance, Zero::zero());

				// owner account should own the asset NFT
				expect_asset_owner::<MockRuntime>(asset, borrower);

				// Loan account should own the loan NFT
				expect_asset_owner::<MockRuntime>(loan, Loan::account_id());

				// check nav
				let res = Loan::update_nav_of_pool(pool_id);
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
	($price_loan:ident,$moving_ceiling:expr,$admin_write_off:expr,$pv_1:expr,$pv_200:expr) => {
		TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let borrower: u64 = Borrower::get();
			// successful issue
			let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);
			let pool_account = PoolLocator { pool_id }.into_account();
			let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
			assert_eq!(pool_balance, 1000 * USD);

			let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
			assert_eq!(owner_balance, Zero::zero());
			let loan_id = loan.1;

			// successful activation
			let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// present value should still be zero
			let loan_data =
				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
			let pv = loan_data
				.present_value(&vec![])
				.expect("present value should not return none");
			assert_eq!(pv, Zero::zero());

			// borrow 50 amount at the instant
			let borrow_amount = Amount::from_inner(50 * USD);
			let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
			assert_ok!(res);

			// check present value
			let loan_data =
				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
			let pv = loan_data
				.present_value(&vec![])
				.expect("present value should not return none");
			assert_eq!(
				pv,
				$pv_1
			);

			// pass some time. maybe 200 days
			let after_200_days = 3600 * 24 * 200;
			Timestamp::set_timestamp(after_200_days);
			let res = Loan::update_nav_of_pool(pool_id);
			assert_ok!(res);
			let (nav, ..) = res.unwrap();
			// present value should be 50.05
			assert_eq!(
				nav,
				$pv_200
			);

			if $moving_ceiling {
				// can borrow upto ceiling
				// ceiling = 125 * 0.8 - debt
				// check present value
				let loan_data =
					LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
				let (_, debt) = loan_data.accrue(after_200_days).unwrap();
				let borrow_amount = Amount::from_inner(100 * USD).checked_sub(&debt).unwrap();
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// cannot borrow more than ceiling, 1
				let borrow_amount = Amount::from_inner(1 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_err!(res, Error::<MockRuntime>::ErrLoanCeilingReached);

				// payback 50 and borrow more later
				let repay_amount = Amount::from_inner(50 * USD);
				let res = Loan::repay(Origin::signed(borrower), pool_id, loan_id, repay_amount);
				assert_ok!(res);

				// pass some time. maybe 500 days
				let after_500_days = 3600 * 24 * 300;
				Timestamp::set_timestamp(after_500_days);

				// you cannot borrow more than 50 since the debt is more than 50 by now
				let borrow_amount = Amount::from_inner(50 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_err!(res, Error::<MockRuntime>::ErrLoanCeilingReached);

				// borrow 40 maybe
				let borrow_amount = Amount::from_inner(40 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);
			} else {
				// borrow another 50 and
				let borrow_amount = Amount::from_inner(50 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_ok!(res);

				// cannot borrow more than ceiling, 1
				let borrow_amount = Amount::from_inner(1 * USD);
				let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
				assert_err!(res, Error::<MockRuntime>::ErrLoanCeilingReached);
			}

			// let the maturity has passed 2 years + 10 day
			let after_2_years = (math::seconds_per_year() * 2) + math::seconds_per_day() * 10;
			let loan_data =
				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
			let (_acc_rate, debt) = loan_data.accrue(after_2_years).unwrap();
			Timestamp::set_timestamp(after_2_years);
			let res = Loan::update_nav_of_pool(pool_id);
			assert_ok!(res);
			let (pv, ..) = res.unwrap();
			// present value should be equal to current outstanding debt
			assert_eq!(pv, debt);
			let (nav, ..) = res.unwrap();
			assert_eq!(pv, nav);

			// call update nav extrinsic and check for event
			let res = Loan::update_nav(Origin::signed(borrower), pool_id);
			assert_ok!(res);
			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
			let (got_pool_id, updated_nav) = match loan_event {
				LoanEvent::NAVUpdated(pool_id, update_nav) => Some((pool_id, update_nav)),
				_ => None,
			}
			.expect("must be a Nav updated event");
			assert_eq!(pool_id, got_pool_id);
			assert_eq!(updated_nav, nav);

			let risk_admin = RiskAdmin::get();
			assert_ok!(
				pallet_tinlake_investor_pool::Pallet::<MockRuntime>::approve_role_for(
					RawOrigin::Signed(PoolAdmin::get()).into(),
					pool_id,
					PoolRole::RiskAdmin,
					vec![
						<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(
							risk_admin
						)
					]
				)
			);
			// write off the loan and check for updated nav
			for group in vec![(3, 10), (5, 15), (7, 20), (20, 30)] {
				let group = WriteOffGroup {
					percentage: Rate::saturating_from_rational(group.1, 100),
					overdue_days: group.0,
				};
				let res =
					Loan::add_write_off_group_to_pool(Origin::signed(risk_admin), pool_id, group);
				assert_ok!(res);
			}

			if $admin_write_off {
				let res = Loan::admin_write_off_loan(Origin::signed(risk_admin), pool_id, loan_id, 2);
				assert_ok!(res);
			} else{
				// write off loan. someone calls write off
				let res = Loan::write_off_loan(Origin::signed(100), pool_id, loan_id);
				assert_ok!(res);
			}
			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
			let (_pool_id, _loan_id, write_off_index) = match loan_event {
				LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
					Some((pool_id, loan_id, write_off_index))
				}
				_ => None,
			}
			.expect("must be a loan written off event");
			// it must be 2 with overdue days as 7 and write off percentage as 20%
			assert_eq!(write_off_index, 2);

			// update nav
			let res = Loan::update_nav(Origin::signed(borrower), pool_id);
			assert_ok!(res);
			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
			let (_pool_id, updated_nav) = match loan_event {
				LoanEvent::NAVUpdated(pool_id, update_nav) => Some((pool_id, update_nav)),
				_ => None,
			}
			.expect("must be a Nav updated event");

			// updated nav should be (1-20%) outstanding debt
			let expected_nav =
				math::convert::<Rate, Amount>(Rate::saturating_from_rational(20, 100))
					.and_then(|rate| debt.checked_mul(&rate))
					.and_then(|written_off_amount| debt.checked_sub(&written_off_amount))
					.unwrap();
			assert_eq!(expected_nav, updated_nav);
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
		Amount::from_inner(48969664319886742817u128),
		// present value after 200 days
		Amount::from_inner(50054820713981957069u128)
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
		Amount::from_inner(48969664319886742817u128),
		// present value after 200 days
		Amount::from_inner(50054820713981957069u128)
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
		Amount::from_inner(49999999999999999999u128),
		// present value after 200 days
		Amount::from_inner(51388800811704851007u128)
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
			create_pool::<MockRuntime>(
				pool_id,
				pool_admin,
				JuniorInvestor::get(),
				SeniorInvestor::get(),
				CurrencyId::Usd,
			);
			let pr_pool_id: PoolIdOf<MockRuntime> = pool_id.into();
			initialise_test_pool::<MockRuntime>(pr_pool_id, 1, pool_admin, None);
			assert_ok!(
				pallet_tinlake_investor_pool::Pallet::<MockRuntime>::approve_role_for(
					RawOrigin::Signed(pool_admin).into(),
					pool_id,
					PoolRole::RiskAdmin,
					vec![
						<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(
							risk_admin
						)
					]
				)
			);

			// fetch write off groups
			let groups = PoolWriteOffGroups::<MockRuntime>::get(pool_id);
			assert_eq!(groups, vec![]);

			for percentage in vec![10, 20, 30, 40, 30, 50, 70, 100] {
				// add a new write off group
				let group = WriteOffGroup {
					percentage: Rate::saturating_from_rational(percentage, 100),
					overdue_days: 3,
				};
				let res =
					Loan::add_write_off_group_to_pool(Origin::signed(risk_admin), pool_id, group);
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
			};
			let res = Loan::add_write_off_group_to_pool(Origin::signed(risk_admin), pool_id, group);
			assert_err!(res, Error::<MockRuntime>::ErrInvalidWriteOffGroup);
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
			let (pool_id, loan, _asset) = issue_test_loan::<MockRuntime>(0, borrower);
			let pool_account = PoolLocator { pool_id }.into_account();
			let pool_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &pool_account);
			assert_eq!(pool_balance, 1000 * USD);

			let owner_balance = balance_of::<MockRuntime>(CurrencyId::Usd, &borrower);
			assert_eq!(owner_balance, Zero::zero());
			let loan_id = loan.1;

			// successful activation
			let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// borrow 50
			Timestamp::set_timestamp(1);
			let borrow_amount = Amount::from_inner(50 * USD);
			let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
			assert_ok!(res);

			// after one year
			// anyone can trigger the call
			let caller = 100;
			Timestamp::set_timestamp(math::seconds_per_year());
			let res = Loan::write_off_loan(Origin::signed(caller), pool_id, loan_id);
			assert_err!(res, Error::<MockRuntime>::ErrLoanHealthy);

			// let the maturity date passes + 1 day
			let t = math::seconds_per_year() * 2 + math::seconds_per_day();
			Timestamp::set_timestamp(t);
			let res = Loan::write_off_loan(Origin::signed(caller), pool_id, loan_id);
			assert_err!(res, Error::<MockRuntime>::ErrNoValidWriteOffGroup);

			// add write off groups
			let risk_admin = RiskAdmin::get();
			assert_ok!(
				pallet_tinlake_investor_pool::Pallet::<MockRuntime>::approve_role_for(
					RawOrigin::Signed(pool_admin).into(),
					pool_id,
					PoolRole::RiskAdmin,
					vec![
						<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(
							risk_admin
						)
					]
				)
			);
			for group in vec![(3, 10), (5, 15), (7, 20), (20, 30)] {
				let res = Loan::add_write_off_group_to_pool(
					Origin::signed(risk_admin),
					pool_id,
					WriteOffGroup {
						percentage: Rate::saturating_from_rational(group.1, 100),
						overdue_days: group.0,
					},
				);
				assert_ok!(res);
			}

			// same since write off group is missing
			let t = math::seconds_per_year() * 2 + math::seconds_per_day();
			Timestamp::set_timestamp(t);
			let res = Loan::write_off_loan(Origin::signed(caller), pool_id, loan_id);
			assert_err!(res, Error::<MockRuntime>::ErrNoValidWriteOffGroup);

			// days, index
			for days_index in vec![(3, 0), (5, 1), (7, 2), (20, 3)] {
				// move to more than 3 days
				let t = math::seconds_per_year() * 2 + math::seconds_per_day() * days_index.0;
				Timestamp::set_timestamp(t);
				let res = Loan::write_off_loan(Origin::signed(caller), pool_id, loan_id);
				assert_ok!(res);

				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
				let (_pool_id, _loan_id, write_off_index) = match loan_event {
					LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
						Some((pool_id, loan_id, write_off_index))
					}
					_ => None,
				}
				.expect("must be a Loan issue event");
				assert_eq!(write_off_index, days_index.1);
				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
					.expect("LoanData should be present");
				assert_eq!(loan_data.write_off_index, Some(days_index.1));
				assert!(!loan_data.admin_written_off);
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

			// successful activation
			let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// borrow 50
			Timestamp::set_timestamp(1);
			let borrow_amount = Amount::from_inner(50 * USD);
			let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
			assert_ok!(res);

			// after one year
			// caller should be admin, can write off before maturity
			let risk_admin = RiskAdmin::get();
			assert_ok!(
				pallet_tinlake_investor_pool::Pallet::<MockRuntime>::approve_role_for(
					RawOrigin::Signed(pool_admin).into(),
					pool_id,
					PoolRole::RiskAdmin,
					vec![
						<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(
							risk_admin
						)
					]
				)
			);

			Timestamp::set_timestamp(math::seconds_per_year());
			let res = Loan::admin_write_off_loan(Origin::signed(risk_admin), pool_id, loan_id, 0);
			assert_err!(res, Error::<MockRuntime>::ErrInvalidWriteOffGroupIndex);

			// let the maturity date passes + 1 day
			let t = math::seconds_per_year() * 2 + math::seconds_per_day();
			Timestamp::set_timestamp(t);
			let res = Loan::admin_write_off_loan(Origin::signed(risk_admin), pool_id, loan_id, 0);
			assert_err!(res, Error::<MockRuntime>::ErrInvalidWriteOffGroupIndex);

			// add write off groups
			for group in vec![(3, 10), (5, 15), (7, 20), (20, 30)] {
				let res = Loan::add_write_off_group_to_pool(
					Origin::signed(risk_admin),
					pool_id,
					WriteOffGroup {
						percentage: Rate::saturating_from_rational(group.1, 100),
						overdue_days: group.0,
					},
				);
				assert_ok!(res);
			}

			// verify and check before and after maturity
			for time in vec![
				math::seconds_per_year(),
				math::seconds_per_year() * 2 + math::seconds_per_day() * 3,
			] {
				Timestamp::set_timestamp(time);
				for index in vec![0, 3, 2, 1, 0] {
					let res = Loan::admin_write_off_loan(
						Origin::signed(risk_admin),
						pool_id,
						loan_id,
						index,
					);
					assert_ok!(res);

					let loan_event =
						fetch_loan_event(last_event()).expect("should be a loan event");
					let (_pool_id, _loan_id, write_off_index) = match loan_event {
						LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
							Some((pool_id, loan_id, write_off_index))
						}
						_ => None,
					}
					.expect("must be a Loan issue event");
					assert_eq!(write_off_index, index);
					let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
						.expect("LoanData should be present");
					assert_eq!(loan_data.write_off_index, Some(index));
					assert!(loan_data.admin_written_off);
				}
			}

			// permission less write off should not work once written off by admin
			let res = Loan::write_off_loan(Origin::signed(100), pool_id, loan_id);
			assert_err!(res, Error::<MockRuntime>::ErrLoanWrittenOffByAdmin)
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

			// successful activation
			let (_rate, _loan_type) = $price_loan::<MockRuntime>(borrower, pool_id, loan_id);

			// borrow 50
			Timestamp::set_timestamp(1);
			let borrow_amount = Amount::from_inner(50 * USD);
			let res = Loan::borrow(Origin::signed(borrower), pool_id, loan_id, borrow_amount);
			assert_ok!(res);

			// let the maturity pass and closing loan should not work
			Timestamp::set_timestamp(math::seconds_per_year() * 2 + 5 * math::seconds_per_day());
			let res = Loan::close_loan(Origin::signed(borrower), pool_id, loan_id);
			assert_err!(res, Error::<MockRuntime>::ErrLoanNotRepaid);

			// add write off groups
			let risk_admin = RiskAdmin::get();
			assert_ok!(
				pallet_tinlake_investor_pool::Pallet::<MockRuntime>::approve_role_for(
					RawOrigin::Signed(pool_admin).into(),
					pool_id,
					PoolRole::RiskAdmin,
					vec![
						<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(
							risk_admin
						)
					]
				)
			);
			for group in vec![(3, 10), (5, 15), (7, 20), (20, 30), (120, 100)] {
				let res = Loan::add_write_off_group_to_pool(
					Origin::signed(risk_admin),
					pool_id,
					WriteOffGroup {
						percentage: Rate::saturating_from_rational(group.1, 100),
						overdue_days: group.0,
					},
				);
				assert_ok!(res);
			}

			if $maturity_checks {
				// write off loan but should not be able to close since its not 100% write off
				let res = Loan::write_off_loan(Origin::signed(200), pool_id, loan_id);
				assert_ok!(res);
				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
				let (_pool_id, _loan_id, write_off_index) = match loan_event {
					LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
						Some((pool_id, loan_id, write_off_index))
					}
					_ => None,
				}
				.expect("must be a Loan issue event");
				assert_eq!(write_off_index, 1);
				let res = Loan::close_loan(Origin::signed(borrower), pool_id, loan_id);
				assert_err!(res, Error::<MockRuntime>::ErrLoanNotRepaid);

				// let it be 120 days beyond maturity, we write off 100% now
				Timestamp::set_timestamp(math::seconds_per_year() * 2 + 120 * math::seconds_per_day());
				let res = Loan::write_off_loan(Origin::signed(200), pool_id, loan_id);
				assert_ok!(res);
			} else {
				// write off as admin
				let res = Loan::admin_write_off_loan(Origin::signed(risk_admin), pool_id, loan_id, 4);
				assert_ok!(res);
			}

			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
			let (_pool_id, _loan_id, write_off_index) = match loan_event {
				LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
					Some((pool_id, loan_id, write_off_index))
				}
				_ => None,
			}
			.expect("must be a Loan written off event");
			assert_eq!(write_off_index, 4);

			// nav should be zero
			let res = Loan::update_nav(Origin::signed(borrower), pool_id);
			assert_ok!(res);
			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
			let (got_pool_id, updated_nav) = match loan_event {
				LoanEvent::NAVUpdated(pool_id, update_nav) => Some((pool_id, update_nav)),
				_ => None,
			}
			.expect("must be a Nav updated event");
			assert_eq!(pool_id, got_pool_id);
			assert_eq!(updated_nav, Zero::zero());

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
