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

//! Types used by registry pallet.

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

use codec::{Decode, Encode};

// Library for building and validating proofs
use proofs::{
    Hasher,
    Proof,
    Verifier};

use runtime_common::{
    Bytes, Salt,
};

use sp_core::H256;

use sp_runtime::{sp_std::vec, sp_std::vec::Vec, traits::Hash};

// ----------------------------------------------------------------------------
// Types definition
// ----------------------------------------------------------------------------

/// Metadata for an instance of a registry.
#[derive(Encode, Decode, Clone, PartialEq, Default, Debug)]
pub struct RegistryInfo {
	/// A configuration option that will enable a user to burn their own tokens
	/// in the [burn] method.
	pub owner_can_burn: bool,
	/// Names of fields required to be provided for verification during a [mint].
	/// These *MUST* be compact encoded.
	pub fields: Vec<Bytes>,
}

/// A complete proof that a value for a given property of a document is the real value.
//
/// Proven by hashing hash(value + property + salt) into a leaf hash of the document
/// merkle tree, then hashing with the given hashes to generate the Merkle root.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct CompleteProof<Hash> {
	/// The value of the associated property of a document. Corrseponds to a leaf in
	/// the document merkle tree.
	pub value: Bytes,
	/// A hexified and compact encoded plain text name for a document field.
	pub property: Bytes,
	/// A salt to be concatenated with the value and property before hashing a merkle leaf.
	pub salt: Salt,
	/// A list of all extra hashes required to build the merkle root hash from the leaf.
	pub hashes: Vec<Hash>,
}

/// Generates the leaf hash from underlying data, other hashes remain the same.
impl From<CompleteProof<H256>> for proofs::Proof<H256> {
	fn from(mut proof: CompleteProof<H256>) -> Self {
		// Generate leaf hash from property ++ value ++ salt
		proof.property.extend(proof.value);
		proof.property.extend(&proof.salt);
		let leaf_hash = sp_io::hashing::keccak_256(&proof.property).into();

		Proof::new(leaf_hash, proof.hashes)
	}
}

/// Proof verifier for registry.
pub(crate) struct ProofVerifier<T: frame_system::Config> {
	/// Array containing static root hashes passed when minting a non-fungible token.
	///
	/// See [ProofVerifier::new] for information on how to pass those hashes. Those
	/// root hashes are passed when invoking [mint] transaction (or extrinsic).
	static_hashes: [T::Hash; 3],
}

// Proof verifier implementation block
impl<T: frame_system::Config> ProofVerifier<T> {
	const BASIC_DATA_ROOT_HASH: usize = 0;
	const ZK_DATA_ROOT_HASH: usize = 1;
	const SIGNATURE_ROOT_HASH: usize = 2;

	/// Build a new proof verifier instance, given a list of static root hashes.
	///
	/// The 'root_hashes' must be passed in a specific order, namely:
	///   1. The basic data root hash (with index ['BASIC_DATA_ROOT_HASH'])
	///   2. The ZK root hash (with index ['ZK_DATA_ROOT_HASH'])
	///   3. The signature root hash (with index ['SIGNATURE_DATA_ROOT_HASH'])
	pub fn new(static_hashes: [T::Hash; 3]) -> Self {
		ProofVerifier { static_hashes }
	}
}

// Implement hasher trait for the proof verifier
impl<T: frame_system::Config> Hasher for ProofVerifier<T> {
	type Hash = T::Hash;

	// Hash the input data
	fn hash(data: &[u8]) -> Self::Hash {
		<T::Hashing as Hash>::hash(data)
	}
}

// Implement verifier trait for registry's proof verifier
impl<T: frame_system::Config> Verifier for ProofVerifier<T> {

    // Calculate a final hash from two given hashes
	fn hash_of(a: Self::Hash, b: Self::Hash) -> Self::Hash {
	    proofs::hashing::hash_of::<Self>(a, b)
	}

	// Calculate initial matches.
	//
	// This function takes 3 static proofs and calculates a document root. The
	// calculated document root is then compared with the given document root.
	// If they match, an `Option` containing a list of precomputed hashes is
	// returned, or None if anything goes wrong.
	// The returned precomputed hashes are then used while validating the proofs.
	//
	//
	// Here's how document's root hash is calculated:
	//                      DocumentRoot
	//                      /          \
	//          Signing Root            Signature Root
	//          /          \
	//   data root 1     data root 2
	fn initial_matches(&self, doc_root: Self::Hash) -> Option<Vec<Self::Hash>> {
		let mut matches: Vec<Self::Hash> = vec![];

		let basic_data_root_hash = self.static_hashes[Self::BASIC_DATA_ROOT_HASH];
		let zk_data_root_hash = self.static_hashes[Self::ZK_DATA_ROOT_HASH];
		let signature_root_hash = self.static_hashes[Self::SIGNATURE_ROOT_HASH];

		// calculate signing root hash (from data root hashes)
		matches.push(basic_data_root_hash);
		matches.push(zk_data_root_hash);
		let signing_root_hash = Self::hash_of(basic_data_root_hash, zk_data_root_hash);

		// calculate document root hash (from signing and signature root hashes)
		matches.push(signing_root_hash);
		matches.push(signature_root_hash);
		let calculated_doc_root_hash = Self::hash_of(signing_root_hash, signature_root_hash);

		// check if calculate and given document root hashes are equivalent
		if calculated_doc_root_hash == doc_root {
			Some(matches)
		} else {
			None
		}
	}
}

/// Data needed to provide proofs during a mint.
#[derive(Encode, Debug, Decode, Default, Clone, PartialEq)]
pub struct MintInfo<Anchor, Hash> {
	/// Unique ID to an anchor document.
	pub anchor_id: Anchor,

	/// The three hashes [DataRoot, SignatureRoot, DocRoot] *MUST* be in this order.
	/// These are used to validate the respective branches of the merkle tree, and
	/// to generate the final document root hash.
	pub static_hashes: [Hash; 3],

	/// Each element of the list is a proof that a certain property of a
	/// document has the specified value.
	pub proofs: Vec<CompleteProof<Hash>>,
}
