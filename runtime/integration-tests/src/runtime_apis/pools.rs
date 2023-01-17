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

use cfg_types::{permissions::PermissionScope::Currency, tokens::CurrencyId};
use codec::Encode;
use development_runtime::apis::PoolsApi;
use frame_support::{assert_ok, dispatch::UnfilteredDispatchable};
use frame_system::Origin;
use fudge::primitives::Chain;
use sp_core::Pair;
use sp_runtime::{app_crypto::sr25519, traits::IdentifyAccount};
use tokio::runtime::Handle;

use super::{ApiEnv, PARA_ID};
use crate::{
	chain,
	chain::centrifuge::RuntimeOrigin,
	pools::utils::{
		accounts::Keyring,
		env::{test_env_default, TestEnv},
		loans::{init_loans_for_pool, issue_default_loan, LoanId, NftManager},
		pools::default_pool_calls,
	},
};

#[tokio::test]
async fn test() {
	ApiEnv::new(Handle::current())
		.startup(|| {
			let mut nft_manager = NftManager::new();
			let set_default_pool = default_pool_calls(Keyring::Admin.into(), 0, &mut nft_manager);
			let set_loans_for_pools =
				init_loans_for_pool(Keyring::Admin.into(), 0, &mut nft_manager);
			let issue_default_loans = issue_default_loan(
				Keyring::Admin.into(),
				0,
				10_000 * 10_000_000_000,
				90 * 60 * 60 * 24,
				&mut nft_manager,
			);

			let alice = sp_runtime::AccountId32::from(
				<sr25519::Pair as sp_core::Pair>::from_string("//Alice", None)
					.unwrap()
					.public()
					.into_account(),
			);
			for call in set_default_pool {
				let res = UnfilteredDispatchable::dispatch_bypass_filter(
					call,
					RuntimeOrigin::signed(Keyring::Admin.into()),
				);
				// assert_ok!(res);
			}
			for call in set_loans_for_pools {
				let res = UnfilteredDispatchable::dispatch_bypass_filter(
					call,
					RuntimeOrigin::signed(Keyring::Admin.into()),
				);
				// assert_ok!(res);
			}
			for call in issue_default_loans {
				let res = UnfilteredDispatchable::dispatch_bypass_filter(
					call,
					RuntimeOrigin::signed(Keyring::Admin.into()),
				);
				// assert_ok!(res);
			}
		})
		.with_api(|api, latest| {
			let currency = api.currency(&latest, 0).unwrap();
			assert_eq!(currency, Some(CurrencyId::AUSD));

			let valuation = api.portfolio_valuation(&latest, 0).unwrap();
			assert_eq!(valuation, Some(0));

			let max_borrow_amount = api
				.max_borrow_amount(&latest, 0, LoanId::from(0_u16))
				.unwrap();
			assert_eq!(max_borrow_amount, None);
		});
}
