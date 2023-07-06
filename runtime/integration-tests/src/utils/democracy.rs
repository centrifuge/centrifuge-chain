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

use cfg_primitives::Balance;
use chain::centrifuge::{
	BlockNumber, CouncilCollective, Runtime, RuntimeCall, RuntimeEvent, PARA_ID,
};
use codec::Encode;
use frame_support::{dispatch::GetDispatchInfo, traits::Bounded};
use fudge::primitives::Chain;
use pallet_collective::MemberCount;
use pallet_democracy::{
	AccountVote, Call as DemocracyCall, Conviction, PropIndex, ReferendumIndex, ReferendumInfo,
	Vote,
};
use sp_core::{blake2_256, H256};
use sp_runtime::traits::{BlakeTwo256, Hash};

use crate::{
	chain,
	utils::{accounts::Keyring, collective::*, env, env::*, preimage::*},
};

pub fn external_propose_majority(call: &RuntimeCall) -> RuntimeCall {
	let hash = BlakeTwo256::hash_of(call);

	RuntimeCall::Democracy(DemocracyCall::external_propose_majority {
		proposal: Bounded::Legacy {
			hash,
			dummy: Default::default(),
		},
	})
}

pub fn fast_track(
	proposal_hash: H256,
	voting_period: BlockNumber,
	delay: BlockNumber,
) -> RuntimeCall {
	RuntimeCall::Democracy(DemocracyCall::fast_track {
		proposal_hash,
		voting_period,
		delay,
	})
}

pub fn democracy_vote(ref_index: ReferendumIndex, vote: AccountVote<Balance>) -> RuntimeCall {
	RuntimeCall::Democracy(DemocracyCall::vote { ref_index, vote })
}

pub fn execute_via_democracy(
	test_env: &mut TestEnv,
	council_members: Vec<Keyring>,
	original_call: RuntimeCall,
	council_threshold: MemberCount,
	voting_period: BlockNumber,
) {
	let original_call_hash = BlakeTwo256::hash_of(&original_call);

	env::run!(
		test_env,
		Chain::Para(PARA_ID),
		RuntimeCall,
		ChainState::PoolEmpty,
		council_members[0] => note_preimage(&original_call)
	);

	env::assert_events!(
		test_env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		EventRange::All,
		RuntimeEvent::System(frame_system::Event::ExtrinsicFailed{..}) if [count 0],
		RuntimeEvent::Preimage(pallet_preimage::Event::Noted{ hash }) if [*hash == original_call_hash],
	);

	let external_propose_majority_call = external_propose_majority(&original_call);

	execute_collective_proposal(
		test_env,
		&council_members,
		external_propose_majority_call,
		council_threshold,
		0,
	);

	let fast_track_call = fast_track(original_call_hash, voting_period, 0);

	execute_collective_proposal(
		test_env,
		&council_members,
		fast_track_call,
		council_threshold,
		1,
	);

	let vote = AccountVote::<Balance>::Standard {
		vote: Vote {
			aye: true,
			conviction: Conviction::Locked2x,
		},
		balance: 1_000_000u128,
	};

	execute_democracy_vote(test_env, &council_members, 0, vote);
}

fn execute_democracy_vote(
	test_env: &mut TestEnv,
	voters: &Vec<Keyring>,
	referendum_index: ReferendumIndex,
	acc_vote: AccountVote<Balance>,
) {
	for acc in voters {
		test_env.evolve().unwrap();

		let ref_info = test_env
			.with_state(Chain::Para(PARA_ID), || {
				pallet_democracy::ReferendumInfoOf::<Runtime>::get(referendum_index).unwrap()
			})
			.unwrap();

		if let ReferendumInfo::Finished { .. } = ref_info {
			// Referendum might be finished by the time all voters get to vote.
			break;
		}

		env::run!(
			test_env,
			Chain::Para(PARA_ID),
			RuntimeCall,
			ChainState::PoolEmpty,
			*acc => democracy_vote(referendum_index, acc_vote)
		);

		env::assert_events!(
			test_env,
			Chain::Para(PARA_ID),
			RuntimeEvent,
			EventRange::All,
			RuntimeEvent::System(frame_system::Event::ExtrinsicFailed{..}) if [count 0],
			RuntimeEvent::Democracy(pallet_democracy::Event::Voted{
				voter,
				ref_index,
				vote,
			}) if [
				*voter == acc.to_account_id()
				&& *ref_index == referendum_index
				&& *vote == acc_vote
			],
		)
	}
}

fn execute_collective_proposal(
	test_env: &mut TestEnv,
	council_members: &Vec<Keyring>,
	proposal: RuntimeCall,
	council_threshold: MemberCount,
	prop_index: PropIndex,
) {
	let prop_hash = BlakeTwo256::hash_of(&proposal);

	// Council proposal

	env::run!(
		test_env,
		Chain::Para(PARA_ID),
		RuntimeCall,
		ChainState::PoolEmpty,
		council_members[0] => collective_propose(proposal.clone(), council_threshold)
	);

	env::assert_events!(
		test_env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		EventRange::All,
		RuntimeEvent::System(frame_system::Event::ExtrinsicFailed{..}) if [count 0],
		RuntimeEvent::Council(pallet_collective::Event::Proposed{
			account,
			proposal_index,
			proposal_hash,
			threshold,
		}) if [
			*account == council_members[0].to_account_id()
			&& *proposal_index == prop_index
			&& *proposal_hash == prop_hash
			&& *threshold == council_threshold
		],
	);

	// Council voting

	for (index, acc) in council_members.iter().enumerate() {
		env::run!(
			test_env,
			Chain::Para(PARA_ID),
			RuntimeCall,
			ChainState::PoolEmpty,
			*acc => collective_vote(prop_hash, prop_index, true)
		);

		env::assert_events!(
			test_env,
			Chain::Para(PARA_ID),
			RuntimeEvent,
			EventRange::All,
			RuntimeEvent::System(frame_system::Event::ExtrinsicFailed{..}) if [count 0],
			RuntimeEvent::Council(pallet_collective::Event::Voted{
				account,
				proposal_hash,
				voted,
				yes,
				no,
			}) if [
				*account == acc.to_account_id()
				&& *proposal_hash == prop_hash
				&& *voted == true
				&& *yes == (index + 1) as u32
				&& *no == 0
			],
		)
	}

	// Council closing

	let proposal_weight = test_env
		.with_state(Chain::Para(PARA_ID), || {
			let external_proposal =
				pallet_collective::ProposalOf::<Runtime, CouncilCollective>::get(prop_hash)
					.unwrap();

			external_proposal.get_dispatch_info().weight
		})
		.unwrap();

	env::run!(
		test_env,
		Chain::Para(PARA_ID),
		RuntimeCall,
		ChainState::PoolEmpty,
		council_members[0] => collective_close(
			prop_hash,
			prop_index,
			proposal_weight.add(1),
			(proposal.encoded_size() + 1) as u32,
		)
	);

	env::assert_events!(
		test_env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		EventRange::All,
		RuntimeEvent::System(frame_system::Event::ExtrinsicFailed{..}) if [count 0],
		RuntimeEvent::Council(pallet_collective::Event::Closed {
			proposal_hash,
			yes,
			no,
		}) if [
			*proposal_hash == prop_hash
			&& *yes == council_members.len() as u32
			&& *no == 0
		],
		RuntimeEvent::Council(pallet_collective::Event::Approved{
			proposal_hash
		}) if [ *proposal_hash == prop_hash],
		RuntimeEvent::Council(pallet_collective::Event::Executed{
			proposal_hash,
			result,
		}) if [ *proposal_hash == prop_hash && result.is_ok()],
	);
}
