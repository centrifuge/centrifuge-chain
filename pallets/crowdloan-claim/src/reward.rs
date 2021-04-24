// Copyright 2019-2021 Centrifuge Inc.
// This file is part of Cent-Chain.

// Cent-Chain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cent-Chain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cent-Chain.  If not, see <http://www.gnu.org/licenses/>.

//! # Reward Trait of the Claim Pallet
//!
//! ## Overview
//! This trait MUST be implemented by chains, planing to use the `Claim` pallet. As the `Claim` pallet
//! takes care of verifying contributors and preventing replay attacks, the sole purpose of the trait
//! is to provide a function that is called, afters those checks have been passed. The pallet implementing
//! this trait is then responsible for triggering the correct reward-mechanisms.

use frame_support::sp_runtime::traits::{Member, AtLeast32BitUnsigned, MaybeSerializeDeserialize, MaybeSerialize};
use frame_support::Parameter;
use frame_support::dispatch::{Codec, DispatchResult};
use frame_support::dispatch::fmt::Debug;
use sp_runtime::traits::{MaybeDisplay, Bounded, MaybeMallocSizeOf};
use sp_runtime::sp_std::hash::Hash;
use sp_runtime::sp_std::str::FromStr;

pub trait Reward {
    /// The account from the parachain, that the claimer provided in his claim call.
    type ParachainAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeSerialize + Ord
        + Default;

    /// The contribution amount in the token of the relay chain.
    type ContributionAmount: Parameter + Member + AtLeast32BitUnsigned + Codec + Default + Copy +
        MaybeSerializeDeserialize + Debug;

    /// Block number type used by the runtime
    type BlockNumber: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay +
        AtLeast32BitUnsigned + Default + Bounded + Copy + Hash +
        FromStr + MaybeMallocSizeOf;

    /// Rewarding function that will be called ones the Claim Pallet has verified the claimer
    /// If this function returns successfully, any subsequent claim of the same claimer will be
    /// rejected by the claim module.
    fn reward(who: Self::ParachainAccountId, contribution: Self::ContributionAmount) -> DispatchResult;

    /// Initialize function that will be called during the initialization of Claim Pallet.
    /// The main purpose of this function is to allow a dynamic configuration of the reward module.
    fn initialize(
        conversion_rate: u32,
        direct_payout_ratio: u32,
        vesting_period: Self::BlockNumber,
        vesting_start: Self::BlockNumber
    ) -> DispatchResult;
}
