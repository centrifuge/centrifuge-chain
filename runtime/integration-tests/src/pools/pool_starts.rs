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
use cfg_primitives::{AccountId, Address, Balance, ItemId};
use cfg_types::{PoolRole, Rate};
use fudge::primitives::Chain;
use pallet_loans::types::Asset;
use sp_runtime::{traits::AccountIdConversion, DispatchError, Storage, TokenError};
use tokio::runtime::Handle;

use crate::{
	chain::centrifuge::{Call, Event, Runtime, PARA_ID},
	pools::utils::{
		accounts::Keyring,
		env::{ChainState, EventRange},
		loans::{borrow_call, init_loans_for_pool, issue_default_loan, NftManager},
		pools::{default_pool_calls, permission_call},
		time::secs::SECONDS_PER_DAY,
		tokens::DECIMAL_BASE_12,
		*,
	},
};

#[tokio::test]
async fn create_init_and_price() {
	// THE MANAGER MUST NOT BE DROPPED! It is the receiver of a lot of channels
	let manager = env::task_manager(Handle::current());
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_balances::<Runtime>(&mut genesis);
		genesis::register_default_asset::<Runtime>(&mut genesis);
		env::test_env_with_centrifuge_storage(&manager, genesis)
	};

	let mut nft_manager = NftManager::new();
	let pool_id = 0u64;
	let loan_amount = 10_000 * DECIMAL_BASE_12;
	let maturity = 90 * SECONDS_PER_DAY;

	env::run!(
		env,
		Chain::Para(PARA_ID),
		Call,
		ChainState::PoolEmpty,
		Keyring::Admin => default_pool_calls(Keyring::Admin.into(), pool_id, &mut nft_manager),
			issue_default_loan(
				Keyring::Admin.into(),
				pool_id,
				loan_amount,
				maturity,
				&mut nft_manager,
			)
	);

	env::assert_events!(
		env,
		Chain::Para(PARA_ID),
		Event,
		EventRange::All,
		Event::System(frame_system::Event::ExtrinsicFailed{..}) if [count 0],
		Event::PoolRegistry(pallet_pool_registry::Event::Created { pool_id, .. }) if [pool_id == 0],
		Event::Loans(pallet_loans::Event::PoolInitialised{pool_id}) if [pool_id == 0],
		Event::Loans(pallet_loans::Event::Created{pool_id, loan_id, collateral})
			if [pool_id == 0 && loan_id == ItemId(1) && collateral == Asset(4294967296, ItemId(1))],
		Event::Loans(pallet_loans::Event::Priced{pool_id, loan_id, ..}) if [pool_id == 0 && loan_id == ItemId(1)],
	);
}
