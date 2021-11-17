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
use crate::mock::TestExternalitiesBuilder;
use crate::mock::{Event, GetUSDCurrencyId, Loan, MockRuntime, Origin, Timestamp, Tokens, Uniques};
use frame_support::traits::tokens::nonfungibles::Create;
use frame_support::{assert_err, assert_ok};
use loan_type::{BulletLoan, LoanType};
use orml_traits::MultiCurrency;
use pallet_loan::Event as LoanEvent;
use runtime_common::{Amount, Balance, ClassId, PoolId, Rate, CFG as USD};
use sp_arithmetic::traits::{checked_pow, CheckedDiv, CheckedMul, CheckedSub};
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::{One, StaticLookup};

fn create_nft_class<T>(
	class_id: u64,
	owner: T::AccountId,
	maybe_admin: Option<T::AccountId>,
) -> <T as Config>::ClassId
where
	T: frame_system::Config<AccountId = u64> + pallet_loan::Config,
	<T as Config>::ClassId: From<u64>,
{
	// Create registry, get registry id. Shouldn't fail.
	let admin = maybe_admin.unwrap_or(owner);
	<Uniques as Create<T::AccountId>>::create_class(&class_id, &owner, &admin)
		.expect("class creation should not fail");
	class_id.into()
}

fn mint_nft<T>(owner: T::AccountId, class_id: T::ClassId) -> T::LoanId
where
	T: frame_system::Config + pallet_loan::Config,
{
	let loan_id: T::LoanId = 1u128.into();
	T::NonFungible::mint_into(&class_id.into(), &loan_id.into(), &owner)
		.expect("mint should not fail");
	loan_id
}

fn create_pool<T, GetCurrencyId>(owner: T::AccountId) -> PoolId
where
	T: pallet_pool::Config<PoolId = PoolId> + frame_system::Config,
	GetCurrencyId: Get<pallet_pool::CurrencyIdOf<T>>,
{
	// currencyId is 1
	pallet_pool::Pallet::<T>::create_new_pool(owner, "USD Pool".into(), GetCurrencyId::get())
}

fn initialise_pool<T>(pool_id: T::PoolId) -> T::ClassId
where
	T: pallet_pool::Config<PoolId = PoolId>
		+ frame_system::Config
		+ pallet_loan::Config<ClassId = ClassId>,
{
	let class_id = create_nft_class::<MockRuntime>(1, 1, Some(Loan::account_id()));
	Loan::initialise_pool(Origin::signed(1), pool_id, class_id)
		.expect("initialisation of pool should not fail");
	class_id
}

// Return last triggered event
fn last_event() -> Event {
	frame_system::Pallet::<MockRuntime>::events()
		.pop()
		.map(|item| item.event)
		.expect("Event expected")
}

fn expect_event<E: Into<Event>>(event: E) {
	assert_eq!(last_event(), event.into());
}

fn expect_asset_owner<T: frame_system::Config + pallet_loan::Config>(
	asset: AssetOf<T>,
	owner: T::AccountId,
) {
	let (class_id, instance_id) = asset.destruct();
	assert_eq!(
		<T as pallet_loan::Config>::NonFungible::owner(&class_id.into(), &instance_id.into())
			.unwrap(),
		owner
	);
}

fn fetch_loan_event(event: Event) -> Option<LoanEvent<MockRuntime>> {
	match event {
		Event::Loan(loan_event) => Some(loan_event),
		_ => None,
	}
}

type MultiCurrencyBalanceOf<T> = <<T as pallet_pool::Config>::MultiCurrency as MultiCurrency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

fn balance_of<T, GetCurrencyId>(account: &T::AccountId) -> MultiCurrencyBalanceOf<T>
where
	T: pallet_pool::Config + frame_system::Config,
	GetCurrencyId: Get<pallet_pool::CurrencyIdOf<T>>,
{
	<T as pallet_pool::Config>::MultiCurrency::total_balance(GetCurrencyId::get(), account)
}

#[test]
fn issue_loan() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let owner: u64 = 10;
			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
			let loan_nft_class_id = initialise_pool::<MockRuntime>(pool_id);
			let asset_class = create_nft_class::<MockRuntime>(2, owner, None);
			let instance_id = mint_nft::<MockRuntime>(owner, asset_class);
			let res = Loan::issue_loan(
				Origin::signed(owner),
				pool_id,
				Asset(asset_class, instance_id),
			);
			assert_ok!(res);

			// post issue checks
			// token nonce should 2
			assert_eq!(NextLoanId::<MockRuntime>::get(), 2u128.into());

			// loanId should be 1
			let loan_id = 1u128.into();
			// event should be emitted
			expect_event(LoanEvent::LoanIssued(pool_id, loan_id));
			let loan_data =
				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");

			// asset is same as we sent before
			assert_eq!(loan_data.asset, Asset(asset_class, instance_id));
			assert_eq!(loan_data.status, LoanStatus::Issued);

			// asset owner is loan pallet
			expect_asset_owner::<MockRuntime>(Asset(asset_class, instance_id), Loan::account_id());

			// wrong owner
			let owner2 = 2;
			let res = Loan::issue_loan(
				Origin::signed(owner2),
				pool_id,
				Asset(asset_class, instance_id),
			);
			assert_err!(res, Error::<MockRuntime>::ErrNotNFTOwner);

			// missing owner
			let instance_id = 100u128.into();
			let res = Loan::issue_loan(
				Origin::signed(owner2),
				pool_id,
				Asset(asset_class, instance_id),
			);
			assert_err!(res, Error::<MockRuntime>::ErrNFTOwnerNotFound);

			// trying to issue a loan with loan nft
			assert_eq!(
				loan_nft_class_id,
				PoolToLoanNftClass::<MockRuntime>::get(pool_id)
					.expect("Registry should be created")
			);
			let res = Loan::issue_loan(
				Origin::signed(owner),
				pool_id,
				Asset(loan_nft_class_id, loan_id),
			);
			assert_err!(res, Error::<MockRuntime>::ErrNotAValidAsset)
		});
}

// #[test]
// fn activate_loan() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let owner: u64 = 100;
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
// 			let asset_registry = create_nft_class::<MockRuntime>(owner);
// 			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
// 			let res = Loan::issue_loan(
// 				Origin::signed(owner),
// 				pool_id,
// 				Asset(asset_registry, token_id),
// 			);
// 			assert_ok!(res);
//
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
//
// 			let oracle: u64 = 1;
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			// interest rate is 5%
// 			let rp = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			let res = Loan::activate_loan(Origin::signed(oracle), pool_id, loan_id, rp, loan_type);
// 			assert_ok!(res);
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanActivated(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue activated event");
// 			// check loan status as Activated
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			assert_eq!(loan_data.status, LoanStatus::Active);
// 			assert_eq!(loan_data.rate_per_sec, rp);
// 			assert_eq!(loan_data.loan_type, loan_type);
// 			assert_eq!(loan_data.ceiling, Amount::from_inner(100 * USD));
// 			assert_eq!(loan_data.write_off_index, None);
// 			assert!(!loan_data.admin_written_off);
//
// 			// cannot activate an already activated loan
// 			let res = Loan::activate_loan(
// 				Origin::signed(oracle),
// 				pool_id,
// 				loan_id,
// 				Rate::one(),
// 				loan_type,
// 			);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanIsActive);
// 		})
// }
//
// #[test]
// fn close_loan() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let owner: u64 = 10;
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
// 			let asset_registry = create_nft_class::<MockRuntime>(owner);
// 			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
// 			let res = Loan::issue_loan(
// 				Origin::signed(owner),
// 				pool_id,
// 				Asset(asset_registry, token_id),
// 			);
// 			assert_ok!(res);
//
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
//
// 			// invalid loan details
// 			let oracle: u64 = 1;
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity date in the past
// 				1,
// 			));
// 			// interest rate is 5%
// 			let rp = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			Timestamp::set_timestamp(100);
// 			let res = Loan::activate_loan(Origin::signed(oracle), pool_id, loan_id, rp, loan_type);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanValueInvalid);
//
// 			// ceiling is zero
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 0%
// 				Zero::zero(),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity in 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			let res = Loan::activate_loan(Origin::signed(oracle), pool_id, loan_id, rp, loan_type);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanValueInvalid);
//
// 			// rate_per_sec is invalid
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity in 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			// interest rate is 0
// 			let rp = Zero::zero();
// 			let res = Loan::activate_loan(Origin::signed(oracle), pool_id, loan_id, rp, loan_type);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanValueInvalid);
//
// 			// activate loan
// 			// interest rate is 5%
// 			let rp = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			let res = Loan::activate_loan(Origin::signed(oracle), pool_id, loan_id, rp, loan_type);
// 			assert_ok!(res);
//
// 			// close the loan
// 			let res = Loan::close_loan(Origin::signed(owner), pool_id, loan_id);
// 			assert_ok!(res);
//
// 			let (pool_id, loan_id, asset) = match fetch_loan_event(last_event())
// 				.expect("should be a loan event")
// 			{
// 				LoanEvent::LoanClosed(pool_id, loan_id, asset) => Some((pool_id, loan_id, asset)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan close event");
// 			assert_eq!(asset, Asset(asset_registry, token_id));
//
// 			// check asset owner
// 			expect_asset_owner::<MockRuntime>(Asset(asset_registry, token_id), owner);
//
// 			// check nft loan owner loan pallet
// 			let loan_nft_registry = PoolToLoanNftClass::<MockRuntime>::get(pool_id)
// 				.expect("must have a loan nft registry created");
// 			expect_asset_owner::<MockRuntime>(
// 				Asset(loan_nft_registry, loan_id),
// 				Loan::account_id(),
// 			);
//
// 			// check loan status as Closed
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			assert_eq!(loan_data.status, LoanStatus::Closed);
// 		})
// }
//
// #[test]
// fn borrow_loan() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let pool_account = pallet_pool::Pallet::<MockRuntime>::account_id();
// 			let owner: u64 = 10;
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 1000 * USD);
//
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, Zero::zero());
//
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
// 			let asset_registry = create_nft_class::<MockRuntime>(owner);
// 			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
// 			let res = Loan::issue_loan(
// 				Origin::signed(owner),
// 				pool_id,
// 				Asset(asset_registry, token_id),
// 			);
// 			assert_ok!(res);
//
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
//
// 			// activate loan
// 			// interest rate is 5%
// 			let rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			let oracle: u64 = 1;
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity in 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			let res = Loan::activate_loan(
// 				Origin::signed(oracle),
// 				pool_id,
// 				loan_id,
// 				rate,
// 				// ceiling is 100 USD
// 				loan_type,
// 			);
// 			assert_ok!(res);
//
// 			// borrow 50 first
// 			Timestamp::set_timestamp(1);
// 			let borrow_amount = Amount::from_inner(50 * USD);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_ok!(res);
//
// 			// check loan data
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			// accumulated rate is now rate per sec
// 			assert_eq!(loan_data.accumulated_rate, rate);
// 			assert_eq!(loan_data.last_updated, 1);
// 			assert_eq!(loan_data.borrowed_amount, Amount::from_inner(50 * USD));
// 			let p_debt = borrow_amount
// 				.checked_div(&math::convert::<Rate, Amount>(loan_data.accumulated_rate).unwrap())
// 				.unwrap();
// 			assert_eq!(loan_data.principal_debt, p_debt);
// 			// pool should have 50 less token
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 950 * USD);
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, 50 * USD);
// 			// nav should be updated to latest present value
// 			let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
// 			let pv = loan_data.present_value().unwrap();
// 			assert_eq!(current_nav, pv, "should be same due to single loan");
//
// 			// borrow another 20 after 1000 seconds
// 			Timestamp::set_timestamp(1001);
// 			let borrow_amount = Amount::from_inner(20 * USD);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_ok!(res);
// 			// check loan data
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			// accumulated rate is rate*rate^1000
// 			assert_eq!(
// 				loan_data.accumulated_rate,
// 				checked_pow(rate, 1000).unwrap().checked_mul(&rate).unwrap()
// 			);
// 			assert_eq!(loan_data.last_updated, 1001);
// 			assert_eq!(loan_data.borrowed_amount, Amount::from_inner(70 * USD));
// 			let c_debt = math::debt(p_debt, loan_data.accumulated_rate).unwrap();
// 			let p_debt = c_debt
// 				.checked_add(&borrow_amount)
// 				.unwrap()
// 				.checked_div(&math::convert::<Rate, Amount>(loan_data.accumulated_rate).unwrap())
// 				.unwrap();
// 			assert_eq!(loan_data.principal_debt, p_debt);
//
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 930 * USD);
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, 70 * USD);
// 			let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
// 			let pv = loan_data.present_value().unwrap();
// 			assert_eq!(current_nav, pv, "should be same due to single loan");
//
// 			// try to borrow more than ceiling
// 			// borrow another 40 after 1000 seconds
// 			Timestamp::set_timestamp(2001);
// 			let borrow_amount = Amount::from_inner(40 * USD);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanCeilingReached);
//
// 			// try to borrow after maturity date
// 			let now = loan_type.maturity_date().unwrap() + 1;
// 			Timestamp::set_timestamp(now);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanMaturityDatePassed);
//
// 			// update nav
// 			let updated_nav = <Loan as TPoolNav<PoolId, Amount>>::update_nav(pool_id).unwrap();
// 			// check loan data
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			// after maturity should be current outstanding
// 			let (_acc_rate, debt) = loan_data.accrue(now).unwrap();
// 			assert_eq!(
// 				updated_nav, debt,
// 				"should be equal to outstanding debt post maturity"
// 			);
// 		})
// }
//
// #[test]
// fn repay_loan() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let pool_account = pallet_pool::Pallet::<MockRuntime>::account_id();
// 			let owner: u64 = 10;
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 1000 * USD);
//
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, Zero::zero());
//
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
// 			let asset_registry = create_nft_class::<MockRuntime>(owner);
// 			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
// 			let res = Loan::issue_loan(
// 				Origin::signed(owner),
// 				pool_id,
// 				Asset(asset_registry, token_id),
// 			);
// 			assert_ok!(res);
//
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
//
// 			// activate loan
// 			// interest rate is 5%
// 			let rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			let oracle: u64 = 1;
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity in 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			let res = Loan::activate_loan(
// 				Origin::signed(oracle),
// 				pool_id,
// 				loan_id,
// 				rate,
// 				// ceiling is 100 USD
// 				loan_type,
// 			);
// 			assert_ok!(res);
//
// 			// try repay without any borrowed
// 			let repay_amount = Amount::from_inner(20 * USD);
// 			let res = Loan::repay(Origin::signed(owner), pool_id, loan_id, repay_amount);
// 			assert_ok!(res);
//
// 			// borrow 50
// 			Timestamp::set_timestamp(1);
// 			let borrow_amount = Amount::from_inner(50 * USD);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_ok!(res);
//
// 			// check loan data
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			// accumulated rate is now rate per sec
// 			assert_eq!(loan_data.accumulated_rate, rate);
// 			assert_eq!(loan_data.last_updated, 1);
// 			assert_eq!(loan_data.borrowed_amount, Amount::from_inner(50 * USD));
// 			let p_debt = borrow_amount
// 				.checked_div(&math::convert::<Rate, Amount>(loan_data.accumulated_rate).unwrap())
// 				.unwrap();
// 			assert_eq!(loan_data.principal_debt, p_debt);
// 			// pool should have 50 less token
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 950 * USD);
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, 50 * USD);
// 			// nav should be updated to latest present value
// 			let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
// 			let pv = loan_data.present_value().unwrap();
// 			assert_eq!(current_nav, pv, "should be same due to single loan");
//
// 			// repay 20 after 1000 seconds
// 			Timestamp::set_timestamp(1001);
// 			let repay_amount = Amount::from_inner(20 * USD);
// 			let res = Loan::repay(Origin::signed(owner), pool_id, loan_id, repay_amount);
// 			assert_ok!(res);
//
// 			// check loan data
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			// accumulated rate is now rate per sec
// 			assert_eq!(
// 				loan_data.accumulated_rate,
// 				checked_pow(rate, 1000).unwrap().checked_mul(&rate).unwrap()
// 			);
// 			assert_eq!(loan_data.last_updated, 1001);
// 			assert_eq!(loan_data.borrowed_amount, Amount::from_inner(50 * USD));
// 			// principal debt should still be more than 30 due to interest
// 			assert!(loan_data.principal_debt > Amount::from_inner(30 * USD));
// 			// pool should have 30 less token
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 970 * USD);
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, 30 * USD);
// 			// nav should be updated to latest present value
// 			let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
// 			let pv = loan_data.present_value().unwrap();
// 			assert_eq!(current_nav, pv, "should be same due to single loan");
//
// 			// repay 30 more after another 1000 seconds
// 			Timestamp::set_timestamp(2001);
// 			let repay_amount = Amount::from_inner(30 * USD);
// 			let res = Loan::repay(Origin::signed(owner), pool_id, loan_id, repay_amount);
// 			assert_ok!(res);
//
// 			// try and close the loan
// 			let res = Loan::close_loan(Origin::signed(owner), pool_id, loan_id);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanNotRepaid);
//
// 			// check loan data
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			// nav should be updated to latest present value
// 			let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
// 			let pv = loan_data.present_value().unwrap();
// 			assert_eq!(current_nav, pv, "should be same due to single loan");
//
// 			// repay the interest
// 			// 50 for 1000 seconds
// 			let amount = Amount::from_inner(50 * USD);
// 			let p_debt = amount
// 				.checked_div(&math::convert::<Rate, Amount>(loan_data.rate_per_sec).unwrap())
// 				.unwrap();
// 			let rate_after_1000 = checked_pow(loan_data.rate_per_sec, 1001).unwrap();
// 			let debt_after_1000 = p_debt
// 				.checked_mul(&math::convert::<Rate, Amount>(rate_after_1000).unwrap())
// 				.unwrap();
//
// 			// 30 for 1000 seconds
// 			let p_debt = debt_after_1000
// 				.checked_sub(&Amount::from_inner(20 * USD))
// 				.unwrap()
// 				.checked_div(&math::convert::<Rate, Amount>(rate_after_1000).unwrap())
// 				.unwrap();
// 			let rate_after_2000 = checked_pow(loan_data.rate_per_sec, 2001).unwrap();
// 			let debt_after_2000 = p_debt
// 				.checked_mul(&math::convert::<Rate, Amount>(rate_after_2000).unwrap())
// 				.unwrap();
// 			let p_debt = debt_after_2000
// 				.checked_sub(&Amount::from_inner(30 * USD))
// 				.unwrap()
// 				.checked_div(&math::convert::<Rate, Amount>(rate_after_2000).unwrap())
// 				.unwrap();
// 			assert_eq!(loan_data.principal_debt, p_debt);
//
// 			// debt after 3000 seconds
// 			Timestamp::set_timestamp(3001);
// 			let rate_after_3000 = checked_pow(loan_data.rate_per_sec, 3001).unwrap();
// 			let debt = p_debt
// 				.checked_mul(&math::convert::<Rate, Amount>(rate_after_3000).unwrap())
// 				.unwrap();
//
// 			// transfer exact interest amount to owner account from dummy account 2
// 			let dummy: u64 = 2;
// 			let transfer_amount: Balance = debt.into_inner().into();
// 			let dest =
// 				<<MockRuntime as frame_system::Config>::Lookup as StaticLookup>::unlookup(owner);
// 			let res = Tokens::transfer(
// 				Origin::signed(dummy),
// 				dest,
// 				GetUSDCurrencyId::get(),
// 				transfer_amount,
// 			);
// 			assert_ok!(res);
//
// 			// repay the interest
// 			let repay_amount = debt;
// 			let res = Loan::repay(Origin::signed(owner), pool_id, loan_id, repay_amount);
// 			assert_ok!(res);
//
// 			// close loan
// 			let res = Loan::close_loan(Origin::signed(owner), pool_id, loan_id);
// 			assert_ok!(res);
//
// 			// check loan data
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			assert_eq!(loan_data.status, LoanStatus::Closed);
// 			assert_eq!(loan_data.principal_debt, Zero::zero());
// 			assert_eq!(loan_data.borrowed_amount, Amount::from_inner(50 * USD));
// 			assert_eq!(loan_data.last_updated, 3001);
// 			// nav should be updated to latest present value and should be zero
// 			let current_nav = <Loan as TPoolNav<PoolId, Amount>>::nav(pool_id).unwrap().0;
// 			let pv = loan_data.present_value().unwrap();
// 			assert_eq!(current_nav, pv, "should be same due to single loan");
// 			assert_eq!(current_nav, Zero::zero());
//
// 			// pool balance should be 1000 + interest
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			let expected_balance = 1000 * USD + transfer_amount;
// 			assert_eq!(pool_balance, expected_balance);
//
// 			// owner balance should be zero
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, Zero::zero());
//
// 			// owner account should own the asset NFT
// 			expect_asset_owner::<MockRuntime>(Asset(asset_registry, token_id), owner);
//
// 			// Loan account should own the loan NFT
// 			let loan_nft_registry = PoolToLoanNftClass::<MockRuntime>::get(pool_id)
// 				.expect("must have a loan nft registry created");
// 			expect_asset_owner::<MockRuntime>(
// 				Asset(loan_nft_registry, loan_id),
// 				Loan::account_id(),
// 			);
//
// 			// check nav
// 			let res = Loan::update_nav_of_pool(pool_id);
// 			assert_ok!(res);
// 			let nav = res.unwrap();
// 			assert_eq!(nav, Zero::zero())
// 		})
// }
//
// #[test]
// fn test_pool_nav() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let pool_account = pallet_pool::Pallet::<MockRuntime>::account_id();
// 			let owner: u64 = 10;
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 1000 * USD);
//
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, Zero::zero());
//
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
// 			let asset_registry = create_nft_class::<MockRuntime>(owner);
// 			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
// 			let res = Loan::issue_loan(
// 				Origin::signed(owner),
// 				pool_id,
// 				Asset(asset_registry, token_id),
// 			);
// 			assert_ok!(res);
//
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
//
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			let pv: Amount = loan_data
// 				.present_value()
// 				.expect("present value should not return none");
// 			assert_eq!(pv, Zero::zero());
//
// 			let res = Loan::update_nav_of_pool(pool_id);
// 			assert_ok!(res);
// 			let nav = res.unwrap();
// 			assert_eq!(nav, Zero::zero());
//
// 			// activate loan
// 			// interest rate is 5%
// 			let rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			let oracle: u64 = 1;
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity in 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			let res = Loan::activate_loan(
// 				Origin::signed(oracle),
// 				pool_id,
// 				loan_id,
// 				rate,
// 				// ceiling is 100 USD
// 				loan_type,
// 			);
// 			assert_ok!(res);
//
// 			// present value should still be zero
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			let pv = loan_data
// 				.present_value()
// 				.expect("present value should not return none");
// 			assert_eq!(pv, Zero::zero());
//
// 			// borrow 50 amount at the instant
// 			let borrow_amount = Amount::from_inner(50 * USD);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_ok!(res);
//
// 			// present value should still be around 50.93
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			let pv = loan_data
// 				.present_value()
// 				.expect("present value should not return none");
// 			assert_eq!(
// 				pv,
// 				Amount::saturating_from_rational(50933551899382200731u128, Amount::accuracy())
// 			);
//
// 			// pass some time. maybe 200 days
// 			let after_200_days = 3600 * 24 * 200;
// 			Timestamp::set_timestamp(after_200_days);
// 			let res = Loan::update_nav_of_pool(pool_id);
// 			assert_ok!(res);
// 			let nav = res.unwrap();
// 			// present value should be 52.06
// 			assert_eq!(
// 				nav,
// 				Amount::saturating_from_rational(52062227586365608471u128, Amount::accuracy())
// 			);
//
// 			// let the maturity has passed 2 years + 10 day
// 			let after_2_years = (math::seconds_per_year() * 2) + math::seconds_per_day() * 10;
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			let (_acc_rate, debt) = loan_data.accrue(after_2_years).unwrap();
// 			Timestamp::set_timestamp(after_2_years);
// 			let res = Loan::update_nav_of_pool(pool_id);
// 			assert_ok!(res);
// 			let pv = res.unwrap();
// 			// present value should be equal to current outstanding debt
// 			assert_eq!(pv, debt);
// 			let nav = res.unwrap();
// 			assert_eq!(pv, nav);
//
// 			// call update nav extrinsic and check for event
// 			let res = Loan::update_nav(Origin::signed(owner), pool_id);
// 			assert_ok!(res);
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (got_pool_id, updated_nav) = match loan_event {
// 				LoanEvent::NAVUpdated(pool_id, update_nav) => Some((pool_id, update_nav)),
// 				_ => None,
// 			}
// 			.expect("must be a Nav updated event");
// 			assert_eq!(pool_id, got_pool_id);
// 			assert_eq!(updated_nav, nav);
//
// 			// write off the loan and check for updated nav
// 			for group in vec![(3, 10), (5, 15), (7, 20), (20, 30)] {
// 				let group = WriteOffGroup {
// 					percentage: Rate::saturating_from_rational(group.1, 100),
// 					overdue_days: group.0,
// 				};
// 				let res = Loan::add_write_off_group(Origin::signed(oracle), pool_id, group);
// 				assert_ok!(res);
// 			}
//
// 			// write off loan. someone calls write off
// 			let res = Loan::write_off_loan(Origin::signed(100), pool_id, loan_id);
// 			assert_ok!(res);
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (_pool_id, _loan_id, write_off_index) = match loan_event {
// 				LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
// 					Some((pool_id, loan_id, write_off_index))
// 				}
// 				_ => None,
// 			}
// 			.expect("must be a loan written off event");
// 			// it must be 2 with overdue days as 7 and write off percentage as 20%
// 			assert_eq!(write_off_index, 2);
//
// 			// update nav
// 			let res = Loan::update_nav(Origin::signed(owner), pool_id);
// 			assert_ok!(res);
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (_pool_id, updated_nav) = match loan_event {
// 				LoanEvent::NAVUpdated(pool_id, update_nav) => Some((pool_id, update_nav)),
// 				_ => None,
// 			}
// 			.expect("must be a Nav updated event");
//
// 			// updated nav should be (1-20%) outstanding debt
// 			let expected_nav =
// 				math::convert::<Rate, Amount>(Rate::saturating_from_rational(20, 100))
// 					.and_then(|rate| debt.checked_mul(&rate))
// 					.and_then(|written_off_amount| debt.checked_sub(&written_off_amount))
// 					.unwrap();
// 			assert_eq!(expected_nav, updated_nav);
// 		})
// }
//
// #[test]
// fn add_write_off_groups() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let oracle: u64 = 1;
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(oracle);
//
// 			// fetch write off groups
// 			let groups = PoolWriteOffGroups::<MockRuntime>::get(pool_id);
// 			assert_eq!(groups, vec![]);
//
// 			for percentage in vec![10, 20, 30, 40, 30, 50, 70, 100] {
// 				// add a new write off group
// 				let group = WriteOffGroup {
// 					percentage: Rate::saturating_from_rational(percentage, 100),
// 					overdue_days: 3,
// 				};
// 				let res = Loan::add_write_off_group(Origin::signed(oracle), pool_id, group);
// 				assert_ok!(res);
// 				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 				let (_pool_id, index) = match loan_event {
// 					LoanEvent::WriteOffGroupAdded(pool_id, index) => Some((pool_id, index)),
// 					_ => None,
// 				}
// 				.expect("must be a write off group added event");
//
// 				// check if the write off group is added
// 				let groups = PoolWriteOffGroups::<MockRuntime>::get(pool_id);
// 				assert_eq!(groups[index as usize], group);
// 				assert_eq!(groups.len() - 1, index as usize);
// 			}
//
// 			// invalid write off group
// 			let group = WriteOffGroup {
// 				percentage: Rate::saturating_from_rational(110, 100),
// 				overdue_days: 3,
// 			};
// 			let res = Loan::add_write_off_group(Origin::signed(oracle), pool_id, group);
// 			assert_err!(res, Error::<MockRuntime>::ErrInvalidWriteOffGroup);
// 		})
// }
//
// #[test]
// fn write_off_loan() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let pool_account = pallet_pool::Pallet::<MockRuntime>::account_id();
// 			let owner: u64 = 10;
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 1000 * USD);
//
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, Zero::zero());
//
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
// 			let asset_registry = create_nft_class::<MockRuntime>(owner);
// 			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
// 			let res = Loan::issue_loan(
// 				Origin::signed(owner),
// 				pool_id,
// 				Asset(asset_registry, token_id),
// 			);
// 			assert_ok!(res);
//
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
//
// 			// activate loan
// 			// interest rate is 5%
// 			let rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			let oracle: u64 = 1;
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity in 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			let res = Loan::activate_loan(
// 				Origin::signed(oracle),
// 				pool_id,
// 				loan_id,
// 				rate,
// 				// ceiling is 100 USD
// 				loan_type,
// 			);
// 			assert_ok!(res);
//
// 			// borrow 50
// 			Timestamp::set_timestamp(1);
// 			let borrow_amount = Amount::from_inner(50 * USD);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_ok!(res);
//
// 			// after one year
// 			// anyone can trigger the call
// 			let caller = 100;
// 			Timestamp::set_timestamp(math::seconds_per_year());
// 			let res = Loan::write_off_loan(Origin::signed(caller), pool_id, loan_id);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanHealthy);
//
// 			// let the maturity date passes + 1 day
// 			let t = math::seconds_per_year() * 2 + math::seconds_per_day();
// 			Timestamp::set_timestamp(t);
// 			let res = Loan::write_off_loan(Origin::signed(caller), pool_id, loan_id);
// 			assert_err!(res, Error::<MockRuntime>::ErrNoValidWriteOffGroup);
//
// 			// add write off groups
// 			for group in vec![(3, 10), (5, 15), (7, 20), (20, 30)] {
// 				let res = Loan::add_write_off_group(
// 					Origin::signed(oracle),
// 					pool_id,
// 					WriteOffGroup {
// 						percentage: Rate::saturating_from_rational(group.1, 100),
// 						overdue_days: group.0,
// 					},
// 				);
// 				assert_ok!(res);
// 			}
//
// 			// same since write off group is missing
// 			let t = math::seconds_per_year() * 2 + math::seconds_per_day();
// 			Timestamp::set_timestamp(t);
// 			let res = Loan::write_off_loan(Origin::signed(caller), pool_id, loan_id);
// 			assert_err!(res, Error::<MockRuntime>::ErrNoValidWriteOffGroup);
//
// 			// days, index
// 			for days_index in vec![(3, 0), (5, 1), (7, 2), (20, 3)] {
// 				// move to more than 3 days
// 				let t = math::seconds_per_year() * 2 + math::seconds_per_day() * days_index.0;
// 				Timestamp::set_timestamp(t);
// 				let res = Loan::write_off_loan(Origin::signed(caller), pool_id, loan_id);
// 				assert_ok!(res);
//
// 				let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 				let (_pool_id, _loan_id, write_off_index) = match loan_event {
// 					LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
// 						Some((pool_id, loan_id, write_off_index))
// 					}
// 					_ => None,
// 				}
// 				.expect("must be a Loan issue event");
// 				assert_eq!(write_off_index, days_index.1);
// 				let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
// 					.expect("LoanData should be present");
// 				assert_eq!(loan_data.write_off_index, Some(days_index.1));
// 				assert!(!loan_data.admin_written_off);
// 			}
// 		})
// }
//
// #[test]
// fn admin_write_off_loan() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let pool_account = pallet_pool::Pallet::<MockRuntime>::account_id();
// 			let owner: u64 = 10;
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 1000 * USD);
//
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, Zero::zero());
//
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
// 			let asset_registry = create_nft_class::<MockRuntime>(owner);
// 			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
// 			let res = Loan::issue_loan(
// 				Origin::signed(owner),
// 				pool_id,
// 				Asset(asset_registry, token_id),
// 			);
// 			assert_ok!(res);
//
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
//
// 			// activate loan
// 			// interest rate is 5%
// 			let rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			let oracle: u64 = 1;
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity in 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			let res = Loan::activate_loan(
// 				Origin::signed(oracle),
// 				pool_id,
// 				loan_id,
// 				rate,
// 				// ceiling is 100 USD
// 				loan_type,
// 			);
// 			assert_ok!(res);
//
// 			// borrow 50
// 			Timestamp::set_timestamp(1);
// 			let borrow_amount = Amount::from_inner(50 * USD);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_ok!(res);
//
// 			// after one year
// 			// caller should be admin, can write off before maturity
// 			let caller = 1;
// 			Timestamp::set_timestamp(math::seconds_per_year());
// 			let res = Loan::admin_write_off_loan(Origin::signed(caller), pool_id, loan_id, 0);
// 			assert_err!(res, Error::<MockRuntime>::ErrInvalidWriteOffGroupIndex);
//
// 			// let the maturity date passes + 1 day
// 			let t = math::seconds_per_year() * 2 + math::seconds_per_day();
// 			Timestamp::set_timestamp(t);
// 			let res = Loan::admin_write_off_loan(Origin::signed(caller), pool_id, loan_id, 0);
// 			assert_err!(res, Error::<MockRuntime>::ErrInvalidWriteOffGroupIndex);
//
// 			// add write off groups
// 			for group in vec![(3, 10), (5, 15), (7, 20), (20, 30)] {
// 				let res = Loan::add_write_off_group(
// 					Origin::signed(oracle),
// 					pool_id,
// 					WriteOffGroup {
// 						percentage: Rate::saturating_from_rational(group.1, 100),
// 						overdue_days: group.0,
// 					},
// 				);
// 				assert_ok!(res);
// 			}
//
// 			// index
// 			for time in vec![
// 				math::seconds_per_year(),
// 				math::seconds_per_year() * 2 + math::seconds_per_day() * 3,
// 			] {
// 				Timestamp::set_timestamp(time);
// 				for index in vec![0, 3, 2, 1, 0] {
// 					let res =
// 						Loan::admin_write_off_loan(Origin::signed(caller), pool_id, loan_id, index);
// 					assert_ok!(res);
//
// 					let loan_event =
// 						fetch_loan_event(last_event()).expect("should be a loan event");
// 					let (_pool_id, _loan_id, write_off_index) = match loan_event {
// 						LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
// 							Some((pool_id, loan_id, write_off_index))
// 						}
// 						_ => None,
// 					}
// 					.expect("must be a Loan issue event");
// 					assert_eq!(write_off_index, index);
// 					let loan_data = LoanInfo::<MockRuntime>::get(pool_id, loan_id)
// 						.expect("LoanData should be present");
// 					assert_eq!(loan_data.write_off_index, Some(index));
// 					assert!(loan_data.admin_written_off);
// 				}
// 			}
//
// 			// permission less write off should not work once written off by admin
// 			let res = Loan::write_off_loan(Origin::signed(100), pool_id, loan_id);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanWrittenOffByAdmin)
// 		})
// }
//
// #[test]
// fn close_written_off_loan() {
// 	TestExternalitiesBuilder::default()
// 		.build()
// 		.execute_with(|| {
// 			let pool_account = pallet_pool::Pallet::<MockRuntime>::account_id();
// 			let owner: u64 = 10;
// 			let pool_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&pool_account);
// 			assert_eq!(pool_balance, 1000 * USD);
//
// 			let owner_balance = balance_of::<MockRuntime, GetUSDCurrencyId>(&owner);
// 			assert_eq!(owner_balance, Zero::zero());
//
// 			let pool_id = create_pool::<MockRuntime, GetUSDCurrencyId>(owner);
// 			let asset_registry = create_nft_class::<MockRuntime>(owner);
// 			let token_id = mint_nft::<MockRuntime>(owner, asset_registry);
// 			let res = Loan::issue_loan(
// 				Origin::signed(owner),
// 				pool_id,
// 				Asset(asset_registry, token_id),
// 			);
// 			assert_ok!(res);
//
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (pool_id, loan_id) = match loan_event {
// 				LoanEvent::LoanIssued(pool_id, loan_id) => Some((pool_id, loan_id)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
//
// 			// activate loan
// 			// interest rate is 5%
// 			let rate = math::rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
// 			let oracle: u64 = 1;
// 			let loan_type = LoanType::BulletLoan(BulletLoan::new(
// 				// advance rate 80%
// 				Rate::saturating_from_rational(80, 100),
// 				// expected loss over asset maturity 0.15%
// 				Rate::saturating_from_rational(15, 10000),
// 				// collateral value
// 				Amount::from_inner(125 * USD),
// 				// 4%
// 				math::rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap(),
// 				// maturity in 2 years
// 				math::seconds_per_year() * 2,
// 			));
// 			let res = Loan::activate_loan(
// 				Origin::signed(oracle),
// 				pool_id,
// 				loan_id,
// 				rate,
// 				// ceiling is 100 USD
// 				loan_type,
// 			);
// 			assert_ok!(res);
//
// 			// borrow 50
// 			Timestamp::set_timestamp(1);
// 			let borrow_amount = Amount::from_inner(50 * USD);
// 			let res = Loan::borrow(Origin::signed(owner), pool_id, loan_id, borrow_amount);
// 			assert_ok!(res);
//
// 			// let the maturity pass and closing loan should not work
// 			Timestamp::set_timestamp(math::seconds_per_year() * 2 + 5 * math::seconds_per_day());
// 			let res = Loan::close_loan(Origin::signed(owner), pool_id, loan_id);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanNotRepaid);
//
// 			// add write off groups
// 			for group in vec![(3, 10), (5, 15), (7, 20), (20, 30), (120, 100)] {
// 				let res = Loan::add_write_off_group(
// 					Origin::signed(oracle),
// 					pool_id,
// 					WriteOffGroup {
// 						percentage: Rate::saturating_from_rational(group.1, 100),
// 						overdue_days: group.0,
// 					},
// 				);
// 				assert_ok!(res);
// 			}
//
// 			// write off loan but should not be able to close since its not 100% write off
// 			let res = Loan::write_off_loan(Origin::signed(200), pool_id, loan_id);
// 			assert_ok!(res);
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (_pool_id, _loan_id, write_off_index) = match loan_event {
// 				LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
// 					Some((pool_id, loan_id, write_off_index))
// 				}
// 				_ => None,
// 			}
// 			.expect("must be a Loan issue event");
// 			assert_eq!(write_off_index, 1);
// 			let res = Loan::close_loan(Origin::signed(owner), pool_id, loan_id);
// 			assert_err!(res, Error::<MockRuntime>::ErrLoanNotRepaid);
//
// 			// let it be 120 days beyond maturity, we write off 100% now
// 			Timestamp::set_timestamp(math::seconds_per_year() * 2 + 120 * math::seconds_per_day());
// 			let res = Loan::write_off_loan(Origin::signed(200), pool_id, loan_id);
// 			assert_ok!(res);
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (_pool_id, _loan_id, write_off_index) = match loan_event {
// 				LoanEvent::LoanWrittenOff(pool_id, loan_id, write_off_index) => {
// 					Some((pool_id, loan_id, write_off_index))
// 				}
// 				_ => None,
// 			}
// 			.expect("must be a Loan written off event");
// 			assert_eq!(write_off_index, 4);
//
// 			// nav should be zero
// 			let res = Loan::update_nav(Origin::signed(owner), pool_id);
// 			assert_ok!(res);
// 			let loan_event = fetch_loan_event(last_event()).expect("should be a loan event");
// 			let (got_pool_id, updated_nav) = match loan_event {
// 				LoanEvent::NAVUpdated(pool_id, update_nav) => Some((pool_id, update_nav)),
// 				_ => None,
// 			}
// 			.expect("must be a Nav updated event");
// 			assert_eq!(pool_id, got_pool_id);
// 			assert_eq!(updated_nav, Zero::zero());
//
// 			// close loan now
// 			let res = Loan::close_loan(Origin::signed(owner), pool_id, loan_id);
// 			assert_ok!(res);
//
// 			let (pool_id, loan_id, asset) = match fetch_loan_event(last_event())
// 				.expect("should be a loan event")
// 			{
// 				LoanEvent::LoanClosed(pool_id, loan_id, asset) => Some((pool_id, loan_id, asset)),
// 				_ => None,
// 			}
// 			.expect("must be a Loan close event");
// 			assert_eq!(asset, Asset(asset_registry, token_id));
//
// 			// check asset owner
// 			expect_asset_owner::<MockRuntime>(Asset(asset_registry, token_id), owner);
//
// 			// check nft loan owner loan pallet
// 			let loan_nft_registry = PoolToLoanNftClass::<MockRuntime>::get(pool_id)
// 				.expect("must have a loan nft registry created");
// 			expect_asset_owner::<MockRuntime>(
// 				Asset(loan_nft_registry, loan_id),
// 				Loan::account_id(),
// 			);
//
// 			// check loan status as Closed
// 			let loan_data =
// 				LoanInfo::<MockRuntime>::get(pool_id, loan_id).expect("LoanData should be present");
// 			assert_eq!(loan_data.status, LoanStatus::Closed);
// 		})
// }
