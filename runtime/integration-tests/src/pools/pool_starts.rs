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
use crate::chain::centrifuge::{Amount, Event, Runtime, PARA_ID};
use crate::pools::utils::*;
use crate::pools::utils::{
	accounts::Keyring,
	env::{ChainState, EventRange},
	loans::NftManager,
	loans::{borrow_call, init_loans_for_pool, issue_default_loan},
	pools::default_pool_calls,
	time::secs::SECONDS_PER_DAY,
	tokens::DECIMAL_BASE_12,
};
use fudge::primitives::Chain;
use sp_runtime::Storage;
use tokio::runtime::Handle;

#[tokio::test]
async fn create_pool() {
	let manager = env::task_manager(Handle::current());
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_balances::<Runtime>(&mut genesis);
		env::test_env_with_centrifuge_storage(&manager, genesis)
	};

	let mut nft_manager = NftManager::new();
	let pool_id = 0u64;
	let loan_amount = 10_000 * DECIMAL_BASE_12;
	let maturity = 90 * SECONDS_PER_DAY;

	env::run!(
		env,
		Chain::Para(PARA_ID),
		ChainState::EvolvedBy(10),
		Keyring::Admin,
		default_pool_calls(Keyring::Admin.into(), pool_id, &mut nft_manager),
		issue_default_loan(
			Keyring::Admin.into(),
			pool_id,
			loan_amount,
			maturity,
			&mut nft_manager
		),
		borrow_call(
			pool_id,
			nft_manager.curr_loan_id(pool_id),
			Amount::from_inner(loan_amount)
		)
	);

	let (block, pool, account) = env
		.with_state(Chain::Para(PARA_ID), || {
			(
				frame_system::Pallet::<Runtime>::block_number(),
				pallet_pools::Pallet::<Runtime>::pool(pool_id),
				frame_system::Pallet::<Runtime>::account(Keyring::Admin.to_account_id()),
			)
		})
		.expect("Get state is available.");

	let events = env::events!(
		env,
		Chain::Para(PARA_ID),
		EventRange::All,
		Event::Loans(..)
			| Event::Pools(..)
			| Event::Uniques(..)
			| Event::System(frame_system::Event::ExtrinsicFailed { .. })
	);

	for event in events {
		tracing::event!(tracing::Level::INFO, ?event);
	}
}
