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
use cfg_traits::rewards::AccountRewards;
use cfg_types::tokens::{CurrencyId, StakingCurrency};
use fudge::primitives::Chain;
use sp_runtime::{traits::Zero, BoundedVec, SaturatedConversion};

use crate::{
	chain::centrifuge::{
		BlockRewards, BlockRewardsBase, CollatorSelection, Period, Runtime, RuntimeCall,
		RuntimeEvent, Tokens, PARA_ID,
	},
	utils::env::{assert_events, EventRange, TestEnv},
};

/// Verify assumptions about joining and leaving collators.
pub(crate) fn assert_session_invariants(
	env: &mut TestEnv,
	session_index: u32,
	collator_count: u32,
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
	let next_collator_count: u32 = collator_count
		.saturating_add(inc.len().saturated_into())
		.saturating_sub(out.len().saturated_into());

	env.with_state(Chain::Para(PARA_ID), || {
		assert_eq!(
			inc,
			BlockRewards::next_session_changes().collators.inc,
			"Joining collators mismatch in session {}",
			session_index
		);
		assert_eq!(
			out,
			BlockRewards::next_session_changes().collators.out,
			"Leaving collators mismatch in session {}",
			session_index
		);
		assert_eq!(
			collator_count,
			BlockRewards::active_session_data().collator_count,
			"Active collator count mismatch in session {}",
			session_index
		);
		assert_eq!(
			Some(next_collator_count),
			BlockRewards::next_session_changes().collator_count,
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

	assert_events!(
		env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		EventRange::One(Period::get().saturating_mul(session_index)),
		RuntimeEvent::BlockRewardsBase(pallet_rewards::Event::GroupRewarded { .. }) if [count 1],
		RuntimeEvent::BlockRewards(pallet_block_rewards::Event::NewSession { .. }) if [count 1],
	);
}

/// Verifies that each provided account address is staked for block rewards.
pub(crate) fn assert_all_staked(v: &[AccountId]) {
	assert!(v.iter().all(|acc| {
		!<Runtime as pallet_block_rewards::Config>::Rewards::account_stake(
			(
				<Runtime as pallet_block_rewards::Config>::Domain::get(),
				<Runtime as pallet_block_rewards::Config>::StakeCurrencyId::get(),
			),
			acc,
		)
		.is_zero()
	}));
}

/// Verifies that none of the provided account addresses is staked for block
/// rewards.
fn assert_all_not_staked(v: &[AccountId]) {
	assert!(v.iter().all(|acc| {
		<Runtime as pallet_block_rewards::Config>::Rewards::account_stake(
			(
				<Runtime as pallet_block_rewards::Config>::Domain::get(),
				<Runtime as pallet_block_rewards::Config>::StakeCurrencyId::get(),
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
