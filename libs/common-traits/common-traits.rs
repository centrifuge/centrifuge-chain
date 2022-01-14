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
trait Trait {
    type Ret;
    type Arg;
    type FixedType;
    const VALUE: u32;
    fn test(arg: Self::Arg) -> Self::Ret;
    fn test_with_self(&self) -> Result<(), ()>;
}
#[allow(unused)]
impl<TupleElement0: Trait> Trait for (TupleElement0,)
where
    TupleElement0: Trait<FixedType = u32>,
{
    type Ret = (TupleElement0::Ret,);
    type Arg = (TupleElement0::Arg,);
    const VALUE: u32 = TupleElement0::VALUE;
    type FixedType = u32;
    fn test(arg: Self::Arg) -> Self::Ret {
        (TupleElement0::test(arg.0),)
    }
    fn test_with_self(&self) -> Result<(), ()> {
        self.0.test_with_self()?;
        Ok(())
    }
}
#[allow(unused)]
impl<TupleElement0: Trait, TupleElement1: Trait> Trait for (TupleElement0, TupleElement1)
where
    TupleElement0: Trait<FixedType = u32>,
    TupleElement1: Trait<FixedType = u32>,
{
    type Ret = (TupleElement0::Ret, TupleElement1::Ret);
    type Arg = (TupleElement0::Arg, TupleElement1::Arg);
    const VALUE: u32 = TupleElement0::VALUE + TupleElement1::VALUE;
    type FixedType = u32;
    fn test(arg: Self::Arg) -> Self::Ret {
        (TupleElement0::test(arg.0), TupleElement1::test(arg.1))
    }
    fn test_with_self(&self) -> Result<(), ()> {
        self.0.test_with_self()?;
        self.1.test_with_self()?;
        Ok(())
    }
}
#[allow(unused)]
impl<TupleElement0: Trait, TupleElement1: Trait, TupleElement2: Trait> Trait
    for (TupleElement0, TupleElement1, TupleElement2)
where
    TupleElement0: Trait<FixedType = u32>,
    TupleElement1: Trait<FixedType = u32>,
    TupleElement2: Trait<FixedType = u32>,
{
    type Ret = (TupleElement0::Ret, TupleElement1::Ret, TupleElement2::Ret);
    type Arg = (TupleElement0::Arg, TupleElement1::Arg, TupleElement2::Arg);
    const VALUE: u32 = TupleElement0::VALUE + TupleElement1::VALUE + TupleElement2::VALUE;
    type FixedType = u32;
    fn test(arg: Self::Arg) -> Self::Ret {
        (
            TupleElement0::test(arg.0),
            TupleElement1::test(arg.1),
            TupleElement2::test(arg.2),
        )
    }
    fn test_with_self(&self) -> Result<(), ()> {
        self.0.test_with_self()?;
        self.1.test_with_self()?;
        self.2.test_with_self()?;
        Ok(())
    }
}
#[allow(unused)]
impl<TupleElement0: Trait, TupleElement1: Trait, TupleElement2: Trait, TupleElement3: Trait> Trait
    for (TupleElement0, TupleElement1, TupleElement2, TupleElement3)
where
    TupleElement0: Trait<FixedType = u32>,
    TupleElement1: Trait<FixedType = u32>,
    TupleElement2: Trait<FixedType = u32>,
    TupleElement3: Trait<FixedType = u32>,
{
    type Ret = (
        TupleElement0::Ret,
        TupleElement1::Ret,
        TupleElement2::Ret,
        TupleElement3::Ret,
    );
    type Arg = (
        TupleElement0::Arg,
        TupleElement1::Arg,
        TupleElement2::Arg,
        TupleElement3::Arg,
    );
    const VALUE: u32 =
        TupleElement0::VALUE + TupleElement1::VALUE + TupleElement2::VALUE + TupleElement3::VALUE;
    type FixedType = u32;
    fn test(arg: Self::Arg) -> Self::Ret {
        (
            TupleElement0::test(arg.0),
            TupleElement1::test(arg.1),
            TupleElement2::test(arg.2),
            TupleElement3::test(arg.3),
        )
    }
    fn test_with_self(&self) -> Result<(), ()> {
        self.0.test_with_self()?;
        self.1.test_with_self()?;
        self.2.test_with_self()?;
        self.3.test_with_self()?;
        Ok(())
    }
}
#[allow(unused)]
impl<
        TupleElement0: Trait,
        TupleElement1: Trait,
        TupleElement2: Trait,
        TupleElement3: Trait,
        TupleElement4: Trait,
    > Trait
    for (
        TupleElement0,
        TupleElement1,
        TupleElement2,
        TupleElement3,
        TupleElement4,
    )
where
    TupleElement0: Trait<FixedType = u32>,
    TupleElement1: Trait<FixedType = u32>,
    TupleElement2: Trait<FixedType = u32>,
    TupleElement3: Trait<FixedType = u32>,
    TupleElement4: Trait<FixedType = u32>,
{
    type Ret = (
        TupleElement0::Ret,
        TupleElement1::Ret,
        TupleElement2::Ret,
        TupleElement3::Ret,
        TupleElement4::Ret,
    );
    type Arg = (
        TupleElement0::Arg,
        TupleElement1::Arg,
        TupleElement2::Arg,
        TupleElement3::Arg,
        TupleElement4::Arg,
    );
    const VALUE: u32 = TupleElement0::VALUE
        + TupleElement1::VALUE
        + TupleElement2::VALUE
        + TupleElement3::VALUE
        + TupleElement4::VALUE;
    type FixedType = u32;
    fn test(arg: Self::Arg) -> Self::Ret {
        (
            TupleElement0::test(arg.0),
            TupleElement1::test(arg.1),
            TupleElement2::test(arg.2),
            TupleElement3::test(arg.3),
            TupleElement4::test(arg.4),
        )
    }
    fn test_with_self(&self) -> Result<(), ()> {
        self.0.test_with_self()?;
        self.1.test_with_self()?;
        self.2.test_with_self()?;
        self.3.test_with_self()?;
        self.4.test_with_self()?;
        Ok(())
    }
}
