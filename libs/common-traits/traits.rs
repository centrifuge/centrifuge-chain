#![feature(prelude_import)]
//! # A common trait for centrifuge
//!
//! This crate provides some common traits used by centrifuge.
//! # Reward trait
//! The trait does assume, that any call of reward has been
//! checked for validity. I.e. there are not validation checks
//! provided by the trait.
#[prelude_import]
use std::prelude::rust_2018::*;
#[macro_use]
extern crate std;
use frame_support::dispatch::{Codec, DispatchResult, DispatchResultWithPostInfo};
use frame_support::scale_info::TypeInfo;
use frame_support::Parameter;
use impl_trait_for_tuples::impl_for_tuples;
use sp_runtime::traits::{
    AtLeast32BitUnsigned, Bounded, MaybeDisplay, MaybeMallocSizeOf, MaybeSerialize,
    MaybeSerializeDeserialize, Member, Zero,
};
use sp_runtime::DispatchError;
use sp_std::fmt::Debug;
use sp_std::hash::Hash;
use sp_std::str::FromStr;
/// A trait used for loosely coupling the claim pallet with a reward mechanism.
///
/// ## Overview
/// The crowdloan reward mechanism is separated from the crowdloan claiming process, the latter
/// being generic, acting as a kind of proxy to the rewarding mechanism, that is specific to
/// to each crowdloan campaign. The aim of this pallet is to ensure that a claim for a reward
/// payout is well-formed, checking for replay attacks, spams or invalid claim (e.g. unknown
/// contributor, exceeding reward amount, ...).
/// See the [`crowdloan-reward`] pallet, that implements a reward mechanism with vesting, for
/// instance.
pub trait Reward {
    /// The account from the parachain, that the claimer provided in her/his transaction.
    type ParachainAccountId: Debug
        + Default
        + MaybeSerialize
        + MaybeSerializeDeserialize
        + Member
        + Ord
        + Parameter
        + TypeInfo;
    /// The contribution amount in relay chain tokens.
    type ContributionAmount: AtLeast32BitUnsigned
        + Codec
        + Copy
        + Debug
        + Default
        + MaybeSerializeDeserialize
        + Member
        + Parameter
        + Zero
        + TypeInfo;
    /// Block number type used by the runtime
    type BlockNumber: AtLeast32BitUnsigned
        + Bounded
        + Copy
        + Debug
        + Default
        + FromStr
        + Hash
        + MaybeDisplay
        + MaybeMallocSizeOf
        + MaybeSerializeDeserialize
        + Member
        + Parameter
        + TypeInfo;
    /// Rewarding function that is invoked from the claim pallet.
    ///
    /// If this function returns successfully, any subsequent claim of the same claimer will be
    /// rejected by the claim module.
    fn reward(
        who: Self::ParachainAccountId,
        contribution: Self::ContributionAmount,
    ) -> DispatchResultWithPostInfo;
}
/// A trait used to convert a type to BigEndian format
pub trait BigEndian<T> {
    fn to_big_endian(&self) -> T;
}
/// A trait that can be used to fetch the nav and update nav for a given pool
pub trait PoolNAV<PoolId, Amount> {
    fn nav(pool_id: PoolId) -> Option<(Amount, u64)>;
    fn update_nav(pool_id: PoolId) -> Result<Amount, DispatchError>;
}
/// A trait that support pool inspection operations such as pool existence checks and pool admin of permission set.
pub trait PoolInspect<AccountId> {
    type PoolId: Parameter + Member + Debug + Copy + Default + TypeInfo;
    /// check if the pool exists
    fn pool_exists(pool_id: Self::PoolId) -> bool;
}
/// A trait that support pool reserve operations such as withdraw and deposit
pub trait PoolReserve<AccountId>: PoolInspect<AccountId> {
    type Balance;
    /// Withdraw `amount` from the reserve to the `to` account.
    fn withdraw(pool_id: Self::PoolId, to: AccountId, amount: Self::Balance) -> DispatchResult;
    /// Deposit `amount` from the `from` account into the reserve.
    fn deposit(pool_id: Self::PoolId, from: AccountId, amount: Self::Balance) -> DispatchResult;
}
pub trait Permissions<AccountId> {
    type Location;
    type Role;
    type Error;
    type Ok;
    fn has_permission(location: Self::Location, who: AccountId, role: Self::Role) -> bool;
    fn add_permission(
        location: Self::Location,
        who: AccountId,
        role: Self::Role,
    ) -> Result<Self::Ok, Self::Error>;
    fn rm_permission(
        location: Self::Location,
        who: AccountId,
        role: Self::Role,
    ) -> Result<Self::Ok, Self::Error>;
}
pub trait Properties {
    type Property;
    type Error;
    type Ok;
    fn exists(&self, property: Self::Property) -> bool;
    fn empty(&self) -> bool;
    fn rm(&mut self, property: Self::Property) -> Result<Self::Ok, Self::Error>;
    fn add(&mut self, property: Self::Property) -> Result<Self::Ok, Self::Error>;
}
pub trait PreConditions<T> {
    fn check(t: &T) -> bool;
}
#[allow(unused)]
impl<T> PreConditions<T> for () {
    fn check(t: &T) -> bool {}
}
#[allow(unused)]
impl<T, TupleElement0: PreConditions<T>> PreConditions<T> for (TupleElement0,) {
    fn check(t: &T) -> bool {
        TupleElement0::check(t)
    }
}
#[allow(unused)]
impl<T, TupleElement0: PreConditions<T>, TupleElement1: PreConditions<T>> PreConditions<T>
    for (TupleElement0, TupleElement1)
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t) & TupleElement1::check(t)
    }
}
#[allow(unused)]
impl<
        T,
        TupleElement0: PreConditions<T>,
        TupleElement1: PreConditions<T>,
        TupleElement2: PreConditions<T>,
    > PreConditions<T> for (TupleElement0, TupleElement1, TupleElement2)
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t) & TupleElement1::check(t) & TupleElement2::check(t)
    }
}
#[allow(unused)]
impl<
        T,
        TupleElement0: PreConditions<T>,
        TupleElement1: PreConditions<T>,
        TupleElement2: PreConditions<T>,
        TupleElement3: PreConditions<T>,
    > PreConditions<T> for (TupleElement0, TupleElement1, TupleElement2, TupleElement3)
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t)
            & TupleElement1::check(t)
            & TupleElement2::check(t)
            & TupleElement3::check(t)
    }
}
#[allow(unused)]
impl<
        T,
        TupleElement0: PreConditions<T>,
        TupleElement1: PreConditions<T>,
        TupleElement2: PreConditions<T>,
        TupleElement3: PreConditions<T>,
        TupleElement4: PreConditions<T>,
    > PreConditions<T>
    for (
        TupleElement0,
        TupleElement1,
        TupleElement2,
        TupleElement3,
        TupleElement4,
    )
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t)
            & TupleElement1::check(t)
            & TupleElement2::check(t)
            & TupleElement3::check(t)
            & TupleElement4::check(t)
    }
}
#[allow(unused)]
impl<
        T,
        TupleElement0: PreConditions<T>,
        TupleElement1: PreConditions<T>,
        TupleElement2: PreConditions<T>,
        TupleElement3: PreConditions<T>,
        TupleElement4: PreConditions<T>,
        TupleElement5: PreConditions<T>,
    > PreConditions<T>
    for (
        TupleElement0,
        TupleElement1,
        TupleElement2,
        TupleElement3,
        TupleElement4,
        TupleElement5,
    )
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t)
            & TupleElement1::check(t)
            & TupleElement2::check(t)
            & TupleElement3::check(t)
            & TupleElement4::check(t)
            & TupleElement5::check(t)
    }
}
#[allow(unused)]
impl<
        T,
        TupleElement0: PreConditions<T>,
        TupleElement1: PreConditions<T>,
        TupleElement2: PreConditions<T>,
        TupleElement3: PreConditions<T>,
        TupleElement4: PreConditions<T>,
        TupleElement5: PreConditions<T>,
        TupleElement6: PreConditions<T>,
    > PreConditions<T>
    for (
        TupleElement0,
        TupleElement1,
        TupleElement2,
        TupleElement3,
        TupleElement4,
        TupleElement5,
        TupleElement6,
    )
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t)
            & TupleElement1::check(t)
            & TupleElement2::check(t)
            & TupleElement3::check(t)
            & TupleElement4::check(t)
            & TupleElement5::check(t)
            & TupleElement6::check(t)
    }
}
#[allow(unused)]
impl<
        T,
        TupleElement0: PreConditions<T>,
        TupleElement1: PreConditions<T>,
        TupleElement2: PreConditions<T>,
        TupleElement3: PreConditions<T>,
        TupleElement4: PreConditions<T>,
        TupleElement5: PreConditions<T>,
        TupleElement6: PreConditions<T>,
        TupleElement7: PreConditions<T>,
    > PreConditions<T>
    for (
        TupleElement0,
        TupleElement1,
        TupleElement2,
        TupleElement3,
        TupleElement4,
        TupleElement5,
        TupleElement6,
        TupleElement7,
    )
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t)
            & TupleElement1::check(t)
            & TupleElement2::check(t)
            & TupleElement3::check(t)
            & TupleElement4::check(t)
            & TupleElement5::check(t)
            & TupleElement6::check(t)
            & TupleElement7::check(t)
    }
}
#[allow(unused)]
impl<
        T,
        TupleElement0: PreConditions<T>,
        TupleElement1: PreConditions<T>,
        TupleElement2: PreConditions<T>,
        TupleElement3: PreConditions<T>,
        TupleElement4: PreConditions<T>,
        TupleElement5: PreConditions<T>,
        TupleElement6: PreConditions<T>,
        TupleElement7: PreConditions<T>,
        TupleElement8: PreConditions<T>,
    > PreConditions<T>
    for (
        TupleElement0,
        TupleElement1,
        TupleElement2,
        TupleElement3,
        TupleElement4,
        TupleElement5,
        TupleElement6,
        TupleElement7,
        TupleElement8,
    )
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t)
            & TupleElement1::check(t)
            & TupleElement2::check(t)
            & TupleElement3::check(t)
            & TupleElement4::check(t)
            & TupleElement5::check(t)
            & TupleElement6::check(t)
            & TupleElement7::check(t)
            & TupleElement8::check(t)
    }
}
#[allow(unused)]
impl<
        T,
        TupleElement0: PreConditions<T>,
        TupleElement1: PreConditions<T>,
        TupleElement2: PreConditions<T>,
        TupleElement3: PreConditions<T>,
        TupleElement4: PreConditions<T>,
        TupleElement5: PreConditions<T>,
        TupleElement6: PreConditions<T>,
        TupleElement7: PreConditions<T>,
        TupleElement8: PreConditions<T>,
        TupleElement9: PreConditions<T>,
    > PreConditions<T>
    for (
        TupleElement0,
        TupleElement1,
        TupleElement2,
        TupleElement3,
        TupleElement4,
        TupleElement5,
        TupleElement6,
        TupleElement7,
        TupleElement8,
        TupleElement9,
    )
{
    fn check(t: &T) -> bool {
        TupleElement0::check(t)
            & TupleElement1::check(t)
            & TupleElement2::check(t)
            & TupleElement3::check(t)
            & TupleElement4::check(t)
            & TupleElement5::check(t)
            & TupleElement6::check(t)
            & TupleElement7::check(t)
            & TupleElement8::check(t)
            & TupleElement9::check(t)
    }
}
