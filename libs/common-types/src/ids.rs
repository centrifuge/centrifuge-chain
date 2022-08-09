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
use crate::PoolLocator;
use frame_support::PalletId;
use sp_runtime::TypeId;

// The TypeId impl we derive pool-accounts from
impl<PoolId> TypeId for PoolLocator<PoolId> {
	const TYPE_ID: [u8; 4] = *b"pool";
}

// Pallet-Ids that define pallets accounts
pub const POOLS_PALLET_ID: PalletId = PalletId(*b"roc/pool");
pub const LOANS_PALLET_ID: PalletId = PalletId(*b"roc/loan");
pub const CHAIN_BRIDGE_PALLET_ID: PalletId = PalletId(*b"cb/bridg");
pub const BRIDGE_PALLET_ID: PalletId = PalletId(*b"c/bridge");
pub const CLAIMS_PALLET_ID: PalletId = PalletId(*b"p/claims");
pub const CROWDLOAN_REWARD_PALLET_ID: PalletId = PalletId(*b"cc/rewrd");
pub const CROWDLOAN_CLAIM_PALLET_ID: PalletId = PalletId(*b"cc/claim");
pub const TREASURY_PALLET_ID: PalletId = PalletId(*b"py/trsry");
pub const NFT_SALES_PALLET_ID: PalletId = PalletId(*b"pal/nfts");
pub const STAKE_POT_PALLET_ID: PalletId = PalletId(*b"PotStake");

// Other ids
pub const CHAIN_BRIDGE_HASH_ID: [u8; 13] = *b"cent_nft_hash";
pub const CHAIN_BRIDGE_NATIVE_TOKEN_ID: [u8; 4] = *b"xCFG";
