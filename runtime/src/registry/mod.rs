//! This substrate pallet defines a Verifiable Attributes Registry
//! for minting and managing non-fungible tokens (NFT). A registry
//! can be treated like a class of NFTs, where each class can define
//! unique minting and burning logic upon creation at runtime.
//!
//! There are many ways to define a registry, and this pallet abstracts
//! the notion of a registry into a trait called [VerifierRegistry].
//!
//! In particular, upon creation the VA Registry is supplied with a list
//! of data field names from the fields attribute of the [RegistryInfo]
//! struct. Values for the fields are provided upon each call to
//! [mint](struct.Module.html#method.mint) a new NFT. As can be seen in
//! the values field of the [MintInfo] struct. MintInfo also takes a
//! list of proofs and an anchor id. The mint method will hash the
//! values into leaves of a merkle tree and aggregate with the proofs
//! to generate the root. When the root hash matches that of the anchor,
//! a mint can be verified.

use sp_core::{H256, U256};
use frame_support::{
    decl_module, decl_storage, decl_event, decl_error,
    ensure, dispatch};
use frame_system::ensure_signed;
use sp_std::{cmp::Eq, vec::Vec};
use unique_assets::traits::{Unique, Mintable, Burnable};
pub use types::{*, VerifierRegistry};
use crate::{nft, proofs, anchor};

// Types for this module
pub mod types;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;


pub trait Trait: frame_system::Trait + nft::Trait + anchor::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as VARegistry {
        /// Nonce for generating new registry ids.
        RegistryNonce: RegistryId;
        /// A mapping of all created registries and their metadata.
        Registries: map hasher(blake2_128_concat) RegistryId => RegistryInfo;
        /// A list of asset ids for each registry.
        // TODO: Try a map of BTreeSets as well, and do a benchmark comparison
        NftLists: double_map hasher(identity) RegistryId, hasher(identity) AssetId<T> => ();
    }
}

decl_event!(
    pub enum Event<T>
    where
        AssetId = AssetId<T>,
        AccountId   = <T as frame_system::Trait>::AccountId,
    {
        /// Successful mint of an NFT from fn [`mint`](struct.Module.html#method.mint)
        Mint(AssetId),
        /// Successful creation of a registry from fn
        /// [`create_registry`](./struct.Module.html#method.create_registry)
        RegistryCreated(RegistryId),
        /// Ownership of the asset has been transferred to the account.
        Transferred(AssetId, AccountId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// A given document id does not match a corresponding document in the anchor storage.
        DocumentNotAnchored,
        /// A specified registry is not in the module storage Registries map.
        RegistryDoesNotExist,
        /// Unable to recreate the anchor hash from the proofs and data provided. This means
        /// the [validate_proofs] method failed.
        InvalidProofs,
        /// The values vector provided to a mint call doesn't match the length of the specified
        /// registry's fields vector.
        InvalidMintingValues,
        /// Thrown when someone who is not the owner of a asset attempts to transfer or burn it.
        NotAssetOwner,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = 10_000]
        pub fn create_registry(origin,
                               info: RegistryInfo,
        ) -> dispatch::DispatchResult {
            ensure_signed(origin)?;

            let registry_id = <Self as VerifierRegistry>::create_registry(&info)?;

            // Emit event
            Self::deposit_event(Event::<T>::RegistryCreated(registry_id));

            Ok(())
        }

        #[weight = 10_000]
        pub fn mint(origin,
                    owner_account: <T as frame_system::Trait>::AccountId,
                    asset_info: T::AssetInfo,
                    mint_info: MintInfo<<T as frame_system::Trait>::Hash, H256>,
        ) -> dispatch::DispatchResult {
            ensure_signed(origin)?;

            // Internal mint validates proofs and modifies state or returns error
            let asset_id = <Self as VerifierRegistry>::mint(owner_account,
                                                            asset_info,
                                                            mint_info)?;

            // Mint event
            Self::deposit_event(RawEvent::Mint(asset_id));

            Ok(())
        }

        #[weight = 10_000]
        pub fn burn(origin, asset_id: AssetId<T>) -> dispatch::DispatchResult {
            ensure_signed(origin)?;

            // Internal nft burn logic
            <nft::Module<T>>::burn(&asset_id)

            // TODO: Emit burn event
        }

        /// Transfer a asset to a new owner.
        ///
        /// The dispatch origin for this call must be the asset owner.
        ///
        /// This function will throw an error if the new owner already owns the maximum
        /// number of this type of asset.
        ///
        /// - `dest_account`: Receiver of the asset.
        /// - `asset_id`: The hash (calculated by the runtime system's hashing algorithm)
        ///   of the info that defines the asset to destroy.
        #[weight = 10_000]
        pub fn transfer(origin, dest_account: T::AccountId, asset_id: AssetId<T>) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(who == <nft::Module<T>>::account_for_asset(&asset_id), Error::<T>::NotAssetOwner);

            <nft::Module<T> as Unique>::transfer(&dest_account, &asset_id)?;
            Self::deposit_event(RawEvent::Transferred(asset_id.clone(), dest_account.clone()));
            Ok(())
        }
    }
}

// Auxillary methods of the module for internal use
impl<T: Trait> Module<T> {
    //fn get_document_root(anchor_id: T::Hash) -> Result<T::Hash, dispatch::DispatchError> {
    fn get_document_root(anchor_id: T::Hash) -> Result<H256, dispatch::DispatchError> {
        let root = match <anchor::Module<T>>::get_anchor_by_id(anchor_id) {
            Some(anchor_data) => Ok(anchor_data.doc_root),
            None => Err(Error::<T>::DocumentNotAnchored),
        }?;

        Ok( H256::from_slice(root.as_ref()) )
    }

    fn create_new_registry_id() -> Result<RegistryId, dispatch::DispatchError> {
        let id = <RegistryNonce>::get();

        // Check for overflow on index
        let nplus1 = <RegistryNonce>::get().checked_add(U256::one())
            .ok_or("Overflow when updating registry nonce.")?;

        // Update the nonce
        <RegistryNonce>::put( nplus1 );

        Ok(id)
    }

    // Convert H256 hashes as the little endian encoding
    fn h256_into_u256(h: H256) -> U256 {
        U256::from_little_endian(h.as_fixed_bytes())
    }
}

// Implement the verifier registry. This module verifies data fields that are custom defined
// by a registry and provided in the MintInfo during a mint invocation.
impl<T: Trait> VerifierRegistry for Module<T> {
    type AccountId    = <T as frame_system::Trait>::AccountId;
    type RegistryId   = RegistryId;
    type RegistryInfo = RegistryInfo;
    type AssetId      = AssetId<T>;
    type AssetInfo    = <T as nft::Trait>::AssetInfo;
    // TODO: Change anchor id type to Bytes
    type MintInfo     = MintInfo<<T as frame_system::Trait>::Hash, H256>;

    // Registries with identical RegistryInfo may exist
    fn create_registry(info: &Self::RegistryInfo) -> Result<Self::RegistryId, dispatch::DispatchError> {
        // Generate registry id as nonce
        let id = Self::create_new_registry_id()?;

        // Insert registry in storage
        Registries::insert(id.clone(), info);

        Ok(id)
    }

    fn mint(owner_account: <T as frame_system::Trait>::AccountId,
            asset_info: T::AssetInfo,
            mint_info: MintInfo<<T as frame_system::Trait>::Hash, H256>,
    ) -> Result<Self::AssetId, dispatch::DispatchError> {
        let registry_id   = asset_info.registry_id();
        let registry_info = Registries::get(registry_id);

        // Check that registry exists
        ensure!(
            // TODO: Use the decl above
            Registries::contains_key(registry_id),
            Error::<T>::RegistryDoesNotExist
        );

        // --------------------------
        // Type checking the document

        // The registry field must be a proof with its value as the token id.
        // If not, the document provided may not contain the data and would
        // be invalid.
        // TODO: Check value == token id
        ensure!(
            mint_info.proofs.iter()
                            .map(|p| p.property.clone())
                            .find(|prop| Self::h256_into_u256(H256::from_slice(prop)) == registry_id)
                            .is_some(),
                            //.collect::<Vec<Bytes>>().is_empty(),
            Error::<T>::InvalidProofs);

        // All properties the registry expects must be provided in proofs.
        // If not, the document provided may not contain these fields and would
        // therefore be invalid.
        ensure!(
            registry_info.fields.iter()
                .fold(true, |acc, field|
                      acc &&
                      mint_info.proofs.iter()
                          .map(|p| p.property.clone())
                          .find(|prop| prop == field)
                          .is_some()),
                          //.collect::<Vec<Bytes>>().is_empty()),
            Error::<T>::InvalidProofs);

        // -------------
        // Verify proofs

        // Get the doc root
        let doc_root = Self::get_document_root(mint_info.anchor_id)?;

        // Generate leaf hashes, turn into proofs::Proof type for validation call
        let proofs = mint_info.proofs.into_iter()
            .map(|p| p.into())
            .collect();

        // Verify the proof against document root
        ensure!(proofs::validate_proofs(doc_root,
                                        &proofs,
                                        mint_info.static_hashes),
                Error::<T>::InvalidProofs);

        // -------
        // Minting

        // Internal nft mint
        let asset_id = <nft::Module<T>>::mint(&owner_account, asset_info)?;

        // Place asset id in registry map
        NftLists::<T>::insert(registry_id, asset_id, ());

        Ok(asset_id)
    }
}
