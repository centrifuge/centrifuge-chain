// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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
//! A registry can be treated like a class of NFTs, where each class can define
//! unique minting and burning logic upon creation at runtime.
//!
//! There are many ways to define a registry, and this pallet abstracts
//! the notion of a registry into a trait called [VerifierRegistry].
//!
//! In particular, upon creation the registry is supplied with a list
//! of data field names from the fields attribute of the [RegistryInfo]
//! struct.
//! Values for the fields are provided upon each call to [mint](struct.Module.html#method.mint)
//! a new NFT. As can be seen in the values field of the [MintInfo] struct. MintInfo also takes a
//! list of proofs and an anchor id.
//! The mint method will hash the values into leaves of a Merkle tree and
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
//! `Event` - Overarching type for pallet events.
//!
//! ### Events
//!
//! `Mint(RegistryId, TokenId)` - Successful mint of an NFT from fn [`mint`](struct.Module.html#method.mint).
//! `RegistryCreated(RegistryId)` - Successful creation of a registry.
//! `Tmp(Hash)` - To keep Event parametric.
//!
//! ### Errors
//!
//! `DocumentNotAnchored` - A given document id does not match a corresponding document in the anchor storage.
//! `RegistryDoesNotExist` - A specified registry is not in the module storage Registries map.
//! `RegistryOverflow`- The registry id is too large.
//! `InvalidProofs` - Unable to recreate the anchor hash from the proofs and data provided (i.e. the [verify_proofs] method failed).
//! `InvalidMintingValues` - The values vector provided to a mint call doesn't match the length of the specified registry's fields vector.
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
// Module imports and re-exports
// ----------------------------------------------------------------------------

// Pallet types and traits definition
pub mod traits;
pub mod types;

// Mock runtime for testing
#[cfg(test)]
pub mod mock;

// Unit test cases
#[cfg(test)]
mod tests;

// Runtime benchmarking (for extrinsics weights evaluation)
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// Extrinsics weight information
mod weights;

// Re-export crate types and traits
use crate::{
	traits::{VerifierRegistry, WeightInfo},
	types::{MintInfo, ProofVerifier, RegistryInfo},
};

// Re-export pallet components in crate namespace (for runtime construction)
pub use pallet::*;

use frame_support::{dispatch::DispatchError, ensure};

use frame_system::ensure_signed;

use proofs::Verifier;

use sp_core::H256;
use sp_runtime::traits::Hash;

use common_traits::BigEndian;
use frame_support::pallet_prelude::Get;
use pallet_nft::types::AssetId;
use unique_assets::traits::Mintable;

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
	use pallet_nft::types::AssetId;

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

		/// Weight information for extrinsic in this pallet
		type WeightInfo: WeightInfo;

		#[pallet::constant]
		type NftPrefix: Get<&'static [u8]>;
	}

	// ------------------------------------------------------------------------
	// Pallet events
	// ------------------------------------------------------------------------

	// The macro generates event metadata and derive Clone, Debug, Eq, PartialEq and Codec
	#[pallet::event]
	// The macro generates a function on Pallet to deposit an event
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Successful mint of an NFT
		Mint(T::RegistryId, T::TokenId),

		/// Successful creation of a new registry
		RegistryCreated(T::RegistryId),
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
	pub type Registries<T: Config> =
		StorageMap<_, Blake2_128Concat, T::RegistryId, RegistryInfo, ValueQuery>;

	/// A mapping of owners
	#[pallet::storage]
	#[pallet::getter(fn get_owner)]
	pub type Owner<T: Config> =
		StorageMap<_, Blake2_128Concat, T::RegistryId, T::AccountId, ValueQuery>;

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
	impl<T: Config> Pallet<T> {
		/// Create a new registry
		#[pallet::weight(<T as Config>::WeightInfo::create_registry())]
		pub fn create_registry(
			origin: OriginFor<T>,
			info: RegistryInfo,
		) -> DispatchResultWithPostInfo {
			let caller = ensure_signed(origin)?;

			let registry_id = Self::create_new_registry(caller, info)?;

			Self::deposit_event(Event::<T>::RegistryCreated(registry_id));

			Ok(().into())
		}

		/// Mint token
		#[pallet::weight(<T as Config>::WeightInfo::mint(mint_info.proofs.len()))]
		pub fn mint(
			origin: OriginFor<T>,
			owner_account: T::AccountId,
			registry_id: T::RegistryId,
			token_id: T::TokenId,
			asset_info: T::AssetInfo,
			mint_info: MintInfo<T::Hash, H256>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			// Internal mint validates proofs and modifies state or returns error
			let asset_id = AssetId(registry_id.clone(), token_id.clone());

			<Self as VerifierRegistry>::mint(who, owner_account, asset_id, asset_info, mint_info)?;

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
	/// Create a new identifier for a registry
	fn create_registry_id() -> Result<T::RegistryId, DispatchError> {
		let id_nonce = Self::get_registry_nonce();

		// First 20 bytes of the runtime hash of the nonce
		let mut id: [u8; 20] = [0; 20];
		id.copy_from_slice(&T::Hashing::hash_of(&id_nonce).as_ref()[..20]);

		// Increment and update (storage of) identifier's nonce
		<RegistryNonce<T>>::put(id_nonce.saturating_add(1));

		Ok(id.into())
	}

	/// Return a document's root hash given an anchor identifier.
	fn get_document_root(anchor_id: T::Hash) -> Result<H256, DispatchError> {
		let root = match <pallet_anchors::Pallet<T>>::get_anchor_by_id(anchor_id) {
			Some(anchor_data) => Ok(anchor_data.doc_root),
			None => Err(Error::<T>::DocumentNotAnchored),
		}?;

		let doc_root = H256::from_slice(root.as_ref());
		Ok(doc_root)
		//Ok(<T::Hashing as Hash>::hash(root.as_ref()))
	}
}

// Implement verifier registry trait for the pallet
impl<T: Config> VerifierRegistry for Pallet<T> {
	type AccountId = T::AccountId;
	type RegistryId = T::RegistryId;
	type RegistryInfo = RegistryInfo;
	type AssetId = AssetId<T::RegistryId, T::TokenId>;
	type AssetInfo = T::AssetInfo;
	type MintInfo = MintInfo<T::Hash, H256>;

	// Registries with identical RegistryInfo may exist
	fn create_new_registry(
		caller: T::AccountId,
		mut info: RegistryInfo,
	) -> Result<T::RegistryId, DispatchError> {
		// Generate registry id as nonce
		let id = Self::create_registry_id()?;

		// Create a field of the registry that is the registry id encoded with a prefix
		let pre_reg = [T::NftPrefix::get(), id.as_ref()].concat();
		info.fields.push(pre_reg);

		// Insert registry in storage
		<Registries<T>>::insert(id.clone(), info);

		// Caller is the owner of the registry
		Owner::<T>::insert(id.clone(), caller);

		Ok(id)
	}

	/// Mint of a non-fungible token
	fn mint(
		caller: T::AccountId,
		owner_account: T::AccountId,
		asset_id: AssetId<T::RegistryId, T::TokenId>,
		asset_info: T::AssetInfo,
		mint_info: MintInfo<T::Hash, H256>,
	) -> Result<(), DispatchError> {
		let (registry_id, token_id) = asset_id.clone().destruct();
		let registry_info = <Registries<T>>::get(registry_id.clone());

		// Check that registry exists
		ensure!(
			<Registries<T>>::contains_key(registry_id),
			Error::<T>::RegistryDoesNotExist
		);

		// --------------------------
		// Type checking the document

		// The last element of the registry fields must be a proof with its
		// property as the [NFT_PREFIX:registry_id] and value as the token id.
		// The token id is the value of the same proof, and must match the id
		// provided in the call.
		let idx = registry_info.fields.len() - 1;
		let proof_value = mint_info.proofs[idx].value.clone();
		ensure!(
			proof_value == token_id.to_big_endian(),
			Error::<T>::InvalidProofs
		);

		// All properties the registry expects must be provided in proofs.
		// If not, the document provided may not contain these fields and would
		// therefore be invalid. The order of proofs is assumed to be the same order
		// as the registry fields.
		ensure!(
			registry_info
				.fields
				.iter()
				.zip(mint_info.proofs.iter().map(|p| &p.property))
				.fold(true, |acc, (field, prop)| acc && (field == prop)),
			Error::<T>::InvalidProofs
		);

		// -------------
		// Verify proofs

		// Get the document root hash
		let doc_root = Self::get_document_root(mint_info.anchor_id)?;

		// Generate leaf hashes and turn them into 'proofs::Proof' type for validation call
		let proofs = mint_info
			.proofs
			.into_iter()
			.map(|proof| {
				// Generate leaf hash from property ++ value ++ salt
				proof.into()
			})
			.collect();

		// Create proof verifier given static hashes
		let proof_verifier = ProofVerifier::new(mint_info.static_hashes);

		// Verify the proof against document root
		ensure!(
			proof_verifier.verify_proofs(doc_root, &proofs),
			Error::<T>::InvalidProofs
		);

		// -------
		// Minting

		// Internal NFT mint
		<pallet_nft::Pallet<T>>::mint(caller, owner_account, asset_id, asset_info)?;

		Ok(())
	}
}
