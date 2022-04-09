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
use pallet_loans::types::Asset;
use runtime_common::{AccountId, Address, Balance, InstanceId};
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::Storage;
use tokio::runtime::Handle;

#[tokio::test]
async fn create_init_and_price() {
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
		ChainState::PoolEmpty,
		Keyring::Admin,
		default_pool_calls(Keyring::Admin.into(), pool_id, &mut nft_manager),
		issue_default_loan(
			Keyring::Admin.into(),
			pool_id,
			loan_amount,
			maturity,
			&mut nft_manager,
		)
	);

	let events = env::events!(
		env,
		Chain::Para(PARA_ID),
		Event,
		EventRange::All,
		Event::Loans(..)
			| Event::Pools(..)
			| Event::Uniques(..)
			| Event::System(frame_system::Event::ExtrinsicFailed { .. })
	);

	for event in events.iter() {
		tracing::event!(tracing::Level::INFO, ?event);
	}

	assert!(events.contains(&Event::Pools(pallet_pools::Event::Created(
		0,
		Keyring::Admin.to_account_id(),
	))));
	assert!(
		events.contains(&Event::Uniques(pallet_uniques::Event::Created {
			class: 0,
			creator: Keyring::Admin.to_account_id(),
			owner: Keyring::Admin.to_account_id()
		}))
	);
	assert!(
		events.contains(&Event::Uniques(pallet_uniques::Event::Created {
			class: 4294967296,
			creator: Keyring::Admin.to_account_id(),
			owner: Keyring::Admin.to_account_id()
		}))
	);
	assert!(
		events.contains(&Event::Uniques(pallet_uniques::Event::Issued {
			class: 4294967296,
			instance: InstanceId(1),
			owner: Keyring::Admin.to_account_id()
		}))
	);
	assert!(
		events.contains(&Event::Uniques(pallet_uniques::Event::Issued {
			class: 0,
			instance: InstanceId(1),
			owner: Keyring::Admin.to_account_id()
		}))
	);
	assert!(
		events.contains(&Event::Uniques(pallet_uniques::Event::Transferred {
			class: 4294967296,
			instance: InstanceId(1),
			from: Keyring::Admin.to_account_id(),
			to: common_types::PoolLocator { pool_id }.into_account()
		}))
	);
	assert!(events.contains(&Event::Loans(pallet_loans::Event::PoolInitialised(0))));
	assert!(events.contains(&Event::Loans(pallet_loans::Event::Created(
		0,
		InstanceId(1),
		Asset(4294967296, InstanceId(1))
	))));
	assert!(events.contains(&Event::Loans(pallet_loans::Event::Priced(0, InstanceId(1)))));
}
