//! # Unique Assets Implementation: Assets
//!
//! This creates an NFT-like runtime module by implementing the
//! Unique, Mintable, and Burnable traits of the unique_assets
//! module. The depended-on unique_assets library provides general
//! types for constructing unique assets. Other modules in this
//! runtime can access the interface provided by this module to
//! define user-facing logic to interact with the runtime NFTs.

use crate::registry::types::InRegistry;
use codec::{Decode, Encode, FullCodec};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch, ensure,
    traits::Get,
    Hashable,
};
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

pub trait Trait<I = DefaultInstance>: frame_system::Trait {
    /// The data type that is used to describe this type of asset.
    type AssetInfo: Hashable + Member + Debug + Default + FullCodec + InRegistry;
    /// The maximum number of this type of asset that may exist (minted - burned).
    type AssetLimit: Get<u128>;
    /// The maximum number of this type of asset that any single account may own.
    type UserAssetLimit: Get<u64>;
    type Event: From<Event<Self, I>> + Into<<Self as frame_system::Trait>::Event>;
}

/// The runtime system's hashing algorithm is used to uniquely identify assets.
pub type AssetId<T> = <T as frame_system::Trait>::Hash;

/// A generic definition of an NFT that will be used by this pallet.
#[derive(Encode, Decode, Clone, Eq, RuntimeDebug)]
pub struct Asset<Hash, AssetInfo> {
    pub id: Hash,
    pub asset: AssetInfo,
}

/// An alias for this pallet's NFT implementation.
pub type AssetFor<T, I> = Asset<AssetId<T>, <T as Trait<I>>::AssetInfo>;

impl<AssetId, AssetInfo> Nft for Asset<AssetId, AssetInfo> {
    type Id = AssetId;
    type Info = AssetInfo;
}

// Needed to maintain a sorted list.
impl<AssetId: Ord, AssetInfo: Eq> Ord for Asset<AssetId, AssetInfo> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<AssetId: Ord, AssetInfo> PartialOrd for Asset<AssetId, AssetInfo> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl<AssetId: Eq, AssetInfo> PartialEq for Asset<AssetId, AssetInfo> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

decl_storage! {
    trait Store for Module<T: Trait<I>, I: Instance = DefaultInstance> as Asset {
        /// The total number of this type of asset that exists (minted - burned).
        Total get(fn total): u128 = 0;
        /// The total number of this type of asset that has been burned (may overflow).
        Burned get(fn burned): u128 = 0;
        /// The total number of this type of asset owned by an account.
        TotalForAccount get(fn total_for_account): map hasher(blake2_128_concat) T::AccountId => u64 = 0;
        /// A mapping from an account to a list of all of the assets of this type that are owned by it.
        AssetsForAccount get(fn assets_for_account): map hasher(blake2_128_concat) T::AccountId => Vec<AssetFor<T, I>>;
        /// A mapping from a asset ID to the account that owns it.
        AccountForAsset get(fn account_for_asset): map hasher(identity) AssetId<T> => T::AccountId;
    }
}

// Empty event to satisfy type constraints
decl_event!(
    pub enum Event<T, I = DefaultInstance>
    where
        AssetId = <T as frame_system::Trait>::Hash,
    {
        Tmp(AssetId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait<I>, I: Instance> {
        // Thrown when there is an attempt to mint a duplicate asset.
        AssetExists,
        // Thrown when there is an attempt to burn or transfer a nonexistent asset.
        NonexistentAsset,
        // Thrown when someone who is not the owner of a asset attempts to transfer or burn it.
        NotAssetOwner,
        // Thrown when the asset admin attempts to mint a asset and the maximum number of this
        // type of asset already exists.
        TooManyAssets,
        // Thrown when an attempt is made to mint or transfer a asset to an account that already
        // owns the maximum number of this type of asset.
        TooManyAssetsForAccount,
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
    type Asset = Asset<AssetId<T>, <T as Trait<I>>::AssetInfo>;
    type AccountId = <T as frame_system::Trait>::AccountId;
    type AssetLimit = T::AssetLimit;
    type UserAssetLimit = T::UserAssetLimit;

    fn total() -> u128 {
        Self::total()
    }

    fn total_for_account(account: &T::AccountId) -> u64 {
        Self::total_for_account(account)
    }

    fn assets_for_account(
        account: &T::AccountId,
    ) -> Vec<Asset<AssetId<T>, <T as Trait<I>>::AssetInfo>> {
        Self::assets_for_account(account)
    }

    fn owner_of(asset_id: &AssetId<T>) -> T::AccountId {
        Self::account_for_asset(asset_id)
    }

    fn transfer(
        dest_account: &T::AccountId,
        asset_id: &AssetId<T>,
    ) -> dispatch::DispatchResult {
        let owner = Self::owner_of(&asset_id);
        ensure!(
            owner != T::AccountId::default(),
            Error::<T, I>::NonexistentAsset
        );

        ensure!(
            Self::total_for_account(dest_account) < T::UserAssetLimit::get(),
            Error::<T, I>::TooManyAssetsForAccount
        );

        let xfer_asset = Asset::<AssetId<T>, <T as Trait<I>>::AssetInfo> {
            id: *asset_id,
            asset: <T as Trait<I>>::AssetInfo::default(),
        };

        TotalForAccount::<T, I>::mutate(&owner, |total| *total -= 1);
        TotalForAccount::<T, I>::mutate(dest_account, |total| *total += 1);
        let asset = AssetsForAccount::<T, I>::mutate(owner, |assets| {
            let pos = assets
                .binary_search(&xfer_asset)
                .expect("We already checked that we have the correct owner; qed");
            assets.remove(pos)
        });
        AssetsForAccount::<T, I>::mutate(dest_account, |assets| {
            match assets.binary_search(&asset) {
                Ok(_pos) => {} // should never happen
                Err(pos) => assets.insert(pos, asset),
            }
        });
        AccountForAsset::<T, I>::insert(&asset_id, &dest_account);

        Ok(())
    }
}

impl<T: Trait<I>, I: Instance>
    Mintable for Module<T, I>
{
    type Asset = Asset<AssetId<T>, <T as Trait<I>>::AssetInfo>;
    type AccountId = <T as frame_system::Trait>::AccountId;

    fn mint(
        owner_account: &T::AccountId,
        asset_info: <T as Trait<I>>::AssetInfo,
    ) -> dispatch::result::Result<AssetId<T>, dispatch::DispatchError> {
        let asset_id = T::Hashing::hash_of(&asset_info);

        ensure!(
            !AccountForAsset::<T, I>::contains_key(&asset_id),
            Error::<T, I>::AssetExists
        );

        ensure!(
            Self::total_for_account(owner_account) < T::UserAssetLimit::get(),
            Error::<T, I>::TooManyAssetsForAccount
        );

        ensure!(
            Self::total() < T::AssetLimit::get(),
            Error::<T, I>::TooManyAssets
        );

        let new_asset = Asset {
            id: asset_id,
            asset: asset_info,
        };

        Total::<I>::mutate(|total| *total += 1);
        TotalForAccount::<T, I>::mutate(owner_account, |total| *total += 1);
        AssetsForAccount::<T, I>::mutate(owner_account, |assets| {
            match assets.binary_search(&new_asset) {
                Ok(_pos) => {} // should never happen
                Err(pos) => assets.insert(pos, new_asset),
            }
        });
        AccountForAsset::<T, I>::insert(asset_id, &owner_account);

        Ok(asset_id)
    }
}


impl<T: Trait<I>, I: Instance>
    Burnable for Module<T, I>
{
    type Asset = Asset<AssetId<T>, <T as Trait<I>>::AssetInfo>;

    fn burned() -> u128 {
        Self::burned()
    }

    fn burn(asset_id: &AssetId<T>) -> dispatch::DispatchResult {
        let owner = Self::owner_of(asset_id);
        ensure!(
            owner != T::AccountId::default(),
            Error::<T, I>::NonexistentAsset
        );

        let burn_asset = Asset::<AssetId<T>, <T as Trait<I>>::AssetInfo> {
            id: *asset_id,
            asset: <T as Trait<I>>::AssetInfo::default(),
        };

        Total::<I>::mutate(|total| *total -= 1);
        Burned::<I>::mutate(|total| *total += 1);
        TotalForAccount::<T, I>::mutate(&owner, |total| *total -= 1);
        AssetsForAccount::<T, I>::mutate(owner, |assets| {
            let pos = assets
                .binary_search(&burn_asset)
                .expect("We already checked that we have the correct owner; qed");
            assets.remove(pos);
        });
        AccountForAsset::<T, I>::remove(&asset_id);

        Ok(())
    }
}
