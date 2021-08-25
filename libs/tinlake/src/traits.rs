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
pub trait StaticPool<PoolId> {
    type AssetId;
    type TrancheId;
    type Tranche;
    type Investor;

    fn assets(pool: PoolId) -> Vec<Self::AssetId>;
    fn add_asset(pool: PoolId, asset: Self::AssetId) -> DispatchResult;
    fn remove_asset(pool: PoolId, asset: Self::AssetId) -> DispatchResult;
    fn tranches(pool: PoolId) -> Vec<Self::Tranche>;
    fn investors(pool: PoolId, Option<Self::TrancheId>) -> Vec<Self::Investor>;
}

// TODO:
pub trait RevolvingPool<PoolId, BlockNumber>: StaticPool<PoolId> {
    fn last_epoch(pool: PoolId) -> BlockNumber;
    fn min_epoch(pool: PoolId) -> BlockNumber;
    fn closeable(pool: PoolId) -> bool;
    fn close_epoch(pool: PoolId) -> DispatchResult;
}

// TODO:
// This pub trait could also be implemented by a Defi-Pallet that acts as a secondary reserve
// besides the pool that will only be used, if the pool is out of capital.
pub trait Reserve<AccountId> {
    type Balance: AtLeast32BitUnsigned + FullCodec + Copy + MaybeSerializeDeserialize + Debug + Default;

    fn deposit(from: AccountId, to: AccountId, amount: Self::Balance) -> DispatchResult;
    fn payout(from: AccountId, to: AccountId, amount:  Self::Balance) -> DispatchResult;
    fn max_reserve(account: AccountId) ->  Self::Balance;
    fn avail_reserve(account: AccountId) ->  Self::Balance;
}

// TODO:
pub trait InvestmentPool<PoolId> {
    type Order; //TODO: We will need some trait here which allows to calculate the in-and-out-flows of this type

    fn order(pool: PoolId, orders: Vec<Orders>) -> DispatchResult;
}

/// TODO:
pub trait Owner<AccountId> {
    type Of: Clone;

    fn ownership(of: Self::Of, who: AccountId) -> bool;
}

// TODO:
pub trait Loan<PoolId, LoanId> {
    type Balance: AtLeast32BitUnsigned + FullCodec + Copy + MaybeSerializeDeserialize + Debug + Default;

    fn borrow(pool: PoolId, loan: LoanId, amount: Self::Balance) -> DispatchResult;
    fn repay(pool: PoolId, loan: LoanId, amount: Self::Balance) -> DispatchResult;
}

// TODO:
pub trait Lockable<Id> {
    type Reason;

    fn lock(id: Id, reason: Self::Reason) -> DispatchResult;
    fn unlock(id: Id, reason: Self::Reason) -> DispatchResult;
    fn locks(id: Id) -> Result<Option<Vec<Self::Reasons>>, DispatchError>;
}

// TODO:
pub trait Collaterale<Id, AccountId> {
    fn seize(what: Id, custodian: AccountId);
    fn seized(what: Id) -> bool;
}

// TODO:
pub trait Asset<Id> {
    type Balance;
    type Info;

    fn value(asset: Id) -> Self::Balance;
    fn info(asset: Id) -> Self::Info;
}

// TODO:
pub trait Accreditation<PoolId, TrancheId, AccountId> {
    fn accredited(pool: PoolId, tranche: TrancheId, who: AccountId) -> bool;
}
