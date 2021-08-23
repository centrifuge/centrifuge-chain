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

/// This module defines tinlake specific traits.

use frame_support::traits::{Currency,};
use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, Member, MaybeSerializeDeserialize};
use frame_support::sp_std::default::Default;
use frame_support::dispatch::{DispatchResult, DispatchError};
use frame_support::sp_std::fmt::Debug;
use codec::traits::FullCodec;
use orml_traits::MultiCurrency;

// TODO:
trait StaticPool<PoolId, AssetId, TrancheId> {
    type Tranche;
    type Investor;

    fn assets(pool: PoolId) -> Vec<AssetId>;
    fn add_asset(pool: PoolId, asset: AssetId) -> DispatchResult;
    fn remove_asset(pool: PoolId, asset: AssetId) -> DispatchResult;
    fn tranches(pool: PoolId) -> Vec<Self::Tranche>;
    fn investors(pool: PoolId, Option<TrancheId>) -> Vec<Self::Investor>;
}

// TODO:
trait RevolvingPool<PoolId, BlockNumber, AssetId, TrancheId>: StaticPool<PoolId, AssetId, TrancheId> {
    fn last_epoch(pool: PoolId) -> BlockNumber;
    fn min_epoch(pool: PoolId) -> BlockNumber;
    fn closeable(pool: PoolId) -> bool;
    fn close_epoch(pool: PoolId) -> DispatchResult;
}

// TODO:
// This trait could also be implemented by a Defi-Pallet that acts as a secondary reserve
// besides the pool that will only be used, if the pool is out of capital.
trait Reserve<PoolId, AccountId> {
    type Balance: AtLeast32BitUnsigned + FullCodec + Copy + MaybeSerializeDeserialize + Debug + Default;

    fn deposit(to: PoolId, amount: Self::Balance) -> DispatchResult;
    fn payout(from: PoolId, amount:  Self::Balance) -> DispatchResult;
    fn max_reserve(account: PoolId) ->  Self::Balance;
    fn avail_reserve(account: PoolId) ->  Self::Balance;
}

// TODO:
trait InvestmentPool<PoolId> {
    type Order;

    fn order(pool: PoolId, orders: Vec<Orders>) -> DispatchResult;
}

/// TODO:
trait Owner<AccountId> {
    type Of;

    fn ownership(of: Self::Of, who: AccountId) -> bool;
}

// TODO:
trait Loan<LoanId> {
    type Balance: AtLeast32BitUnsigned + FullCodec + Copy + MaybeSerializeDeserialize + Debug + Default;

    fn borrow(id: LoanId, amount: Self::Balance) -> DispatchResult;
    fn repay(id: LoanId, amount: Self::Balance) -> DispatchResult;
}

// TODO:
trait Lockable<Id> {
    type Reason;

    fn lock(id: Id, reason: Reason) -> DispatchResult;
    fn unlock(id: Id) -> DispatchResult;
    fn locks(id: Id) -> Result<Option<Vec<Self::Reasons>>, DispatchError>;
}

// TODO:
trait Collaterale<Id, AccountId> {
    fn seize(what: Id, custodian: AccountId);
    fn seized(what: Id) -> bool;
}

// TODO:
trait Asset<Id> {
    type Balance;
    type Info;

    fn value(asset: Id) -> Self::Balance;
    fn info(asset: Id) -> Self::Info;
}

// TODO:
trait Accreditation<PooldId, TrancheId, AccountId> {
    fn accredited(pool: PoolId, tranche: TrancheId, who: AccountId) -> bool;
}
