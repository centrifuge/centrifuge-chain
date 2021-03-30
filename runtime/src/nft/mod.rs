//! # Unique Assets Implementation: Assets
//!
//! This creates an NFT-like runtime module by implementing the
//! Unique, Mintable, and Burnable traits of the unique_assets
//! module. The depended-on unique_assets library provides general
//! types for constructing unique assets. Other modules in this
//! runtime can access the interface provided by this module to
//! define user-facing logic to interact with the runtime NFTs.

use crate::va_registry::types::{AssetId, AssetIdRef, TokenId, RegistryId};
use unique_assets::traits::{Unique, Nft, Mintable};
use sp_runtime::{traits::Member, RuntimeDebug};
use codec::{Decode, Encode, FullCodec};
use sp_std::{cmp::Eq, fmt::Debug};
use frame_system::ensure_signed;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch, ensure,
    traits::Get,
    Hashable,
};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: frame_system::Trait {
    /// The data type that is used to describe this type of asset.
    type AssetInfo: Hashable + Member + Debug + Default + FullCodec;
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
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
}

decl_storage! {
    trait Store for Module<T: Trait> as Asset {
        /// A mapping from a asset ID to the account that owns it.
        AccountForAsset get(fn account_for_asset): double_map hasher(blake2_128_concat) RegistryId, hasher(blake2_128_concat) TokenId => Option<T::AccountId>;
        /// A double mapping of registry id and asset id to an asset's info.
        Assets get(fn asset): double_map hasher(blake2_128_concat) RegistryId, hasher(blake2_128_concat) TokenId => Option<<T as Trait>::AssetInfo>;
    }
}

// Empty event to satisfy type constraints
decl_event!(
    pub enum Event<T> where
        <T as frame_system::Trait>::AccountId,
    {
        /// Ownership of the asset has been transferred to the account.
        Transferred(RegistryId, AssetId, AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        // Thrown when there is an attempt to mint a duplicate asset.
        AssetExists,
        // Thrown when there is an attempt to transfer a nonexistent asset.
        NonexistentAsset,
        // Thrown when someone who is not the owner of a asset attempts to transfer or burn it.
        NotAssetOwner,
    }
}

// Empty module so that storage can be declared
decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        /// Transfer a asset to a new owner.
        ///
        /// The dispatch origin for this call must be the asset owner.
        ///
        /// - `dest_account`: Receiver of the asset.
        /// - `asset_id`: The hash (calculated by the runtime system's hashing algorithm)
        ///   of the info that defines the asset to destroy.
        #[weight = T::DbWeight::get().reads_writes(1,1) + 195_000_000]
        pub fn transfer(origin,
                        dest_account: T::AccountId,
                        registry_id: RegistryId,
                        token_id: TokenId)
        -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;

            let asset_id = AssetId(registry_id, token_id);
            <Self as Unique>::transfer(&who, &dest_account, &asset_id)?;

            Self::deposit_event(RawEvent::Transferred(registry_id, asset_id, dest_account));

            Ok(())
        }
    }
}

impl<T: Trait>
    Unique for Module<T>
{
    type Asset = Asset<AssetId, <T as Trait>::AssetInfo>;
    type AccountId = <T as frame_system::Trait>::AccountId;

    fn owner_of(asset_id: &AssetId) -> Option<T::AccountId> {
        let (registry_id, token_id) = AssetIdRef::from(asset_id).destruct();
        Self::account_for_asset(registry_id, token_id)
    }

    fn transfer(
        caller: &T::AccountId,
        dest_account: &T::AccountId,
        asset_id: &AssetId,
    ) -> dispatch::DispatchResult {
        let owner = Self::owner_of(asset_id)
            .ok_or(Error::<T>::NonexistentAsset)?;
        let (registry_id, token_id) = AssetIdRef::from(asset_id).destruct();

        // Check that the caller is owner of asset
        ensure!(caller == &owner,
                Error::<T>::NotAssetOwner);

        // Replace owner with destination account
        AccountForAsset::<T>::insert(registry_id, token_id, dest_account);

        Ok(())
    }
}

impl<T: Trait>
    Mintable for Module<T>
{
    type Asset = Asset<AssetId, <T as Trait>::AssetInfo>;
    type AccountId = <T as frame_system::Trait>::AccountId;

    /// Inserts an owner with a registry/token id.
    /// Does not do any checks on the caller.
    fn mint(
        _caller: &Self::AccountId,
        owner_account: &Self::AccountId,
        asset_id: &AssetId,
        asset_info: <T as Trait>::AssetInfo,
    ) -> dispatch::result::Result<(), dispatch::DispatchError> {
        let (registry_id, token_id) = AssetIdRef::from(asset_id).destruct();

        // Ensure asset with id in registry does not already exist
        ensure!(
            !AccountForAsset::<T>::contains_key(registry_id, token_id),
            Error::<T>::AssetExists
        );

        // Insert into storage
        AccountForAsset::<T>::insert(registry_id, token_id, owner_account);

        //TODO(mig) Pass metadata on asset_info as soon as we have storage fees
        let empty_asset: <T as Trait>::AssetInfo = Default::default();
        Assets::<T>::insert(registry_id, token_id, empty_asset);

        Ok(())
    }
}
