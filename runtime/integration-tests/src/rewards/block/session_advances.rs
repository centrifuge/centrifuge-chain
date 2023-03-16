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

use cfg_primitives::AccountId;
use cfg_types::tokens::{CurrencyId, StakingCurrency};
use codec::Encode;
use fudge::primitives::Chain;
use pallet_block_rewards::{AccountRewards, COLLATOR_GROUP_ID};
use pallet_collator_selection::Call as CollatorSelectionCall;
use sp_runtime::{traits::Zero, Storage};
use tokio::runtime::Handle;

use crate::{
	chain::centrifuge::{
		BlockRewards, BlockRewardsBase, CollatorSelection, Period, Runtime, RuntimeCall,
		RuntimeEvent, Tokens, PARA_ID,
	},
	rewards::block::{
		env::{default_collators, default_genesis_block_rewards},
		invariants::assert_session_invariants,
	},
	utils::{
		accounts::Keyring,
		env::{
			assert_events, pass_n, test_env_with_centrifuge_storage, ChainState, EventRange,
			TestEnv,
		},
		extrinsics::{nonce_centrifuge, xt_centrifuge},
	},
};

/// Execute extrinsic to add the given keyring as a candidate of the CollatorSelection.
/// Upon success, the collator's session key will be included in the next sessions queued keys
/// and thus be added to `NextEpochChanges::<Runtime>.collators.joining.`
pub(crate) fn add_collator(env: &mut TestEnv, who: Keyring) {
	let xt = xt_centrifuge(
		&env,
		who.clone(),
		nonce_centrifuge(&env, who),
		RuntimeCall::CollatorSelection(CollatorSelectionCall::register_as_candidate {}),
	)
	.unwrap();
	env.append_extrinsic(Chain::Para(PARA_ID), xt.encode())
		.unwrap();
}

#[cfg(feature = "fast-runtime")]
#[tokio::test]
async fn collator_list_synchronized() {
	let mut genesis = Storage::default();
	default_genesis_block_rewards(&mut genesis);
	let mut env = test_env_with_centrifuge_storage(Handle::current(), genesis);

	let collators = default_collators();
	let collator_accounts: Vec<AccountId> = collators
		.clone()
		.iter()
		.map(|c| c.to_account_id())
		.collect();

	add_collator(&mut env, collators[0]);
	add_collator(&mut env, collators[1]);

	// SESSION 0 -> 1;
	frame_support::assert_ok!(pass_n(&mut env, Period::get().into()));
	assert_session_invariants(
		&mut env,
		1,
		1,
		vec![collator_accounts[0].clone(), collator_accounts[1].clone()],
		vec![],
	);

	add_collator(&mut env, collators[2]);
	add_collator(&mut env, collators[3]);
	add_collator(&mut env, collators[4]);
	add_collator(&mut env, collators[5]);

	// SESSION 1 -> 2;
	frame_support::assert_ok!(pass_n(&mut env, Period::get().into()));

	// Alice leaves
	let xt = xt_centrifuge(
		&env,
		collators[0].clone(),
		nonce_centrifuge(&env, collators[0]),
		RuntimeCall::CollatorSelection(CollatorSelectionCall::leave_intent {}),
	)
	.unwrap();
	env.append_extrinsic(Chain::Para(PARA_ID), xt.encode())
		.unwrap();
	assert_session_invariants(
		&mut env,
		2,
		3,
		vec![
			collator_accounts[2].clone(),
			collator_accounts[3].clone(),
			collator_accounts[4].clone(),
			collator_accounts[5].clone(),
		],
		vec![],
	);

	// SESSION 2 -> 3;
	frame_support::assert_ok!(pass_n(&mut env, Period::get().into()));

	assert_events!(
		env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		EventRange::Range(2 * Period::get(), 3 * Period::get()),
		RuntimeEvent::CollatorSelection(pallet_collator_selection::Event::CandidateRemoved { .. }) if [count 1],
	);
	assert_session_invariants(&mut env, 3, 7, vec![], vec![collators[0].clone().into()]);

	env.with_state(Chain::Para(PARA_ID), || {
		for collator in collator_accounts[0..2].iter() {
			assert!(
				!<Runtime as pallet_block_rewards::Config>::Rewards::compute_reward(
					(
						<Runtime as pallet_block_rewards::Config>::Domain::get(),
						CurrencyId::Staking(StakingCurrency::BlockRewards),
					),
					collator.into(),
				)
				.unwrap()
				.is_zero()
			);
		}
		for collator in collator_accounts[2..].iter() {
			assert!(
				<Runtime as pallet_block_rewards::Config>::Rewards::compute_reward(
					(
						<Runtime as pallet_block_rewards::Config>::Domain::get(),
						CurrencyId::Staking(StakingCurrency::BlockRewards),
					),
					collator.into(),
				)
				.unwrap()
				.is_zero()
			);
		}
	});

	// SESSION 3 -> 4;
	frame_support::assert_ok!(pass_n(&mut env, Period::get().into()));
	env.with_state(Chain::Para(PARA_ID), || {
		for collator in collator_accounts[2..].iter() {
			assert!(
				!<Runtime as pallet_block_rewards::Config>::Rewards::compute_reward(
					(
						<Runtime as pallet_block_rewards::Config>::Domain::get(),
						CurrencyId::Staking(StakingCurrency::BlockRewards),
					),
					collator.into(),
				)
				.unwrap()
				.is_zero()
			);
		}
	});
}
