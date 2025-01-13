// Copyright 2024 Centrifuge Foundation (centrifuge.io).
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

use cfg_primitives::{AccountId, Balance, InvestmentId, PoolId, TrancheId};
use cfg_traits::{Permissions, PreConditions};
use cfg_types::{
	permissions::{PermissionScope, PoolRole, Role, TrancheInvestorInfo},
	tokens::CurrencyId,
};
use frame_support::dispatch::DispatchResult;
use orml_traits::GetByKey;
use pallet_investments::OrderType;
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;

/// Check if an account has a pool admin role
pub struct PoolAdminCheck<P>(PhantomData<P>);

impl<P> PreConditions<(AccountId, PoolId)> for PoolAdminCheck<P>
where
	P: Permissions<AccountId, Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
{
	type Result = bool;

	fn check((account_id, pool_id): (AccountId, PoolId)) -> bool {
		P::has(
			PermissionScope::Pool(pool_id),
			account_id,
			Role::PoolRole(PoolRole::PoolAdmin),
		)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn satisfy((account_id, pool_id): (AccountId, PoolId)) {
		P::add(
			PermissionScope::Pool(pool_id),
			account_id,
			Role::PoolRole(PoolRole::PoolAdmin),
		)
		.unwrap();
	}
}

/// Checks whether the given `who` has the role
/// of a `TrancheInvestor` without having `FrozenInvestor` for the given pool
/// and tranche.
pub struct IsUnfrozenTrancheInvestor<P>(PhantomData<P>);
impl<
		P: Permissions<AccountId, Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>
			+ GetByKey<
				(PermissionScope<PoolId, CurrencyId>, AccountId, TrancheId),
				Option<TrancheInvestorInfo<TrancheId>>,
			>,
	> PreConditions<OrderType<AccountId, InvestmentId, Balance>> for IsUnfrozenTrancheInvestor<P>
{
	type Result = DispatchResult;

	fn check(order: OrderType<AccountId, InvestmentId, Balance>) -> Self::Result {
		let (who, pool_id, tranche_id) = match order {
			OrderType::Investment {
				who,
				investment_id: (pool_id, tranche_id),
				..
			}
			| OrderType::Redemption {
				who,
				investment_id: (pool_id, tranche_id),
				..
			} => (who, pool_id, tranche_id),
		};

		let is_tranche_investor =
			P::get(&(PermissionScope::Pool(pool_id), who.clone(), tranche_id)).is_some()
				&& !P::has(
					PermissionScope::Pool(pool_id),
					who,
					Role::PoolRole(PoolRole::FrozenTrancheInvestor(tranche_id)),
				);

		if is_tranche_investor || cfg!(feature = "runtime-benchmarks") {
			Ok(())
		} else {
			// TODO: We should adapt the permissions pallets interface to return an error
			// instead of a boolean. This makes the redundant "does not have role" error,
			// which downstream pallets always need to generate, not needed anymore.
			Err(DispatchError::Other(
				"Account does not have the TrancheInvestor permission.",
			))
		}
	}
}
