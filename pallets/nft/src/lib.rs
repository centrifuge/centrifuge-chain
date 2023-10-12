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

//! # Non-fungible tokens (NFT) processing pallet.
//!
//! This creates an NFT-like pallet by implementing the `Unique`, `Mintable`,
//! and `Burnable` traits of the `unique_assets` module.

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

// Pallet types and traits definition
pub mod types;

// Pallet mock runtime
#[cfg(test)]
mod mock;
// Pallet unit test cases
#[cfg(test)]
mod tests;

// Extrinsic weight information
pub mod weights;

// Export crate types and traits
use cfg_primitives::types::FixedArray;
use cfg_traits::fees::{Fee, Fees};
use chainbridge::types::ResourceId;
use codec::FullCodec;
pub use pallet::*;
use proofs::{hashing::bundled_hash_from_proofs, DepositAddress, Proof, Verifier};
use sp_core::H256;
use sp_std::{fmt::Debug, vec::Vec};
use weights::WeightInfo;

use crate::types::{BundleHasher, HasherHashOf, ProofVerifier, SystemHashOf};

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

	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use super::*;

	// NFT pallet type declaration.
	//
	// This structure is a placeholder for traits and functions implementation
	// for the pallet.
	#[pallet::pallet]
		pub struct Pallet<T>(_);

	// ------------------------------------------------------------------------
	// Pallet configuration
	// ------------------------------------------------------------------------

	/// NFT pallet's configuration trait.
	///
	/// Associated types and constants are declared in this trait. If the pallet
	/// depends on other super-traits, the latter must be added to this trait,
	/// such as, in this case, [`pallet_balances::Config`] super-traits. Note
	/// that [`frame_system::Config`] must always be included.
	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_balances::Config
		+ pallet_anchors::Config
		+ chainbridge::Config
	{
		/// Associated type for Event enum
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Chain identifier type
		type ChainId: Parameter + Member + Debug + Default + FullCodec + Into<u8> + From<u8>;

		/// Resource hash id.
		///
		/// This type was initially declared in the bridge pallet but was moved
		/// here to avoid circular dependencies.
		#[pallet::constant]
		type ResourceHashId: Get<ResourceId>;

		/// Additional fee charged for validating NFT proof (when minting a
		/// NFT).
		#[pallet::constant]
		type NftProofValidationFeeKey: Get<<Self::Fees as Fees>::FeeKey>;

		/// Weight information for extrinsics in this pallet
		type WeightInfo: WeightInfo;
	}

	// ------------------------------------------------------------------------
	// Pallet events
	// ------------------------------------------------------------------------

	// The macro generates event metadata and derive Clone, Debug, Eq, PartialEq and
	// Codec
	#[pallet::event]
	// The macro generates a function on Pallet to deposit an event
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DepositAsset(<T as frame_system::Config>::Hash),
	}

	// ------------------------------------------------------------------------
	// Pallet errors
	// ------------------------------------------------------------------------

	#[pallet::error]
	pub enum Error<T> {
		/// Unable to recreate the anchor hash from the proofs and data
		/// provided.
		InvalidProofs,

		/// A given document id does not match a corresponding document in the
		/// anchor storage.
		DocumentNotAnchored,
	}

	// ------------------------------------------------------------------------
	// Pallet dispatchable functions
	// ------------------------------------------------------------------------

	// Declare Call structure and implement dispatchable (or callable) functions.
	//
	// Dispatchable functions are transactions modifying the state of the chain.
	// They are also called extrinsics are constitute the pallet's public interface.
	// Note that each parameter used in functions must implement `Clone`, `Debug`,
	// `Eq`, `PartialEq` and `Codec` traits.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Validates the proofs provided against the document root associated
		/// with the anchor_id. Once the proofs are verified, we create a
		/// bundled hash `(deposit_address + [proof[i].hash])` Bundled Hash is
		/// deposited to an DepositAsset event for bridging purposes.
		///
		/// Adds additional fee to compensate the current cost of target chains
		/// # <weight>
		/// - depends on the arguments
		/// # </weight>
		#[pallet::weight(<T as Config>::WeightInfo::validate_mint())]
		#[pallet::call_index(0)]
		pub fn validate_mint(
			origin: OriginFor<T>,
			anchor_id: SystemHashOf<T>,
			deposit_address: DepositAddress,
			proofs: Vec<Proof<HasherHashOf<BundleHasher>>>,
			static_proofs: FixedArray<HasherHashOf<BundleHasher>, 3>,
			dest_id: <T as Config>::ChainId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			// Return anchored document root hash
			let anchor_data = <pallet_anchors::Pallet<T>>::get_anchor_by_id(anchor_id)
				.ok_or(Error::<T>::DocumentNotAnchored)?;

			// Create a proof verifier with static proofs
			let proof_verifier = ProofVerifier::new(static_proofs);

			// Validate the proofs again the provided document root
			ensure!(
				proof_verifier
					.verify_proofs(H256::from_slice(anchor_data.doc_root.as_ref()), &proofs),
				Error::<T>::InvalidProofs
			);

			// Returns a Ethereum-compatible Keccak hash of deposit_address +
			// hash(keccak(name+value+salt)) of each proof provided.
			let bundled_hash = Self::get_bundled_hash_from_proofs(proofs, deposit_address);
			Self::deposit_event(Event::<T>::DepositAsset(bundled_hash));

			let metadata = bundled_hash.as_ref().to_vec();

			// Burn additional fees from the calling account
			T::Fees::fee_to_burn(&who, Fee::Key(T::NftProofValidationFeeKey::get()))?;

			let resource_id: ResourceId = T::ResourceHashId::get();
			<chainbridge::Pallet<T>>::transfer_generic(dest_id.into(), resource_id, metadata)?;

			Ok(().into())
		}
	}
} // end of 'pallet' module

// ----------------------------------------------------------------------------
// Pallet implementation block
// ----------------------------------------------------------------------------

// Implement public and private pallet functions.
//
// This main implementation block contains two categories of functions, namely:
// - Public functions: These are functions that are `pub` and generally fall
//   into inspector functions that do not write to storage and operation
//   functions that do.
// - Private functions: These are private helpers or utilities that cannot be
//   called from other pallets.
impl<T: Config> Pallet<T> {
	/// Returns a Ethereum compatible (i.e. Keccak-based) hash.
	///
	/// This function generate a Keccak bundle of deposit_address +
	/// hash(keccak(name+value+salt)) of each proof provided.
	fn get_bundled_hash_from_proofs(
		proofs: Vec<Proof<HasherHashOf<BundleHasher>>>,
		deposit_address: DepositAddress,
	) -> SystemHashOf<T> {
		let bundled_hash = bundled_hash_from_proofs::<BundleHasher>(proofs, deposit_address);
		let mut result: SystemHashOf<T> = Default::default();
		result.as_mut().copy_from_slice(&bundled_hash[..]);
		result
	}
}
