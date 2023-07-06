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

use codec::Encode;
use frame_support::{traits::Len, weights::Weight};
use pallet_collective::{Call as CouncilCall, MemberCount, ProposalIndex};
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Hash};

use crate::chain::centrifuge::{Runtime, RuntimeCall};

pub fn collective_propose(proposal: RuntimeCall, threshold: MemberCount) -> RuntimeCall {
	let proposal_len = proposal.encode().len();
	let hash = BlakeTwo256::hash_of(&proposal);

	RuntimeCall::Council(CouncilCall::propose {
		threshold,
		proposal: Box::new(proposal),
		length_bound: proposal_len as u32,
	})
}

pub fn collective_vote(proposal: H256, index: ProposalIndex, approve: bool) -> RuntimeCall {
	RuntimeCall::Council(CouncilCall::vote {
		proposal,
		index,
		approve,
	})
}

pub fn collective_close(
	proposal_hash: H256,
	index: ProposalIndex,
	proposal_weight_bound: Weight,
	length_bound: u32,
) -> RuntimeCall {
	RuntimeCall::Council(CouncilCall::close {
		proposal_hash,
		index,
		proposal_weight_bound,
		length_bound,
	})
}
