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

//! # Non-fungible tokens (NFT) processing pallet
//!
//! This creates an NFT-like pallet by implementing the `Unique`, `Mintable`,
//! and `Burnable` traits of the `unique_assets` module.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ## Overview
//! This creates an NFT-like pallet by implementing the 'Unique', 'Mintable', 
//! and 'Burnable' traits of the 'unique_assets' module.
//! The depended-on unique_assets library provides general
//! types for constructing unique assets.
//! Other modules in this runtime can access the interface provided 
//! by this module to define user-facing logic to interact with the 
//! runtime NFTs.
//!
//! ## Terminology
//! 
//! ## Usage
//!
//! ## Interface
//!
//! ### Supported Origins
//!
//! Signed origin is valid.
//!
//! ### Types
//!
//! <code>\`AssetInfo\`</code>  The data type that is used to describe this type of asset.
//! <code>\`Event\`</code> Associated type for Event enum.
//! <code>\`WeightInfo\`</code> Weight information for extrinsics in this pallet.
//!
//! ### Events
//!
//! <code>\`Transferred\`</code> Event triggered when the ownership of the asset has been transferred to the account.
//!
//! ### Errors
//! <code>\`AssetExists\`</code> Thrown when there is an attempt to mint a duplicate asset.
//! <code>\`NonexistentAsset\`</code> Thrown when there is an attempt to transfer a nonexistent asset.
//! <code>\`NotAssetOwner\`</code> Thrown when someone who is not the owner of a asset attempts to transfer or burn it.
//!
//! ### Dispatchable Functions
//!
//! Callable functions (or extrinsics), also considered as transactions, materialize the
//! pallet contract. Here's the callable functions implemented in this module:
//!
//! [`transfer`]
//! 
//! ### Public Functions
//!
//! ## Genesis Configuration
//! The pallet is parameterized and configured via [parameter_types] macro, at the time the runtime is built
//! by means of the [`construct_runtime`] macro.
//!
//! ## Related Pallets
//! This pallet is tightly coupled to the following pallets:
//! - Substrate FRAME's [`balances` pallet](https://github.com/paritytech/substrate/tree/master/frame/balances).
//! - Centrifuge Chain [`bridge` pallet](https://github.com/centrifuge/centrifuge-chain/tree/master/pallets/bridge).
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

// Mock runtime
#[cfg(test)]
mod mock;

// Unit test cases
#[cfg(test)]
mod tests;

// Extrinsics weight information
mod weights;
 
use codec::{
    Decode, 
    Encode, 
    FullCodec
};

// Frame, system and frame primitives
use frame_support::{
    dispatch::{
        DispatchError,
        DispatchResult,
        DispatchResultWithPostInfo,
        result::Result,
    },
    ensure,
    Hashable,
    weights::Weight,
};

use sp_runtime::{
    traits::Member,
    RuntimeDebug,
};

use sp_std::{
    fmt::Debug
};

// Unique asset traits
use unique_assets::traits::{
    Mintable,
    Nft,
    Unique, 
};

//use crate::pallet_va_registry::types::{AssetId, AssetIdRef, TokenId, RegistryId};
use centrifuge_primitives::{
    AssetId, 
    AssetIdRef, 
    TokenId, 
    RegistryId
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
        fn transfer() -> Weight;
    }
} // end of 'traits' module

// Generic definition of a non-fungible token (NFT)
#[derive(Encode, Decode, Default, Clone, RuntimeDebug)]
pub struct Asset<Hash, AssetInfo> {
    pub id: Hash,
    pub asset: AssetInfo,
}

impl<AssetId, AssetInfo> Nft for Asset<AssetId, AssetInfo> {
    type Id = AssetId;
    type Info = AssetInfo;
}


// ----------------------------------------------------------------------------
// Pallet module
// ----------------------------------------------------------------------------

// NFT pallet module
//
// The name of the pallet is provided by `construct_runtime` and is used as
// the unique identifier for the pallet's storage. It is not defined in the 
// pallet itself.
#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    // NFT pallet type declaration.
    //
    // This structure is a placeholder for traits and functions implementation
    // for the pallet.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);


    // ------------------------------------------------------------------------
    // Pallet configuration
    // ------------------------------------------------------------------------

    /// NFT pallet's configuration trait.
    ///
    /// Associated types and constants are declared in this trait. If the pallet
    /// depends on other super-traits, the latter must be added to this trait, 
    /// such as, in this case, [`pallet_balances::Config`] super-traits. Note that 
    /// [`frame_system::Config`] must always be included.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {

        /// The data type that is used to describe this type of asset.
        type AssetInfo: Hashable + Member + Debug + Default + FullCodec;

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
    // Additional argument to specify the metadata to use for given type
    #[pallet::metadata(T::AccountId = "AccountId", RegistryId = "RegistryId")]
    pub enum Event<T: Config> {

        /// Ownership of the asset has been transferred to the account.
        Transferred(RegistryId, AssetId, T::AccountId),
    }


    // ------------------------------------------------------------------------
    // Pallet storage items
    // ------------------------------------------------------------------------

    /// A double mapping of registry ID and asset ID to the account that owns it.
    #[pallet::storage]
	#[pallet::getter(fn account_for_asset)]
    pub type AccountForAsset<T: Config> = StorageDoubleMap<
        _, 
        Blake2_128Concat,
        RegistryId, 
        Blake2_128Concat, 
        TokenId,
        T::AccountId>;

    /// A double mapping of registry ID and asset ID to an asset's info.
    #[pallet::storage]
	#[pallet::getter(fn asset)]
    pub type Assets<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        RegistryId, 
        Blake2_128Concat,
        TokenId,
        T::AssetInfo>;


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

        // Thrown when there is an attempt to mint a duplicate asset.
        AssetExists,

        // Thrown when there is an attempt to transfer a nonexistent asset.
        NonexistentAsset,

        // Thrown when someone who is not the owner of a asset attempts to transfer or burn it.
        NotAssetOwner,
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

        /// Transfer an asset to a new owner.
        ///
        /// The dispatch origin for this call must be the asset owner.
        ///
        /// - `dest_account`: Receiver of the asset.
        /// - `asset_id`: The hash (calculated by the runtime system's hashing algorithm)
        ///   of the info that defines the asset to destroy.
        #[pallet::weight(<T as Config>::WeightInfo::transfer())]
        pub fn transfer(
            origin: OriginFor<T>,
            dest_account: T::AccountId,
            registry_id: RegistryId,
            token_id: TokenId)
        -> DispatchResultWithPostInfo {

            let who = ensure_signed(origin)?;

            let asset_id = AssetId(registry_id, token_id);
            <Self as Unique>::transfer(&who, &dest_account, &asset_id)?;

            Self::deposit_event(Event::Transferred(registry_id, asset_id, dest_account));

            Ok(().into())
        }
    }

} // end of 'pallet' module


// ----------------------------------------------------------------------------
// Pallet implementation block
// ----------------------------------------------------------------------------

// Implement unique trait for pallet
impl<T: Config> Unique for Pallet<T> {

    type Asset = Asset<AssetId, T::AssetInfo>;
    type AccountId = <T as frame_system::Config>::AccountId;

    fn owner_of(asset_id: &AssetId) -> Option<T::AccountId> {
        let (registry_id, token_id) = AssetIdRef::from(asset_id).destruct();
        Self::account_for_asset(registry_id, token_id)
    }

    fn transfer(
        caller: &T::AccountId,
        dest_account: &T::AccountId,
        asset_id: &AssetId,
    ) -> DispatchResult {
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

// Implement mintable trait for pallet
impl<T: Config> Mintable for Pallet<T>
{
    type Asset = Asset<AssetId, T::AssetInfo>;
    type AccountId = T::AccountId;

    /// Inserts an owner with a registry/token id.
    /// Does not do any checks on the caller.
    fn mint(
        _caller: &Self::AccountId,
        owner_account: &Self::AccountId,
        asset_id: &AssetId,
        asset_info: T::AssetInfo,
    ) -> Result<(), DispatchError> {
        let (registry_id, token_id) = AssetIdRef::from(asset_id).destruct();

        // Ensure asset with id in registry does not already exist
        ensure!(
            !AccountForAsset::<T>::contains_key(registry_id, token_id),
            Error::<T>::AssetExists
        );

        // Insert into storage
        AccountForAsset::<T>::insert(registry_id, token_id, owner_account);
        Assets::<T>::insert(registry_id, token_id, asset_info);

        Ok(())
    }
}
