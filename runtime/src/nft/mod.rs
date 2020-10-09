//! # Unique Assets Implementation: Assets
//!
//! This creates an NFT-like runtime module by implementing the
//! Unique, Mintable, and Burnable traits of the unique_assets
//! module. The depended-on unique_assets library provides general
//! types for constructing unique assets. Other modules in this
//! runtime can access the interface provided by this module to
//! define user-facing logic to interact with the runtime NFTs.

use crate::registry::types::{InRegistry, HasId, AssetId, RegistryId};
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

//#[cfg(test)]
//mod mock;

//#[cfg(test)]
//mod tests;

pub trait Trait<I = DefaultInstance>: frame_system::Trait {
    /// The data type that is used to describe this type of asset.
    type AssetInfo: Hashable + Member + Debug + Default + FullCodec + InRegistry + HasId;
    type Event: From<Event<Self, I>> + Into<<Self as frame_system::Trait>::Event>;
}

// A generic definition of an NFT that will be used by this pallet.
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct Asset<Hash, AssetInfo> {
    pub id: Hash,
    pub asset: AssetInfo,
}

impl<AssetId, AssetInfo> Nft for Asset<AssetId, AssetInfo> {
    type Id = AssetId;
    type Info = AssetInfo;
    type RegistryId = RegistryId;
}

decl_storage! {
    trait Store for Module<T: Trait<I>, I: Instance = DefaultInstance> as Asset {
        /// A mapping from a asset ID to the account that owns it.
        AccountForAsset get(fn account_for_asset): double_map hasher(blake2_128_concat) RegistryId, hasher(blake2_128_concat) AssetId => T::AccountId;
        /// A double mapping of registry id and asset id to an asset's info.
        Assets get(fn asset): double_map hasher(blake2_128_concat) RegistryId, hasher(blake2_128_concat) AssetId => <T as Trait<I>>::AssetInfo;
    }
}

// Empty event to satisfy type constraints
decl_event!(
    pub enum Event<T, I = DefaultInstance>
    where
        Hash = <T as frame_system::Trait>::Hash,
    {
        Tmp(Hash),
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
    type Asset = Asset<AssetId, <T as Trait<I>>::AssetInfo>;
    type AccountId = <T as frame_system::Trait>::AccountId;

    fn owner_of(registry_id: &RegistryId, asset_id: &AssetId) -> T::AccountId {
        Self::account_for_asset(registry_id, asset_id)
    }

    fn transfer(
        dest_account: &T::AccountId,
        registry_id: &RegistryId,
        asset_id: &AssetId,
    ) -> dispatch::DispatchResult {
        let owner = Self::owner_of(registry_id, asset_id);
        ensure!(
            owner != T::AccountId::default(),
            Error::<T, I>::NonexistentAsset
        );

        // Replace owner with destination account
        AccountForAsset::<T, I>::insert(&registry_id, &asset_id, &dest_account);

        Ok(())
    }
}

impl<T: Trait<I>, I: Instance>
    Mintable for Module<T, I>
{
    type Asset = Asset<AssetId, <T as Trait<I>>::AssetInfo>;
    type AccountId = <T as frame_system::Trait>::AccountId;

    fn mint(
        owner_account: &T::AccountId,
        asset_info: <T as Trait<I>>::AssetInfo,
    ) -> dispatch::result::Result<AssetId, dispatch::DispatchError> {
        let asset_id = asset_info.id().clone();
        let registry_id = asset_info.registry_id();

        ensure!(
            !AccountForAsset::<T, I>::contains_key(&registry_id, &asset_id),
            Error::<T, I>::AssetExists
        );

        AccountForAsset::<T, I>::insert(&registry_id, asset_id, &owner_account);
        Assets::<T, I>::insert(registry_id, asset_id, asset_info);

        Ok(asset_id)
    }
}
