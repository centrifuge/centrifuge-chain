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

///! A module that contains all ids where we should be REALLY careful
///! when changing them.
use crate::PoolLocator;
use sp_runtime::TypeId;
use frame_support::PalletId;

// The TypeId impl we derive pool-accounts from
impl<PoolId> TypeId for PoolLocator<PoolId> {
    const TYPE_ID: [u8; 4] = *b"pool";
}

// Pallet-Ids that define pallets accounts
pub type PoolsPalletId = PalletId(*b"roc/pool");
pub type LoansPalletId = PalletId(*b"roc/loan");
pub type ChainBridgePalletId = PalletId(*b"cb/bridg");
pub type BridgePalletId = PalletId(*b"c/bridge");
pub type ClaimsPalletId = PalletId(*b"p/claims");
pub type CrowdloanRewardPalletId = PalletId(*b"cc/rewrd");
pub type CrowdloanClaimPalletId = PalletId(*b"cc/claim");
pub type TreasuryPalletId = PalletId(*b"py/trsry");
pub type NftSalesPalletId = PalletId(*b"pal/nfts");
pub type StakePotPalletId = PalletId(*b"PotStake");