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

use cfg_primitives::{AccountId, PoolId};
use development_runtime::apis::PoolsApi;
use frame_support::{assert_ok, dispatch::UnfilteredDispatchable, traits::OnInitialize};
use tokio::runtime::Handle;

use super::ApiEnv;
use crate::{
	chain::{centrifuge, centrifuge::RuntimeOrigin},
	pools::utils::{
		accounts::Keyring,
		dispatch::dispatch,
		investments::invest,
		loans::{borrow_call, issue_default_loan, LoanId, NftManager},
		pools::default_pool_calls,
		time::secs::{SECONDS_PER_DAY, SECONDS_PER_YEAR},
		tokens::DECIMAL_BASE_12,
	},
};

const POOL_ID: PoolId = 1;

#[tokio::test]
async fn test() {
	ApiEnv::new(Handle::current())
		.startup(|| {
			let pool_admin: AccountId = Keyring::Alice.into();

			let mut nft_manager = NftManager::new();
			let set_default_pool =
				default_pool_calls(pool_admin.clone(), POOL_ID, &mut nft_manager);

			let issue_default_loans = issue_default_loan(
				pool_admin.clone(),
				POOL_ID,
				100_000 * DECIMAL_BASE_12,
				2 * SECONDS_PER_YEAR,
				&mut nft_manager,
			);

			dispatch!(set_default_pool, pool_admin.clone());

			// set timestamp to around 1 week and update interest accrual
			let now = development_runtime::Timestamp::now();
			let after_one_week = now + 7 * SECONDS_PER_DAY;
			pallet_timestamp::Now::<centrifuge::Runtime>::set(after_one_week.into());
			centrifuge::InterestAccrual::on_initialize(0);

			let investemnt_calls = invest(pool_admin.clone(), POOL_ID, 0, 1_000 * DECIMAL_BASE_12);

			dispatch!(investemnt_calls, pool_admin.clone());
			dispatch!(issue_default_loans, pool_admin.clone());

			let borrow_calls = vec![borrow_call(
				POOL_ID,
				LoanId::from(1_u16),
				10_000 * DECIMAL_BASE_12,
			)];
			dispatch!(borrow_calls, pool_admin.clone());

			// set timestamp to around 1 year
			let now = development_runtime::Timestamp::now();
			let after_one_year = now + SECONDS_PER_YEAR;
			pallet_timestamp::Now::<centrifuge::Runtime>::set(after_one_year.into());
			centrifuge::InterestAccrual::on_initialize(0);
		})
		.with_api(|api, latest| {
			let valuation = api.portfolio_valuation(&latest, POOL_ID).unwrap();
			assert_eq!(valuation, Some(11838183477344413));

			// None existing loan is None
			let max_borrow_amount = api
				.max_borrow_amount(&latest, POOL_ID, LoanId::from(0_u16))
				.unwrap();
			assert_eq!(max_borrow_amount, None);

			// Existing and borrowed loan has Some()
			let max_borrow_amount = api
				.max_borrow_amount(&latest, POOL_ID, LoanId::from(1_u16))
				.unwrap();
			assert_eq!(max_borrow_amount, Some(80_000 * DECIMAL_BASE_12));
		});
}
