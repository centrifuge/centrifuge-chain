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
use std::{collections::HashMap, time::Duration};

use cfg_primitives::{
	AccountId, Address, Balance, CollectionId, ItemId, LoanId, PoolId, SECONDS_PER_YEAR,
};
use cfg_traits::ops::{EnsureAdd, EnsureDiv};
use cfg_types::fixed_point::Rate;
use pallet_loans::{
	types::{LoanInfo, MaxBorrowAmount},
	valuation::{DiscountedCashFlow, ValuationMethod},
	Call as LoansCall,
};
use pallet_uniques::Call as UniquesCall;
use sp_runtime::{traits::One, FixedPointNumber};

use crate::{chain::centrifuge::RuntimeCall, utils::tokens::rate_from_percent};

type Asset = (CollectionId, ItemId);

// TODO: Remove once #1189 is merged
fn interest_rate_per_year_to_sec(rate_per_annum: Rate) -> Rate {
	rate_per_annum
		.ensure_div(Rate::saturating_from_integer(SECONDS_PER_YEAR))
		.unwrap()
		.ensure_add(Rate::one())
		.unwrap()
}

/// Structure that manages collateral and loan nft ids
pub struct NftManager {
	collaterals: HashMap<PoolId, ItemId>,
	loans: HashMap<PoolId, ItemId>,
}

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

/// Issues a default loan with the following properties
/// * 15% APR
/// * value with amount
/// * maturity as given
/// * Type: DiscountedCashFlow with UpToTotalBorrowed
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
	let loan_info = LoanInfo::new((
		manager.collateral_class_id(pool_id),
		manager.next_collateral_id(pool_id),
	))
	.maturity(Duration::from_secs(maturity))
	.interest_rate(rate_from_percent(15))
	.collateral_value(amount)
	.max_borrow_amount(MaxBorrowAmount::UpToTotalBorrowed {
		advance_rate: rate_from_percent(90),
	})
	.valuation_method(ValuationMethod::DiscountedCashFlow(DiscountedCashFlow {
		probability_of_default: rate_from_percent(5),
		loss_given_default: rate_from_percent(50),
		discount_rate: interest_rate_per_year_to_sec(rate_from_percent(4)),
	}));

	issue_loan(owner, pool_id, loan_info, manager)
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
	loan_info: LoanInfo<Asset, Balance, Rate>,
	manager: &mut NftManager,
) -> Vec<RuntimeCall> {
	let mut calls = Vec::new();
	calls.push(mint_nft_call(
		manager.collateral_class_id(pool_id),
		manager.next_collateral_id(pool_id),
		owner,
	));
	calls.push(create_loan_call(pool_id, loan_info));
	calls
}

pub fn create_loan_call(pool_id: PoolId, info: LoanInfo<Asset, Balance, Rate>) -> RuntimeCall {
	RuntimeCall::Loans(LoansCall::create { pool_id, info })
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
