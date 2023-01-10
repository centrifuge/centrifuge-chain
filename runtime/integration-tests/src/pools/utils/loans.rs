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
use std::collections::HashMap;

use cfg_primitives::{AccountId, Address, Balance, CollectionId, ItemId, PoolId};
use cfg_types::fixed_point::Rate;
use pallet_loans::{
	loan_type::{BulletLoan, LoanType},
	math::interest_rate_per_sec,
	types::Asset,
	Call as LoansCall,
};
use pallet_uniques::Call as UniquesCall;

use crate::{chain::centrifuge::RuntimeCall, pools::utils::tokens::rate_from_percent};

/// Structure that manages collateral and loan nft ids
pub struct NftManager {
	collaterals: HashMap<PoolId, ItemId>,
	loans: HashMap<PoolId, ItemId>,
}

/// The id we use for loans
pub type LoanId = ItemId;

// The id we use for collaterals
pub type CollateralId = ItemId;

impl NftManager {
	pub fn new() -> Self {
		Self {
			collaterals: HashMap::new(),
			loans: HashMap::new(),
		}
	}

	/// Currently simply maps pool_id = loan_class_id for a pool
	pub fn loan_class_id(&self, pool_id: PoolId) -> CollectionId {
		pool_id
	}

	/// Maps (pool_id + 1) << 32 = collateral_class id
	///
	/// panics if pool_id >= u32::MAX - 1 as this would result in an overflow
	/// during shifting.
	pub fn collateral_class_id(&self, pool_id: PoolId) -> CollectionId {
		assert!(
			pool_id < u32::MAX.into(),
			"Pool-id must be smaller u32::MAX for testing. To ensure no-clashes in NFT class-ids"
		);
		let id = (pool_id + 1) << 32;
		id
	}

	pub fn curr_loan_id(&mut self, pool_id: PoolId) -> ItemId {
		self.loans.entry(pool_id).or_insert(ItemId(1)).clone()
	}

	fn next_loan_id(&mut self, pool_id: PoolId) -> ItemId {
		let id = self.loans.entry(pool_id).or_insert(ItemId(1));
		let next = id.clone();
		*id = ItemId(id.0);
		next
	}

	pub fn curr_collateral_id(&mut self, pool_id: PoolId) -> ItemId {
		self.loans.entry(pool_id).or_insert(ItemId(1)).clone()
	}

	fn next_collateral_id(&mut self, pool_id: PoolId) -> ItemId {
		let id = self.collaterals.entry(pool_id).or_insert(ItemId(1));
		let next = id.clone();
		*id = ItemId(id.0);
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
) -> Vec<RuntimeCall> {
	let loan_class = manager.loan_class_id(pool_id);
	let collateral_class = manager.collateral_class_id(pool_id);
	let mut calls = Vec::new();
	calls.push(create_nft_call(owner.clone(), collateral_class));
	calls.push(create_nft_call(owner, loan_class));
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
/// 	* discount_rate: 4% ,
pub fn issue_default_loan(
	owner: AccountId,
	pool_id: PoolId,
	amount: Balance,
	maturity: u64,
	manager: &mut NftManager,
) -> Vec<RuntimeCall> {
	let loan_type = LoanType::BulletLoan(BulletLoan::new(
		rate_from_percent(90),
		rate_from_percent(5),
		rate_from_percent(50),
		amount,
		interest_rate_per_sec(rate_from_percent(4))
			.expect("Essential: Creating rate per sec must not fail."),
		maturity,
	));

	issue_loan(owner, pool_id, rate_from_percent(15), loan_type, manager)
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
	interest_rate_per_year: Rate,
	loan_type: LoanType<Rate, Balance>,
	manager: &mut NftManager,
) -> Vec<RuntimeCall> {
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
		interest_rate_per_year,
		loan_type,
	));
	calls
}

pub fn initialise_pool_call(pool_id: PoolId, loan_nft_class_id: CollectionId) -> RuntimeCall {
	RuntimeCall::Loans(LoansCall::initialise_pool {
		pool_id,
		loan_nft_class_id,
	})
}

pub fn create_loan_call(pool_id: PoolId, collateral: Asset<CollectionId, ItemId>) -> RuntimeCall {
	RuntimeCall::Loans(LoansCall::create {
		pool_id,
		collateral,
	})
}

pub fn price_loan_call(
	pool_id: PoolId,
	loan_id: LoanId,
	interest_rate_per_year: Rate,
	loan_type: LoanType<Rate, Balance>,
) -> RuntimeCall {
	RuntimeCall::Loans(LoansCall::price {
		pool_id,
		loan_id,
		interest_rate_per_year,
		loan_type,
	})
}

pub fn borrow_call(pool_id: PoolId, loan_id: LoanId, amount: Balance) -> RuntimeCall {
	RuntimeCall::Loans(LoansCall::borrow {
		pool_id,
		loan_id,
		amount,
	})
}

pub fn repay_call(pool_id: PoolId, loan_id: LoanId, amount: Balance) -> RuntimeCall {
	RuntimeCall::Loans(LoansCall::repay {
		pool_id,
		loan_id,
		amount,
	})
}

pub fn close_loan_call(pool_id: PoolId, loan_id: LoanId) -> RuntimeCall {
	RuntimeCall::Loans(LoansCall::close { pool_id, loan_id })
}

pub fn create_nft_call(admin: AccountId, collection: CollectionId) -> RuntimeCall {
	RuntimeCall::Uniques(UniquesCall::create {
		admin: Address::Id(admin),
		collection,
	})
}

pub fn mint_nft_call(collection: CollectionId, item: ItemId, owner: AccountId) -> RuntimeCall {
	RuntimeCall::Uniques(UniquesCall::mint {
		collection,
		item,
		owner: Address::Id(owner),
	})
}
