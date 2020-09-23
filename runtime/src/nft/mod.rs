//! # Unique Assets Implementation: Commodities
//!
//! This creates an NFT-like runtime module by implementing the
//! Unique, Mintable, and Burnable traits of the unique_assets
//! module. The depended-on unique_assets library provides general
//! types for constructing unique assets. Other modules in this
//! runtime can access the interface provided by this module to
//! define user-facing logic to interact with the runtime NFTs.

use codec::{Decode, Encode, FullCodec};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch, ensure,
    traits::{EnsureOrigin, Get},
    Hashable,
};
use frame_system::ensure_signed;
use sp_runtime::{
    traits::{Hash, Member},
    RuntimeDebug,
};
use sp_std::{
    cmp::{Eq, Ordering},
    fmt::Debug,
    vec::Vec,
};

use unique_assets::traits::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// TODO: Tmp placed here
pub type RegistryId = u128;
pub trait InRegistry {
    fn registry_id(&self) -> RegistryId;
}

pub trait Trait<I = DefaultInstance>: frame_system::Trait /* Mintable<Self as frame_system::Trait> */ {
    /// The data type that is used to describe this type of commodity.
    type CommodityInfo: Hashable + Member + Debug + Default + FullCodec + InRegistry;
    /// The maximum number of this type of commodity that may exist (minted - burned).
    type CommodityLimit: Get<u128>;
    /// The maximum number of this type of commodity that any single account may own.
    type UserCommodityLimit: Get<u64>;
    type Event: From<Event<Self, I>> + Into<<Self as frame_system::Trait>::Event>;
}

/// The runtime system's hashing algorithm is used to uniquely identify commodities.
pub type CommodityId<T> = <T as frame_system::Trait>::Hash;

/// A generic definition of an NFT that will be used by this pallet.
#[derive(Encode, Decode, Clone, Eq, RuntimeDebug)]
pub struct Commodity<Hash, CommodityInfo> {
    pub id: Hash,
    pub commodity: CommodityInfo,
}

/// An alias for this pallet's NFT implementation.
pub type CommodityFor<T, I> = Commodity<CommodityId<T>, <T as Trait<I>>::CommodityInfo>;

impl<CommodityId, CommodityInfo> Nft for Commodity<CommodityId, CommodityInfo> {
    type Id = CommodityId;
    type Info = CommodityInfo;
}

// Needed to maintain a sorted list.
impl<CommodityId: Ord, CommodityInfo: Eq> Ord for Commodity<CommodityId, CommodityInfo> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<CommodityId: Ord, CommodityInfo> PartialOrd for Commodity<CommodityId, CommodityInfo> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl<CommodityId: Eq, CommodityInfo> PartialEq for Commodity<CommodityId, CommodityInfo> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

decl_storage! {
    trait Store for Module<T: Trait<I>, I: Instance = DefaultInstance> as Commodity {
        /// The total number of this type of commodity that exists (minted - burned).
        Total get(fn total): u128 = 0;
        /// The total number of this type of commodity that has been burned (may overflow).
        Burned get(fn burned): u128 = 0;
        /// The total number of this type of commodity owned by an account.
        TotalForAccount get(fn total_for_account): map hasher(blake2_128_concat) T::AccountId => u64 = 0;
        /// A mapping from an account to a list of all of the commodities of this type that are owned by it.
        CommoditiesForAccount get(fn commodities_for_account): map hasher(blake2_128_concat) T::AccountId => Vec<CommodityFor<T, I>>;
        /// A mapping from a commodity ID to the account that owns it.
        AccountForCommodity get(fn account_for_commodity): map hasher(identity) CommodityId<T> => T::AccountId;
    }
}

// Empty event to satisfy type constraints
decl_event!(
    pub enum Event<T, I = DefaultInstance>
    where
        CommodityId = <T as frame_system::Trait>::Hash,
    {
        Tmp(CommodityId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait<I>, I: Instance> {
        // Thrown when there is an attempt to mint a duplicate commodity.
        CommodityExists,
        // Thrown when there is an attempt to burn or transfer a nonexistent commodity.
        NonexistentCommodity,
        // Thrown when someone who is not the owner of a commodity attempts to transfer or burn it.
        NotCommodityOwner,
        // Thrown when the commodity admin attempts to mint a commodity and the maximum number of this
        // type of commodity already exists.
        TooManyCommodities,
        // Thrown when an attempt is made to mint or transfer a commodity to an account that already
        // owns the maximum number of this type of commodity.
        TooManyCommoditiesForAccount,
    }
}

// Empty module so that storage can be declared
decl_module! {
    pub struct Module<T: Trait<I>, I: Instance = DefaultInstance> for enum Call where origin: T::Origin {
        type Error = Error<T, I>;
        fn deposit_event() = default;
    }
}

impl<T: Trait<I>, I: Instance>
    Unique for Module<T, I>
{
    type Asset = Commodity<CommodityId<T>, <T as Trait<I>>::CommodityInfo>;
    type AccountId = <T as frame_system::Trait>::AccountId;
    type AssetLimit = T::CommodityLimit;
    type UserAssetLimit = T::UserCommodityLimit;

    fn total() -> u128 {
        Self::total()
    }

    fn total_for_account(account: &T::AccountId) -> u64 {
        Self::total_for_account(account)
    }

    fn assets_for_account(
        account: &T::AccountId,
    ) -> Vec<Commodity<CommodityId<T>, <T as Trait<I>>::CommodityInfo>> {
        Self::commodities_for_account(account)
    }

    fn owner_of(commodity_id: &CommodityId<T>) -> T::AccountId {
        Self::account_for_commodity(commodity_id)
    }

    fn transfer(
        dest_account: &T::AccountId,
        commodity_id: &CommodityId<T>,
    ) -> dispatch::DispatchResult {
        let owner = Self::owner_of(&commodity_id);
        ensure!(
            owner != T::AccountId::default(),
            Error::<T, I>::NonexistentCommodity
        );

        ensure!(
            Self::total_for_account(dest_account) < T::UserCommodityLimit::get(),
            Error::<T, I>::TooManyCommoditiesForAccount
        );

        let xfer_commodity = Commodity::<CommodityId<T>, <T as Trait<I>>::CommodityInfo> {
            id: *commodity_id,
            commodity: <T as Trait<I>>::CommodityInfo::default(),
        };

        TotalForAccount::<T, I>::mutate(&owner, |total| *total -= 1);
        TotalForAccount::<T, I>::mutate(dest_account, |total| *total += 1);
        let commodity = CommoditiesForAccount::<T, I>::mutate(owner, |commodities| {
            let pos = commodities
                .binary_search(&xfer_commodity)
                .expect("We already checked that we have the correct owner; qed");
            commodities.remove(pos)
        });
        CommoditiesForAccount::<T, I>::mutate(dest_account, |commodities| {
            match commodities.binary_search(&commodity) {
                Ok(_pos) => {} // should never happen
                Err(pos) => commodities.insert(pos, commodity),
            }
        });
        AccountForCommodity::<T, I>::insert(&commodity_id, &dest_account);

        Ok(())
    }
}

impl<T: Trait<I>, I: Instance>
    Mintable for Module<T, I>
{
    type Asset = Commodity<CommodityId<T>, <T as Trait<I>>::CommodityInfo>;
    type AccountId = <T as frame_system::Trait>::AccountId;

    fn mint(
        owner_account: &T::AccountId,
        commodity_info: <T as Trait<I>>::CommodityInfo,
    ) -> dispatch::result::Result<CommodityId<T>, dispatch::DispatchError> {
        let commodity_id = T::Hashing::hash_of(&commodity_info);

        ensure!(
            !AccountForCommodity::<T, I>::contains_key(&commodity_id),
            Error::<T, I>::CommodityExists
        );

        ensure!(
            Self::total_for_account(owner_account) < T::UserCommodityLimit::get(),
            Error::<T, I>::TooManyCommoditiesForAccount
        );

        ensure!(
            Self::total() < T::CommodityLimit::get(),
            Error::<T, I>::TooManyCommodities
        );

        let new_commodity = Commodity {
            id: commodity_id,
            commodity: commodity_info,
        };

        Total::<I>::mutate(|total| *total += 1);
        TotalForAccount::<T, I>::mutate(owner_account, |total| *total += 1);
        CommoditiesForAccount::<T, I>::mutate(owner_account, |commodities| {
            match commodities.binary_search(&new_commodity) {
                Ok(_pos) => {} // should never happen
                Err(pos) => commodities.insert(pos, new_commodity),
            }
        });
        AccountForCommodity::<T, I>::insert(commodity_id, &owner_account);

        Ok(commodity_id)
    }
}


impl<T: Trait<I>, I: Instance>
    Burnable for Module<T, I>
{
    type Asset = Commodity<CommodityId<T>, <T as Trait<I>>::CommodityInfo>;

    fn burned() -> u128 {
        Self::burned()
    }

    fn burn(commodity_id: &CommodityId<T>) -> dispatch::DispatchResult {
        let owner = Self::owner_of(commodity_id);
        ensure!(
            owner != T::AccountId::default(),
            Error::<T, I>::NonexistentCommodity
        );

        let burn_commodity = Commodity::<CommodityId<T>, <T as Trait<I>>::CommodityInfo> {
            id: *commodity_id,
            commodity: <T as Trait<I>>::CommodityInfo::default(),
        };

        Total::<I>::mutate(|total| *total -= 1);
        Burned::<I>::mutate(|total| *total += 1);
        TotalForAccount::<T, I>::mutate(&owner, |total| *total -= 1);
        CommoditiesForAccount::<T, I>::mutate(owner, |commodities| {
            let pos = commodities
                .binary_search(&burn_commodity)
                .expect("We already checked that we have the correct owner; qed");
            commodities.remove(pos);
        });
        AccountForCommodity::<T, I>::remove(&commodity_id);

        Ok(())
    }
}
