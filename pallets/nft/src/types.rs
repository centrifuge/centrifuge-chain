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

//! Types used by non-fungible token (NFT) pallet


// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

use codec::{
    Decode, 
    Encode,
};

use sp_core::{
    H256, 
    keccak_256, 
    blake2_256
};

use sp_runtime::RuntimeDebug;

use sp_std::vec::Vec;

// Library for building and validating proofs
use proofs::{
    Hasher,
    Proof,
    Verifier,
    hashing::sort_hash_of
};

use unique_assets::traits::Nft;


// ----------------------------------------------------------------------------
// Types definition
// ----------------------------------------------------------------------------

// Generic definition of a non-fungible token (NFT), as used in this pallet
#[derive(Encode, Decode, Default, Clone, RuntimeDebug)]
pub struct Asset<Hash, AssetInfo> {
    pub id: Hash,
    pub asset: AssetInfo,
}

impl<AssetId, AssetInfo> Nft for Asset<AssetId, AssetInfo> {
    type Id = AssetId;
    type Info = AssetInfo;
}

/// Proof verifier data structure.
pub struct ProofVerifier {

    /// Array containing static root hashes passed when minting a non-fungible token.
    ///
    /// See [ProofVerifier::new] for information on how to pass these hashes. Those
    /// root hashes are passed when invoking [mint] transaction (or extrinsic).
    static_hashes: [<Self as Hasher>::Hash; 3],
}

// Proof verifier implementation block
impl ProofVerifier {

    const BASIC_DATA_ROOT_HASH: usize = 0;
    const ZK_DATA_ROOT_HASH: usize = 1;
    const SIGNATURE_ROOT_HASH: usize = 2;

    /// Build a new proof verifier instance, given a list of static root hashes.
    ///
    /// The 'root_hashes' must be passed in a specific order, namely:
    ///   1. The basic data root hash (with index ['BASIC_DATA_ROOT_HASH'])
    ///   2. The ZK root hash (see index ['ZK_DATA_ROOT_HASH'])
    ///   3. The signature root hash (see index ['SIGNATURE_DATA_ROOT_HASH'])
    pub fn new(static_hashes: [<Self as Hasher>::Hash; 3]) -> Self {
        ProofVerifier {
            static_hashes,
        }
    }
}

// Implement hasher trait for the registry's proof verifier
impl Hasher for ProofVerifier{
    type Hash = H256;

    fn hash(data: &[u8]) -> [u8; 32] {
        blake2_256(data)
    }
}

// Implement verifier trait for registry's proof verifier
impl Verifier for ProofVerifier {

    fn hash_of(a: Self::Hash, b: Self::Hash) -> Self::Hash {
        sort_hash_of::<Self>(a, b)
    }

    // Initial matches calculation.
    fn initial_matches(&self, doc_root: Self::Hash) -> Option<Vec<Self::Hash>> {
// TODO: be sure it is okay what to pass here
        Some(vec![doc_root])    
    }
}

/// Bundle hasher structure.
pub struct BundleHasher;


// Implement the proofs hasher trait for this bundle hasher.
impl Hasher for BundleHasher {
    
	type Hash = H256;

	fn hash(data: &[u8]) -> [u8; 32] {
		keccak_256(data)
	}
}

// Implement the NFT pallet's bundle hasher functions.
impl BundleHasher {

    /// Returns a bundled hash from a list of proofs
    pub fn get_bundled_hash_from_proofs(proofs: Vec<Proof<<Self as Hasher>::Hash>>, deposit_address: [u8; 20]) -> <Self as Hasher>::Hash {
        // extract (leaf) hashes from proofs
        let hashes = proofs.iter().map(|proof| proof.leaf_hash).collect();

        // compute the resulting bundled hash
        proofs::hashing::bundled_hash::<Self>(hashes, deposit_address)
    }
}
