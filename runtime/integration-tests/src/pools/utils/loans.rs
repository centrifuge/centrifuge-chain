// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Utilities around the loans pallet
use crate::chain::centrifuge::{Address, Amount, Balance, Call, Rate};
use crate::pools::utils::time::Moment;
use crate::pools::utils::tokens::rate_from_percent;
use pallet_loans::{
	loan_type::{BulletLoan, LoanType},
	math::interest_rate_per_sec,
	types::Asset,
	Call as LoansCall,
};
use pallet_uniques::Call as UniquesCall;
use runtime_common::{AccountId, ClassId, InstanceId, PoolId};
use sp_arithmetic::traits::{checked_pow, One};
use sp_runtime::{traits::CheckedMul, FixedPointNumber};
use std::collections::HashMap;

/// Structure that manages collateral and loan nft ids
pub struct NftManager {
	collaterals: HashMap<PoolId, InstanceId>,
	loans: HashMap<PoolId, InstanceId>,
}

/// The id we use for loans
pub type LoanId = InstanceId;

// The id we use for collaterals
pub type CollateralId = InstanceId;

impl NftManager {
	pub fn new() -> Self {
		Self {
			collaterals: HashMap::new(),
			loans: HashMap::new(),
		}
	}

	/// Currently simply maps pool_id = loan_class_id for a pool
	pub fn loan_class_id(&self, pool_id: PoolId) -> ClassId {
		pool_id
	}

	/// Maps (pool_id + 1) << 32 = collateral_class id
	///
	/// panics if pool_id >= u32::MAX - 1 as this would result in an overflow
	/// during shifting.
	pub fn collateral_class_id(&self, pool_id: PoolId) -> ClassId {
		assert!(
			pool_id < u32::MAX.into(),
			"Pool-id must be smaller u32::MAX for testing. To ensure no-clashes in NFT class-ids"
		);
		let id = (pool_id + 1) << 32;
		id
	}

	pub fn curr_loan_id(&mut self, pool_id: PoolId) -> InstanceId {
		self.loans.entry(pool_id).or_insert(InstanceId(1)).clone()
	}

	fn next_loan_id(&mut self, pool_id: PoolId) -> InstanceId {
		let id = self.loans.entry(pool_id).or_insert(InstanceId(1));
		let next = id.clone();
		*id = InstanceId(id.0);
		next
	}

	pub fn curr_collateral_id(&mut self, pool_id: PoolId) -> InstanceId {
		self.loans.entry(pool_id).or_insert(InstanceId(1)).clone()
	}

	fn next_collateral_id(&mut self, pool_id: PoolId) -> InstanceId {
		let id = self.collaterals.entry(pool_id).or_insert(InstanceId(1));
		let next = id.clone();
		*id = InstanceId(id.0);
		next
	}
}

/// Creates the necessary extrinsics to initialises a pool in the loans pallet.
/// The pool must already exist for this extrinsics to succeed.
///
/// Extrinsics that are generated:
/// * Loans::initialise_pool
/// * Uniques::create -> for Loan nft class
/// * Uniques::create -> for Collateral nft class
pub fn init_loans_for_pool(
	owner: AccountId,
	pool_id: PoolId,
	manager: &mut NftManager,
) -> Vec<Call> {
	let loan_class = manager.loan_class_id(pool_id);
	let collateral_class = manager.collateral_class_id(pool_id);
	let mut calls = Vec::new();
	calls.push(create_nft_call(owner.clone(), loan_class));
	calls.push(create_nft_call(owner, collateral_class));
	calls.push(initialise_pool_call(pool_id, loan_class));
	calls
}

/// Issues a default loan with the following properties
/// * 15% APR
/// * value with amount
/// * maturity as given
/// * Type: BulletLoan
/// 	* advance_rate: 90%,
///     * probability_of_default: 5%,
///     * loss_given_default: 50%,
/// 	* discount_rate: 2% ,
pub fn issue_default_bullet_loan(
	owner: AccountId,
	pool_id: PoolId,
	amount: Balance,
	maturity: u64,
	manager: &mut NftManager,
) -> Vec<Call> {
	let loan_type = LoanType::BulletLoan(BulletLoan::new(
		rate_from_percent(90),
		rate_from_percent(5),
		rate_from_percent(50),
		Amount::from_inner(amount),
		interest_rate_per_sec(rate_from_percent(2))
			.expect("Essential: Creating rate per sec must not fail."),
		maturity,
	));

	issue_loan(
		owner,
		pool_id,
		interest_rate_per_sec(rate_from_percent(15))
			.expect("Essential: Creating rate per sec must not fail."),
		loan_type,
		manager,
	)
}

/// Issues a loan.
/// Should always be used instead of manually issuing a loan as this keeps the `NftManager`
/// in sync.
///
/// * owner should also be `PricingAdmin`
/// * owner should be owner of `CollateralClass`
///
/// Does create the following calls:
/// * mint collateral nft
/// * creates a new loan with this collateral
/// * prices the loan accordingly to input
pub fn issue_loan(
	owner: AccountId,
	pool_id: PoolId,
	intereset_rate_per_sec: Rate,
	loan_type: LoanType<Rate, Amount>,
	manager: &mut NftManager,
) -> Vec<Call> {
	let mut calls = Vec::new();
	calls.push(mint_nft_call(
		manager.collateral_class_id(pool_id),
		manager.next_collateral_id(pool_id),
		owner,
	));
	calls.push(create_loan_call(
		pool_id,
		Asset(
			manager.collateral_class_id(pool_id),
			manager.curr_collateral_id(pool_id),
		),
	));
	calls.push(price_loan_call(
		pool_id,
		manager.next_loan_id(pool_id),
		intereset_rate_per_sec,
		loan_type,
	));
	calls
}

pub fn initialise_pool_call(pool_id: PoolId, loan_nft_class_id: ClassId) -> Call {
	Call::Loans(LoansCall::initialise_pool {
		pool_id,
		loan_nft_class_id,
	})
}

pub fn create_loan_call(pool_id: PoolId, collateral: Asset<ClassId, InstanceId>) -> Call {
	Call::Loans(LoansCall::create {
		pool_id,
		collateral,
	})
}

pub fn price_loan_call(
	pool_id: PoolId,
	loan_id: LoanId,
	interest_rate_per_sec: Rate,
	loan_type: LoanType<Rate, Amount>,
) -> Call {
	Call::Loans(LoansCall::price {
		pool_id,
		loan_id,
		interest_rate_per_sec,
		loan_type,
	})
}

pub fn borrow_call(pool_id: PoolId, loan_id: LoanId, amount: Amount) -> Call {
	Call::Loans(LoansCall::borrow {
		pool_id,
		loan_id,
		amount,
	})
}

pub fn repay_call(pool_id: PoolId, loan_id: LoanId, amount: Amount) -> Call {
	Call::Loans(LoansCall::repay {
		pool_id,
		loan_id,
		amount,
	})
}

pub fn close_loan_call(pool_id: PoolId, loan_id: LoanId) -> Call {
	Call::Loans(LoansCall::close { pool_id, loan_id })
}

pub fn create_nft_call(admin: AccountId, class: ClassId) -> Call {
	Call::Uniques(UniquesCall::create {
		admin: Address::Id(admin),
		class,
	})
}

pub fn mint_nft_call(class: ClassId, instance: InstanceId, owner: AccountId) -> Call {
	Call::Uniques(UniquesCall::mint {
		class,
		instance,
		owner: Address::Id(owner),
	})
}

pub fn update_nav(pool_id: PoolId) -> Call {
	Call::Loans(LoansCall::update_nav { pool_id })
}

/// Calculates the expected repayment amount for a given
/// principal amount, rate and period.
/// Compounding happens every second.
///
/// Rate should be in percent. E.g. 15 -> 15%APR
///
/// Logic: n = second-per-year
/// * principal * (1 + r/n)^(end-start)
pub fn calculate_expected_repayment_amount(
	borrowed_amount: Balance,
	apr: u64,
	origination_date: Moment,
	maturity_date: Moment,
) -> Balance {
	let interst_rate_per_sec =
		interest_rate_per_sec(rate_from_percent(apr)).expect("Rate per sec works");
	let delta: usize = maturity_date
		.checked_sub(origination_date)
		.expect("End must be greater start of period")
		.try_into()
		.expect("Must run on 64 bit machine.");
	let rate = checked_pow(interst_rate_per_sec, delta).expect("Power must not overflow");
	rate.checked_mul_int(borrowed_amount)
		.expect("Overflow in multiplication")
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::pools::utils::time::START_DATE;
	use crate::pools::utils::tokens::DECIMAL_BASE_12;
	use runtime_common::SECONDS_PER_YEAR;

	#[test]
	fn calculate_expected_repayment_amount_works() {
		let borrowed_amount = 100_000 * DECIMAL_BASE_12;
		let amount = calculate_expected_repayment_amount(
			borrowed_amount,
			20,
			START_DATE,
			START_DATE + SECONDS_PER_YEAR,
		);
		assert_eq!(amount, 122140275738556129);
	}
}
