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

//! A module that contains all ids where we should be REALLY careful
//! when changing them.
use frame_support::PalletId;
use sp_runtime::TypeId;

use crate::{
	domain_address::{DomainAddress, DomainLocator},
	investments::InvestmentAccount,
};

// The TypeId impl we derive pool-accounts from
impl<InvestmentId> TypeId for InvestmentAccount<InvestmentId> {
	const TYPE_ID: [u8; 4] = *b"invs";
}

// Pallet-Ids that define pallets accounts
pub const POOLS_PALLET_ID: PalletId = PalletId(*b"roc/pool");
pub const CHAIN_BRIDGE_PALLET_ID: PalletId = PalletId(*b"chnbrdge");
pub const CROWDLOAN_REWARD_PALLET_ID: PalletId = PalletId(*b"cc/rewrd");
pub const CROWDLOAN_CLAIM_PALLET_ID: PalletId = PalletId(*b"cc/claim");
pub const TREASURY_PALLET_ID: PalletId = PalletId(*b"py/trsry");
pub const STAKE_POT_PALLET_ID: PalletId = PalletId(*b"PotStake");
pub const BLOCK_REWARDS_PALLET_ID: PalletId = PalletId(*b"cfg/blrw");
pub const LIQUIDITY_REWARDS_PALLET_ID: PalletId = PalletId(*b"cfg/lqrw");
pub const POOL_FEES_PALLET_ID: PalletId = PalletId(*b"cfg/plfs");
pub const TOKEN_MUX_PALLET_ID: PalletId = PalletId(*b"cfg/tmux");

// Other ids
pub const CHAIN_BRIDGE_HASH_ID: [u8; 13] = *b"cent_nft_hash";
pub const CHAIN_BRIDGE_NATIVE_TOKEN_ID: [u8; 4] = *b"xCFG";

// Reward related
/// The identifier of the group eligible to receive block rewards.
pub const COLLATOR_GROUP_ID: u32 = 1;

impl TypeId for DomainAddress {
	const TYPE_ID: [u8; 4] = *b"dadr";
}

impl<Domain> TypeId for DomainLocator<Domain> {
	const TYPE_ID: [u8; 4] = *b"domn";
}
