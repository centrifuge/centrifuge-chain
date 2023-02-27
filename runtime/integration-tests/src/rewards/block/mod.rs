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

use altair_runtime::CFG;
use cfg_primitives::{AccountId, Address, Balance, BlockNumber, ItemId};
use cfg_types::{fixed_point::Rate, permissions::PoolRole};
use codec::Encode;
use frame_support::traits::fungibles::Mutate;
use fudge::primitives::Chain;
use pallet_block_rewards::{AccountRewards, COLLATOR_GROUP_ID, STAKE_CURRENCY_ID};
use pallet_collator_selection::{Call as CollatorSelectionCall, CandidateInfo};
use pallet_loans::types::Asset;
use sp_runtime::{
	traits::{AccountIdConversion, Convert, Zero},
	BoundedVec, DispatchError, SaturatedConversion, Storage, TokenError,
};
use tokio::runtime::Handle;

use crate::{
	chain::centrifuge::{
		BlockRewards, BlockRewardsBase, CollatorSelection, Period, Runtime, RuntimeCall,
		RuntimeEvent, Tokens, PARA_ID,
	},
	pools::utils::{
		accounts::Keyring,
		env::{ChainState, EventRange, TestEnv},
		extrinsics::{nonce_centrifuge, xt_centrifuge},
		time::secs::SECONDS_PER_DAY,
		tokens::DECIMAL_BASE_12,
		*,
	},
};

fn default_collators() -> Vec<Keyring> {
	vec![
		Keyring::Alice,
		Keyring::Bob,
		Keyring::Charlie,
		Keyring::Dave,
		Keyring::Eve,
		Keyring::Ferdie,
	]
}

fn default_genesis_block_rewards(genesis: &mut Storage) {
	genesis::default_native_balances::<Runtime>(genesis);
	genesis::admin_invulnerable::<Runtime>(genesis);
	genesis::default_session_keys::<Runtime>(genesis);
	genesis::admin_collator::<Runtime>(genesis);
}

#[tokio::test]
async fn env_works() {
	let mut genesis = Storage::default();
	default_genesis_block_rewards(&mut genesis);
	let mut env = env::test_env_with_centrifuge_storage(Handle::current(), genesis);

	let collator_accounts: Vec<AccountId> = default_collators()
		.clone()
		.iter()
		.map(|c| c.to_account_id())
		.collect();

	// Ensure default collators are neither candidates nor invulnerables
	env.with_state(Chain::Para(PARA_ID), || {
		let candidates = CollatorSelection::candidates();
		let invulnerables = CollatorSelection::invulnerables();
		assert!(collator_accounts
			.iter()
			.all(|j| !candidates.iter().any(|candidate| &candidate.who == j)
				&& !invulnerables.iter().any(|invulnerable| invulnerable == j)));
	});
}

#[cfg(feature = "fast-runtime")]
#[tokio::test]
async fn collator_list_synchronized() {
	let mut genesis = Storage::default();
	default_genesis_block_rewards(&mut genesis);
	let mut env = env::test_env_with_centrifuge_storage(Handle::current(), genesis);

	let collators = default_collators();
	let collator_accounts: Vec<AccountId> = collators
		.clone()
		.iter()
		.map(|c| c.to_account_id())
		.collect();

	add_collator(&mut env, collators[0]);
	add_collator(&mut env, collators[1]);

	// SESSION 0 -> 1;
	frame_support::assert_ok!(env::pass_n(&mut env, Period::get().into()));
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
	frame_support::assert_ok!(env::pass_n(&mut env, Period::get().into()));

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
	frame_support::assert_ok!(env::pass_n(&mut env, Period::get().into()));

	env::assert_events!(
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
						STAKE_CURRENCY_ID,
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
						STAKE_CURRENCY_ID,
					),
					collator.into(),
				)
				.unwrap()
				.is_zero()
			);
		}
	});

	// SESSION 3 -> 4;
	frame_support::assert_ok!(env::pass_n(&mut env, Period::get().into()));
	env.with_state(Chain::Para(PARA_ID), || {
		for collator in collator_accounts[2..].iter() {
			assert!(
				!<Runtime as pallet_block_rewards::Config>::Rewards::compute_reward(
					(
						<Runtime as pallet_block_rewards::Config>::Domain::get(),
						STAKE_CURRENCY_ID,
					),
					collator.into(),
				)
				.unwrap()
				.is_zero()
			);
		}
	});
}

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

/// Verify assumptions about joining and leaving collators.
pub(crate) fn assert_session_invariants(
	env: &mut TestEnv,
	session_index: u32,
	num_collators: u32,
	joining: Vec<AccountId>,
	leaving: Vec<AccountId>,
) {
	let inc =
		BoundedVec::<_, <Runtime as pallet_block_rewards::Config>::MaxCollators>::truncate_from(
			joining.clone(),
		);
	let out =
		BoundedVec::<_, <Runtime as pallet_block_rewards::Config>::MaxCollators>::truncate_from(
			leaving.clone(),
		);
	let next_num_collators: u32 = num_collators
		.saturating_add(inc.len().saturated_into())
		.saturating_sub(out.len().saturated_into());

	env.with_state(Chain::Para(PARA_ID), || {
		assert_eq!(
			inc,
			BlockRewards::next_epoch_changes().collators.inc,
			"Joining collators mismatch in session {}",
			session_index
		);
		assert_eq!(
			out,
			BlockRewards::next_epoch_changes().collators.out,
			"Leaving collators mismatch in session {}",
			session_index
		);
		assert_eq!(
			num_collators,
			BlockRewards::active_epoch_data().num_collators,
			"Active collator count mismatch in session {}",
			session_index
		);
		assert_eq!(
			Some(next_num_collators),
			BlockRewards::next_epoch_changes().num_collators,
			"Next collator count mismatch in session {}",
			session_index
		);

		// joining should not be staked yet
		assert_all_not_staked(&joining[..]);

		// leaving should still be staked
		assert_all_staked(&leaving[..]);

		let candidates: Vec<AccountId> = CollatorSelection::candidates()
			.into_iter()
			.map(|c| c.who)
			.collect();
		// joining should already be candidates
		assert_all_candidate(&candidates[..], &joining[..]);

		// leaving should not be candidates anymore
		assert_all_not_candidate(&candidates[..], &leaving[..]);
	});

	env::assert_events!(
		env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		EventRange::One(Period::get().saturating_mul(session_index)),
		RuntimeEvent::BlockRewardsBase(pallet_rewards::Event::GroupRewarded { .. }) if [count 1],
		RuntimeEvent::BlockRewards(pallet_block_rewards::Event::NewEpoch { .. }) if [count 1],
	);
}

/// Verifies that each provided account address is staked for block rewards.
fn assert_all_staked(v: &[AccountId]) {
	assert!(v.iter().all(|acc| {
		!<Runtime as pallet_block_rewards::Config>::Rewards::account_stake(
			(
				<Runtime as pallet_block_rewards::Config>::Domain::get(),
				STAKE_CURRENCY_ID,
			),
			acc,
		)
		.is_zero()
	}));
}

/// Verifies that none of the provided account addresses is staked for block rewards.
fn assert_all_not_staked(v: &[AccountId]) {
	assert!(v.iter().all(|acc| {
		<Runtime as pallet_block_rewards::Config>::Rewards::account_stake(
			(
				<Runtime as pallet_block_rewards::Config>::Domain::get(),
				STAKE_CURRENCY_ID,
			),
			acc,
		)
		.is_zero()
	}));
}

/// Verifies that candidates is a superset of the given slice.
fn assert_all_candidate(candidates: &[AccountId], v: &[AccountId]) {
	assert!(v
		.iter()
		.all(|acc| candidates.iter().any(|candidate| acc == candidate)));
}

/// Verifies that both slices are disjoint.
fn assert_all_not_candidate(candidates: &[AccountId], v: &[AccountId]) {
	assert!(v
		.iter()
		.all(|acc| !candidates.iter().any(|candidate| acc == candidate)));
}
