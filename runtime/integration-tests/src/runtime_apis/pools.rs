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
use frame_support::assert_ok;
use frame_support::dispatch::UnfilteredDispatchable;
use frame_system::Origin;
use development_runtime::apis::PoolsApi;
use fudge::primitives::Chain;
use sp_runtime::app_crypto::sr25519;
use tokio::runtime::Handle;

use super::{ApiEnv, PARA_ID};
use crate::{
	chain,
	pools::utils::{
		accounts::Keyring,
		env::{test_env_default, TestEnv},
		loans::{init_loans_for_pool, LoanId, NftManager},
		pools::{default_pool, pool_setup_calls},
	},
};
use crate::chain::centrifuge::RuntimeOrigin;
use sp_core::Pair;
use sp_runtime::traits::IdentifyAccount;

#[tokio::test]
async fn test() {
	ApiEnv::new(Handle::current())
		.startup(|| {
			let mut nft_manager = NftManager::new();
			let mut env = test_env_default(Handle::current());
			default_pool(&mut env, &mut nft_manager, Keyring::Admin, 0);
			let set_loans_for_pools =
				init_loans_for_pool(Keyring::Admin.into(), 0, &mut nft_manager);
					// .into_iter()
					// .map(|call| call.encode())
					// .collect();
			let alice = sp_runtime::AccountId32::from(
				<sr25519::Pair as sp_core::Pair>::from_string("//Alice", None)
					.unwrap()
					.public()
					.into_account(),
			);
			for call in set_loans_for_pools {
				let res = UnfilteredDispatchable::dispatch_bypass_filter(call, Origin::Signed(alice.clone()));
				assert_ok!(res);
			}
			// TestEnv::batch_sign_and_submit(
			// 	&mut env,
			// 	Chain::Para(PARA_ID),
			// 	Keyring::Admin.into(),
			// 	set_loans_for_pools,
			// )
			// .expect("Setup loans for pool calls are succesful");
		})
		.with_api(|api, latest| {
			let valuation = api.currency(&latest, 0).unwrap();
			assert_eq!(valuation, Some(CurrencyId::AUSD));
		});
}
