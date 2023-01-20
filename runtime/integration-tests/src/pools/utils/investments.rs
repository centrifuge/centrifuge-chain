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

//! Utilities around the invemstemnts pallet

use cfg_primitives::{AccountId, PoolId};
use cfg_traits::TrancheCurrency as _;
use cfg_types::tokens::TrancheCurrency;
use pallet_pool_system::tranches::TrancheIndex;

use crate::{
	chain,
	chain::{
		centrifuge,
		centrifuge::{RuntimeCall, RuntimeOrigin},
	},
	pools::utils::{accounts::Keyring, loans::update_nav_call, pools::close_epoch_call},
};

/// Funds a Pool with a given PoolId. The pool must already exist and initalised.
/// We fund the JuniorTranche of the Pool for TrancheInvestors 1 to 11.
///
/// Extrinsics that are generated:
/// * Loans::update_nav
/// * PoolSystem::close_epoch
pub fn invest(
	account: AccountId,
	pool_id: PoolId,
	tranche_index: usize,
	amount: u128,
) -> Vec<RuntimeCall> {
	let mut calls = Vec::new();

	let tranche_id = {
		let pool = pallet_pool_system::Pool::<centrifuge::Runtime>::get(pool_id).unwrap();
		pool.tranches
			.ids_residual_top()
			.get(tranche_index)
			.unwrap()
			.clone()
	};
	for id in 1..11 {
		centrifuge::Investments::update_invest_order(
			RuntimeOrigin::signed(Keyring::TrancheInvestor(id).into()),
			TrancheCurrency::generate(pool_id, tranche_id),
			amount,
		)
		.unwrap()
	}

	calls.push(update_nav_call(pool_id));
	calls.push(close_epoch_call(pool_id));

	calls
}
