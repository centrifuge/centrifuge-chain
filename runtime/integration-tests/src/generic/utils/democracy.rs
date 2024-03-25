use std::ops::Add;

use cfg_primitives::{Balance, BlockNumber, CouncilCollective};
use frame_support::{dispatch::GetDispatchInfo, traits::Bounded, weights::Weight};
use pallet_collective::{Call as CouncilCall, MemberCount, ProposalIndex};
use pallet_democracy::{
	AccountVote, Call as DemocracyCall, Conviction, PropIndex, ReferendumIndex, ReferendumInfo,
	Vote,
};
use pallet_preimage::Call as PreimageCall;
use parity_scale_codec::Encode;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Hash};

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		//envs::fudge_env::FudgeSupport,
	},
	utils::accounts::Keyring,
};

pub fn note_preimage<T: Runtime>(call: T::RuntimeCallExt) -> T::RuntimeCallExt {
	let encoded_call = call.encode();

	PreimageCall::note_preimage {
		bytes: encoded_call,
	}
	.into()
}

pub fn external_propose_majority<T: Runtime>(call: T::RuntimeCallExt) -> T::RuntimeCallExt {
	let hash = BlakeTwo256::hash_of(&call);

	DemocracyCall::external_propose_majority {
		proposal: Bounded::Legacy {
			hash,
			dummy: Default::default(),
		},
	}
	.into()
}

pub fn fast_track<T: Runtime>(
	proposal_hash: H256,
	voting_period: BlockNumber,
	delay: BlockNumber,
) -> T::RuntimeCallExt {
	DemocracyCall::fast_track {
		proposal_hash,
		voting_period,
		delay,
	}
	.into()
}

/*
pub fn execute_via_democracy<T: Runtime + FudgeSupport>(
	env: &mut impl Env<T>,
	council_members: Vec<Keyring>,
	original_call: T::RuntimeCallExt,
	council_threshold: MemberCount,
	voting_period: BlockNumber,
	starting_prop_index: PropIndex,
	starting_ref_index: ReferendumIndex,
) -> (PropIndex, ReferendumIndex) {
	let original_call_hash = BlakeTwo256::hash_of(&original_call);

	env.submit_later(
		council_members[0].into(),
		note_preimage::<T>(original_call.clone()),
	)
	.expect("Preimage noting is successful");

	env.pass(Blocks::UntilEvent {
		event: pallet_preimage::Event::<T>::Noted {
			hash: original_call_hash,
		}
		.into(),
		limit: 3,
	});

	let external_propose_majority_call = external_propose_majority::<T>(original_call);

	execute_collective_proposal::<T>(
		env,
		&council_members,
		external_propose_majority_call,
		council_threshold,
		starting_prop_index,
	);

	let fast_track_call = fast_track::<T>(original_call_hash, voting_period, 0);

	execute_collective_proposal::<T>(
		env,
		&council_members,
		fast_track_call,
		council_threshold,
		starting_prop_index + 1,
	);

	let vote = AccountVote::<Balance>::Standard {
		vote: Vote {
			aye: true,
			conviction: Conviction::Locked2x,
		},
		balance: 1_000_000u128,
	};

	execute_democracy_vote(env, &council_members, starting_ref_index, vote);

	(starting_prop_index + 2, starting_ref_index + 1)
}
*/

pub fn democracy_vote<T: Runtime>(
	ref_index: ReferendumIndex,
	vote: AccountVote<Balance>,
) -> T::RuntimeCallExt {
	DemocracyCall::vote { ref_index, vote }.into()
}

fn execute_democracy_vote<T: Runtime>(
	env: &mut impl Env<T>,
	voters: &Vec<Keyring>,
	referendum_index: ReferendumIndex,
	acc_vote: AccountVote<Balance>,
) {
	for acc in voters {
		let ref_info = env.parachain_state(|| {
			pallet_democracy::ReferendumInfoOf::<T>::get(referendum_index).unwrap()
		});

		if let ReferendumInfo::Finished { .. } = ref_info {
			// Referendum might be finished by the time all voters get to vote.
			break;
		}

		env.submit_later(*acc, democracy_vote::<T>(referendum_index, acc_vote))
			.expect("Voting is successful");

		env.pass(Blocks::UntilEvent {
			event: pallet_democracy::Event::<T>::Voted {
				voter: acc.to_account_id(),
				ref_index: referendum_index,
				vote: acc_vote,
			}
			.into(),
			limit: 3,
		});
	}
}

pub fn collective_propose<T: Runtime>(
	proposal: T::RuntimeCallExt,
	threshold: MemberCount,
) -> T::RuntimeCallExt {
	let proposal_len = proposal.encode().len();

	CouncilCall::propose {
		threshold,
		proposal: Box::new(proposal),
		length_bound: proposal_len as u32,
	}
	.into()
}

pub fn collective_vote<T: Runtime>(
	proposal: H256,
	index: ProposalIndex,
	approve: bool,
) -> T::RuntimeCallExt {
	CouncilCall::vote {
		proposal,
		index,
		approve,
	}
	.into()
}

pub fn collective_close<T: Runtime>(
	proposal_hash: H256,
	index: ProposalIndex,
	proposal_weight_bound: Weight,
	length_bound: u32,
) -> T::RuntimeCallExt {
	CouncilCall::close {
		proposal_hash,
		index,
		proposal_weight_bound,
		length_bound,
	}
	.into()
}

fn execute_collective_proposal<T: Runtime>(
	env: &mut impl Env<T>,
	council_members: &Vec<Keyring>,
	proposal: T::RuntimeCallExt,
	council_threshold: MemberCount,
	prop_index: PropIndex,
) {
	let prop_hash = BlakeTwo256::hash_of(&proposal);

	env.submit_later(
		council_members[0].into(),
		collective_propose::<T>(proposal.clone(), council_threshold),
	)
	.expect("Collective proposal is successful");

	env.pass(Blocks::UntilEvent {
		event: pallet_collective::Event::<T, CouncilCollective>::Proposed {
			account: council_members[0].into(),
			proposal_index: prop_index,
			proposal_hash: prop_hash,
			threshold: council_threshold,
		}
		.into(),
		limit: 3,
	});

	for (index, acc) in council_members.iter().enumerate() {
		env.submit_later(*acc, collective_vote::<T>(prop_hash, prop_index, true))
			.expect("Collective voting is successful");

		env.pass(Blocks::UntilEvent {
			event: pallet_collective::Event::<T, CouncilCollective>::Voted {
				account: council_members[0].into(),
				proposal_hash: prop_hash,
				voted: true,
				yes: (index + 1) as u32,
				no: 0,
			}
			.into(),
			limit: 3,
		});
	}

	let proposal_weight = env.parachain_state(|| {
		let external_proposal =
			pallet_collective::ProposalOf::<T, CouncilCollective>::get(prop_hash).unwrap();

		external_proposal.get_dispatch_info().weight
	});

	env.submit_later(
		council_members[0].into(),
		collective_close::<T>(
			prop_hash,
			prop_index,
			proposal_weight.add(1.into()),
			(proposal.encoded_size() + 1) as u32,
		),
	)
	.expect("Collective close is successful");

	env.pass(Blocks::UntilEvent {
		event: pallet_collective::Event::<T, CouncilCollective>::Closed {
			proposal_hash: prop_hash,
			yes: council_members.len() as u32,
			no: 0,
		}
		.into(),
		limit: 3,
	});

	env.check_event(pallet_collective::Event::<T, CouncilCollective>::Approved {
		proposal_hash: prop_hash,
	})
	.expect("Approved event is present.");
	env.check_event(pallet_collective::Event::<T, CouncilCollective>::Executed {
		proposal_hash: prop_hash,
		result: Ok(()),
	})
	.expect("Executed event is present.");
}
