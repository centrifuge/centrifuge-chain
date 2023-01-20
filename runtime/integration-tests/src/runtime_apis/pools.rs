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

use cfg_primitives::{AccountId, Moment};
use cfg_types::{
	permissions::{PermissionScope, PermissionScope::Currency, PoolRole, Role},
	tokens::CurrencyId,
};
use cfg_utils::set_block_number_timestamp;
use codec::Encode;
use development_runtime::apis::PoolsApi;
use frame_support::{assert_ok, dispatch::UnfilteredDispatchable, traits::UnixTime};
use frame_system::Origin;
use fudge::primitives::Chain;
use pallet_loans::math;
use sp_core::Pair;
use sp_runtime::{app_crypto::sr25519, traits::IdentifyAccount};
use tokio::runtime::Handle;

use super::{ApiEnv, PARA_ID};
use crate::{
	chain,
	chain::centrifuge::{Runtime, RuntimeOrigin},
	pools::utils::{
		accounts::Keyring,
		env::{test_env_default, TestEnv},
		loans::{borrow_call, init_loans_for_pool, issue_default_loan, LoanId, NftManager},
		pools::default_pool_calls,
	},
};

#[tokio::test]
async fn test() {
	ApiEnv::new(Handle::current())
		.startup(|| {
			let pool_admin: AccountId = Keyring::Alice.into();

			let mut nft_manager = NftManager::new();
			let set_default_pool = default_pool_calls(pool_admin.clone(), 0, &mut nft_manager);

			let issue_default_loans = issue_default_loan(
				pool_admin.clone(),
				0,
				10_000 * 10_000_000_000,
				90 * 60 * 60 * 24,
				&mut nft_manager,
			);

			for call in set_default_pool {
				let res = UnfilteredDispatchable::dispatch_bypass_filter(
					call,
					RuntimeOrigin::signed(pool_admin.clone()),
				);
				assert_ok!(res);
			}
			for call in issue_default_loans {
				let res = UnfilteredDispatchable::dispatch_bypass_filter(
					call,
					RuntimeOrigin::signed(pool_admin.clone()),
				);
				assert_ok!(res);
			}

			let borrow_call = UnfilteredDispatchable::dispatch_bypass_filter(
				borrow_call(0, LoanId::from(1_u16), 100_000),
				RuntimeOrigin::signed(pool_admin.clone()),
			);

			assert_ok!(borrow_call);

			// set timestamp to around 1 year
			let now = development_runtime::Timestamp::now();
			let after_one_year = now + 31_536_000 * 1000;
			set_block_number_timestamp::<Runtime>(Default::default(), after_one_year.into());
		})
		.with_api(|api, latest| {
			let valuation = api.portfolio_valuation(&latest, 0).unwrap();
			assert_eq!(valuation, Some(0));

			let max_borrow_amount = api
				.max_borrow_amount(&latest, 0, LoanId::from(1_u16))
				.unwrap();
			assert_eq!(max_borrow_amount, Ok(0));
		});
}
