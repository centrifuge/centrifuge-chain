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

use frame_support::{
    ensure, dispatch, traits::Get,
    decl_module, decl_storage, decl_event, decl_error,
    weights::{DispatchClass, Pays}};
use sp_core::{H256, U256, H160};
use frame_system::ensure_signed;
use sp_std::{cmp::Eq, vec::Vec};
use unique_assets::traits::Mintable;
pub use types::{*, VerifierRegistry, NFTS_PREFIX};
use crate::{nft, proofs, anchor};

// Types for this module
pub mod types;

#[cfg(test)]
mod mock;

#[cfg(test)]
pub mod tests;

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
    }
}

decl_event!(
    pub enum Event<T>
    where
        <T as frame_system::Trait>::Hash,
    {
        /// Successful mint of an NFT from fn [`mint`](struct.Module.html#method.mint)
        Mint(RegistryId, TokenId),
        /// Successful creation of a registry from fn
        /// [`create_registry`](./struct.Module.html#method.create_registry)
        RegistryCreated(RegistryId),
        // To keep Event parametric
        Tmp(Hash),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// A given document id does not match a corresponding document in the anchor storage.
        DocumentNotAnchored,
        /// A specified registry is not in the module storage Registries map.
        RegistryDoesNotExist,
        /// The registry id is too large.
        RegistryOverflow,
        /// Unable to recreate the anchor hash from the proofs and data provided. This means
        /// the [validate_proofs] method failed.
        InvalidProofs,
        /// The values vector provided to a mint call doesn't match the length of the specified
        /// registry's fields vector.
        InvalidMintingValues,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = T::DbWeight::get().reads_writes(1,2) + 195_000_000]
        pub fn create_registry(origin,
                               info: RegistryInfo,
        ) -> dispatch::DispatchResult {
            ensure_signed(origin)?;

            let registry_id = <Self as VerifierRegistry>::create_registry(info)?;

            // Emit event
            Self::deposit_event(Event::<T>::RegistryCreated(registry_id));

            Ok(())
        }

        #[weight =
            (mint_info.proofs.len().saturating_mul(100_000) as u64
                + T::DbWeight::get().reads_writes(3,2)
                + 195_000_000,
            DispatchClass::Normal,
            Pays::Yes)]
        pub fn mint(origin,
                    owner_account: <T as frame_system::Trait>::AccountId,
                    registry_id: RegistryId,
                    token_id: TokenId,
                    asset_info: T::AssetInfo,
                    mint_info: MintInfo<<T as frame_system::Trait>::Hash, H256>,
        ) -> dispatch::DispatchResult {
            let who = ensure_signed(origin)?;

            // Internal mint validates proofs and modifies state or returns error
            let asset_id = AssetId(registry_id, token_id);
            <Self as VerifierRegistry>::mint(&who,
                                             &owner_account,
                                             &asset_id,
                                             asset_info,
                                             mint_info)?;

            // Mint event
            Self::deposit_event(RawEvent::Mint(registry_id, token_id));

            Ok(())
        }
    }
}

// Auxillary methods of the module for internal use
impl<T: Trait> Module<T> {
    fn get_document_root(anchor_id: T::Hash) -> Result<H256, dispatch::DispatchError> {
        let root = match <anchor::Module<T>>::get_anchor_by_id(anchor_id) {
            Some(anchor_data) => Ok(anchor_data.doc_root),
            None => Err(Error::<T>::DocumentNotAnchored),
        }?;

        Ok( H256::from_slice(root.as_ref()) )
    }

    fn create_new_registry_id() -> Result<RegistryId, dispatch::DispatchError> {
        let id = <RegistryNonce>::get();

        // TODO: Make a U160 type for RegistryId with the uint crate.
        // Passing through U256 is inefficient and H160 is unneeded.
        let mut res = Vec::<u8>::with_capacity(32);
        unsafe { res.set_len(32); }
        // U256 > H160 so no need for a checked_add
        U256::from_little_endian(id.as_bytes())
             .saturating_add(U256::one())
             .to_little_endian(&mut res);

        // Interpreted in big endian
        let nplus1 = H160::from_slice(&res[0..20]);

        // Update the nonce
        <RegistryNonce>::put( nplus1 );

        Ok(id)
    }
}

// Implement the verifier registry. This module verifies data fields that are custom defined
// by a registry and provided in the MintInfo during a mint invocation.
impl<T: Trait> VerifierRegistry for Module<T> {
    type AccountId    = <T as frame_system::Trait>::AccountId;
    type RegistryId   = RegistryId;
    type RegistryInfo = RegistryInfo;
    type AssetId      = AssetId;
    type AssetInfo    = <T as nft::Trait>::AssetInfo;
    type MintInfo     = MintInfo<<T as frame_system::Trait>::Hash, H256>;

    // Registries with identical RegistryInfo may exist
    fn create_registry(mut info: Self::RegistryInfo) -> Result<Self::RegistryId, dispatch::DispatchError> {
        // Generate registry id as nonce
        let id = Self::create_new_registry_id()?;

        // Create a field of the registry that is the registry id encoded with a prefix
        let pre_reg = [NFTS_PREFIX, id.as_bytes()].concat();
        info.fields.push(pre_reg);

        // Insert registry in storage
        Registries::insert(id.clone(), info);

        Ok(id)
    }

    fn mint(caller: &<T as frame_system::Trait>::AccountId,
            owner_account: &<T as frame_system::Trait>::AccountId,
            asset_id: &Self::AssetId,
            asset_info: T::AssetInfo,
            mint_info: MintInfo<<T as frame_system::Trait>::Hash, H256>,
    ) -> Result<(), dispatch::DispatchError> {
        let (registry_id, token_id) = AssetIdRef::from(asset_id).destruct();
        let registry_info = Registries::get(registry_id);

        // Check that registry exists
        ensure!(
            Registries::contains_key(registry_id),
            Error::<T>::RegistryDoesNotExist
        );

        // --------------------------
        // Type checking the document

        // The last element of the registry fields must be a proof with its
        // property as the [NFT_PREFIX:registry_id] and value as the token id.
        // The token id is the value of the same proof, and must match the id
        // provided in the call.
        let idx         = registry_info.fields.len()-1;
        let token_value = &mint_info.proofs[ idx ].value;
        ensure!(
            &U256::from_big_endian(&token_value) == token_id,
            Error::<T>::InvalidProofs);

        // All properties the registry expects must be provided in proofs.
        // If not, the document provided may not contain these fields and would
        // therefore be invalid. The order of proofs is assumed to be the same order
        // as the registry fields.
        ensure!(
            registry_info.fields.iter()
                .zip( mint_info.proofs.iter().map(|p| &p.property) )
                .fold(true, |acc, (field, prop)|
                      acc && (field == prop)),
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
        <nft::Module<T>>::mint(caller, owner_account, asset_id, asset_info)?;

        Ok(())
    }
}
