// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.


//! # Verifiable attributes (VA) registry pallet
//!
//! This Substrate FRAME pallet defines a **Verifiable Attributes Registry**
//! for minting and managing non-fungible tokens (NFTs).
//!
//! ## Overview
//! This Substrate FRAME pallet defines a Verifiable Attributes Registry
//! for minting and managing non-fungible tokens (NFTs).
//! A registry can be treated like a class of NFTs, where each class can define
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
//! list of proofs and an anchor id. 
//! The mint method will hash the values into leaves of a merkle tree and 
//! aggregate with the proofs to generate the root. When the root hash matches 
//! that of the anchor, a mint can be verified.
//!
//! ## Terminology
//! 
//! ## Usage
//!
//! ## Interface
//!
//! ### Supported Origins
//! Valid origin is an administrator or root.
//!
//! ### Types
//! <code>\`Event\`</code> Overarching type for pallet events.
//!
//! ### Events
//!
//! <code>\`Mint(RegistryId, TokenId)\`</code> Successful mint of an NFT from fn [`mint`](struct.Module.html#method.mint).
//! <code>\`RegistryCreated(RegistryId)\`</code> Successful creation of a registry.
//! <code>\`Tmp(Hash)\`</code> To keep Event parametric.
//!
//! ### Errors
//!
//! <code>\`DocumentNotAnchored\`</code> A given document id does not match a corresponding document in the anchor storage.
//! <code>\`RegistryDoesNotExist\`</code> A specified registry is not in the module storage Registries map.
//! <code>\`RegistryOverflow\`</code> The registry id is too large.
//! <code>\`InvalidProofs\`</code> Unable to recreate the anchor hash from the proofs and data provided (i.e. the [validate_proofs] method failed).
//! <code>\`InvalidMintingValues\`</code> The values vector provided to a mint call doesn't match the length of the specified registry's fields vector.
//!
//! ### Dispatchable Functions
//!
//! Callable functions (or extrinsics), also considered as transactions, materialize the
//! pallet contract. Here's the callable functions implemented in this module:
//!
//! [`create_registry`]
//! [`mint`]
//! 
//! ### Public Functions
//!
//! ## Genesis Configuration
//! The pallet is parameterized and configured via [parameter_types] macro, at the time the runtime is built
//! by means of the [`construct_runtime`] macro.
//!
//! ## Dependencies
//! This pallet is tightly coupled to:
//! - Substrate FRAME's [balances pallet](https://github.com/paritytech/substrate/tree/master/frame/balances).
//!
//! ## References
//! - [Substrate FRAME v2 attribute macros](https://crates.parity.io/frame_support/attr.pallet.html).
//!
//! ## Credits
//! The Centrifugians Tribe <tribe@centrifuge.io>
//!
//! ## License
//! GNU General Public License, Version 3, 29 June 2007 <https://www.gnu.org/licenses/gpl-3.0.html>

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

// Mock runtime for testing
#[cfg(test)]
mod mock;

// Unit test cases
#[cfg(test)]
mod tests;

// Runtime benchmarking (for extrinsics weights evaluation)
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// Extrinsics weight information
mod weights;

// Document proofs building and verification
//
// TODO: Note that proofs will be soon moved to a separate lib.
mod proofs;

// Runtime, system and frame primitives
use frame_support::{
    ensure,
    dispatch::{
        DispatchError,
    },
    weights::{
        Weight,
    },
};

use frame_system::{
  ensure_signed
};

use sp_core::{
    H256,
    H160,
    U256, 
};

use sp_runtime::{
    traits::Hash,
};

// Fungible assets
use unique_assets::traits::Mintable;

// Functions for document proofs verification
pub use crate::proofs::*;

pub use centrifuge_primitives::{
    AssetId,
    MintInfo,
    RegistryId,
    RegistryInfo,
    TokenId,
    VerifierRegistry, 
    NFTS_PREFIX,
};

// Extrinsics weight information
pub use crate::traits::WeightInfo as PalletWeightInfo;

// Re-export in crate namespace (for runtime construction)
pub use pallet::*;


// ----------------------------------------------------------------------------
// Traits and types declaration
// ----------------------------------------------------------------------------

pub mod traits {

    use super::*;
    
    /// Weight information for pallet extrinsics
    ///
    /// Weights are calculated using runtime benchmarking features.
    /// See [`benchmarking`] module for more information. 
    pub trait WeightInfo {
        fn create_registry() -> Weight;
        fn mint() -> Weight;
    }
} // end of 'traits' module


// ----------------------------------------------------------------------------
// Pallet module
// ----------------------------------------------------------------------------

// Verifiable attributes registry pallet module
//
// The name of the pallet is provided by `construct_runtime` and is used as
// the unique identifier for the pallet's storage. It is not defined in the 
// pallet itself.
#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;


    // Verifiable attributes registry pallet type declaration.
    //
    // This structure is a placeholder for traits and functions implementation
    // for the pallet.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);


    // ------------------------------------------------------------------------
    // Pallet configuration
    // ------------------------------------------------------------------------

    /// Verifiable attributes registry pallet's configuration trait.
    ///
    /// Associated types and constants are declared in this trait. If the pallet
    /// depends on other super-traits, the latter must be added to this trait, 
    /// such as, in this case, [`frame_system::Config`] or [`pallet_nft::Config`]
    /// super-traits. Note that [`frame_system::Config`] must always be included.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_nft::Config + pallet_anchors::Config {

        /// Associated type for Event enum
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: PalletWeightInfo;
    }


    // ------------------------------------------------------------------------
    // Pallet events
    // ------------------------------------------------------------------------

    // The macro generates event metadata and derive Clone, Debug, Eq, PartialEq and Codec
    #[pallet::event]
    // The macro generates a function on Pallet to deposit an event
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {

        /// Successful mint of an NFT from fn [`mint`](struct.Module.html#method.mint)
        Mint(RegistryId, TokenId),

        /// Successful creation of a registry from fn
        /// [`create_registry`](./struct.Module.html#method.create_registry)
        RegistryCreated(RegistryId),

        // To keep Event parametric
        Tmp(T::Hash),
    }

    
    // ------------------------------------------------------------------------
    // Pallet storage items
    // ------------------------------------------------------------------------

    /// Nonce for generating new registry ids.
    #[pallet::storage]
	#[pallet::getter(fn get_registry_nonce)]
    pub type RegistryNonce<T: Config> = StorageValue<_, u128, ValueQuery>;
    
    /// A mapping of all created registries and their metadata.
    #[pallet::storage]
	#[pallet::getter(fn get_registries)]
    pub type Registries<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        RegistryId,
        RegistryInfo
    >;

    /// A mapping of owners
    #[pallet::storage]
	#[pallet::getter(fn get_owner)]
    pub type Owner<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        RegistryId, 
        T::AccountId
    >;


    // ------------------------------------------------------------------------
    // Pallet genesis configuration
    // ------------------------------------------------------------------------

	// The genesis configuration type.
	#[pallet::genesis_config]
	pub struct GenesisConfig {
        // nothing to do folks!!!!
    }

	// The default value for the genesis config type.
	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self {
                // nothing to do folks!!!!
            }
		}
	}

	// The build of genesis for the pallet.
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
            // nothing to do folks!!!!
		}
	}


    // ----------------------------------------------------------------------------
    // Pallet lifecycle hooks
    // ----------------------------------------------------------------------------
    
    #[pallet::hooks]
	impl<T:Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}


    // ------------------------------------------------------------------------
    // Pallet errors
    // ------------------------------------------------------------------------

    #[pallet::error]
	pub enum Error<T> {

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


    // ------------------------------------------------------------------------
    // Pallet dispatchable functions
    // ------------------------------------------------------------------------

    // Declare Call struct and implement dispatchable (or callable) functions.
    //
    // Dispatchable functions are transactions modifying the state of the chain. They
    // are also called extrinsics are constitute the pallet's public interface.
    // Note that each parameter used in functions must implement `Clone`, `Debug`, 
    // `Eq`, `PartialEq` and `Codec` traits.
    #[pallet::call]
	impl<T:Config> Pallet<T> {
        
        /// Create a new registry
        #[pallet::weight(<T as Config>::WeightInfo::create_registry())]
        pub fn create_registry(
            origin: OriginFor<T>,
            info: RegistryInfo,
        ) -> DispatchResultWithPostInfo {
            let caller = ensure_signed(origin)?;

            let registry_id = <Self as VerifierRegistry>::create_registry(caller, info)?;

            // Emit event
            Self::deposit_event(Event::<T>::RegistryCreated(registry_id));

            Ok(().into())
        }


        /// Mint token
//        #[weight =
//           (mint_info.proofs.len().saturating_mul(1_000_000) as u64
//               + T::DbWeight::get().reads_writes(3,2)
//                + 195_000_000,
//           DispatchClass::Normal,
//           Pays::Yes)]
        #[pallet::weight(<T as Config>::WeightInfo::mint(mint_info.proofs.len()))]
        pub fn mint(
            origin: OriginFor<T>,
            owner_account: <T as frame_system::Config>::AccountId,
            registry_id: RegistryId,
            token_id: TokenId,
            asset_info: T::AssetInfo,
            mint_info: MintInfo<<T as frame_system::Config>::Hash, H256>,
        ) -> DispatchResultWithPostInfo {

            let who = ensure_signed(origin)?;

            // Internal mint validates proofs and modifies state or returns error
            let asset_id = AssetId(registry_id, token_id);
            <Self as VerifierRegistry>::mint(
                &who,
                &owner_account,
                &asset_id,
                asset_info,
                mint_info
            )?;

            // Mint event
            Self::deposit_event(Event::Mint(registry_id, token_id));

            Ok(().into())
        }
    }

} // end of 'pallet' module


// ----------------------------------------------------------------------------
// Pallet implementation block
// ----------------------------------------------------------------------------

// Pallet implementation block.
//
// This main implementation block contains two categories of functions, namely:
// - Public functions: These are functions that are `pub` and generally fall into 
//   inspector functions that do not write to storage and operation functions that do.
// - Private functions: These are private helpers or utilities that cannot be called 
//   from other pallets.
impl<T: Config> Pallet<T> {

    /// Get document's root hash
    fn get_document_root(anchor_id: T::Hash) -> Result<H256, DispatchError> {
        let root = match <pallet_anchors::Pallet<T>>::get_anchor_by_id(anchor_id) {
            Some(anchor_data) => Ok(anchor_data.doc_root),
            None => Err(Error::<T>::DocumentNotAnchored),
        }?;

        Ok( H256::from_slice(root.as_ref()) )
    }

    fn create_new_registry_id() -> Result<RegistryId, DispatchError> {
        let id_nonce = Self::get_registry_nonce();

        // First 20 bytes of the runtime hash of the nonce
        let id = H160::from_slice(&T::Hashing::hash_of(&id_nonce).as_ref()[..20]);

        // Update the nonce
        <RegistryNonce<T>>::put( id_nonce.saturating_add(1) );

        Ok(id)
    }
}

// Implement the verifier registry trait for the pallet.
//
// This module verifies data fields that are custom defined by a registry and 
// provided in the MintInfo during a mint invocation.
impl<T: Config> VerifierRegistry for Pallet<T> {

    type AccountId    = <T as frame_system::Config>::AccountId;
    type RegistryId   = RegistryId;
    type RegistryInfo = RegistryInfo;
    type AssetId      = AssetId;
    type AssetInfo    = <T as pallet_nft::Config>::AssetInfo;
    type MintInfo     = MintInfo<<T as frame_system::Config>::Hash, H256>;

    // Registries with identical RegistryInfo may exist
    fn create_registry(caller: Self::AccountId, mut info: Self::RegistryInfo) -> Result<Self::RegistryId, dispatch::DispatchError> {
        // Generate registry id as nonce
        let id = Self::create_new_registry_id()?;

        // Create a field of the registry that is the registry id encoded with a prefix
        let pre_reg = [NFTS_PREFIX, id.as_bytes()].concat();
        info.fields.push(pre_reg);

        // Insert registry in storage
        Registries::insert(id.clone(), info);
        // Caller is the owner of the registry
        Owner::<T>::insert(id.clone(), caller);

        Ok(id)
    }

    fn mint(caller: &<T as frame_system::Config>::AccountId,
            owner_account: &<T as frame_system::Config>::AccountId,
            asset_id: &Self::AssetId,
            asset_info: T::AssetInfo,
            mint_info: MintInfo<<T as frame_system::Config>::Hash, H256>,
    ) -> Result<(), DispatchError> {
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
        ensure!(
            proofs::validate_proofs(doc_root,
                &proofs,
                mint_info.static_hashes),
            Error::<T>::InvalidProofs
        );

        // -------
        // Minting

        // Internal nft mint
        <pallet_nft::Pallet<T>>::mint(caller, owner_account, asset_id, asset_info)?;

        Ok(())
    }
}
