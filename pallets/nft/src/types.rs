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

use codec::{Decode, Encode};

use sp_runtime::{sp_std::vec::Vec, traits::Hash};

//use sp_std::vec::Vec;

// Library for building and validating proofs
use proofs::{hashing::sort_hash_of, Hasher, Verifier};

// ----------------------------------------------------------------------------
// Types definition
// ----------------------------------------------------------------------------

/// A global identifier for an nft/asset on-chain. Composed of a registry and token id.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct AssetId<RegistryId, TokenId>(pub RegistryId, pub TokenId);

impl<RegistryId, TokenId> AssetId<RegistryId, TokenId> {
	pub fn destruct(self) -> (RegistryId, TokenId) {
		(self.0, self.1)
	}
}

/// Proof verifier data structure.
pub(crate) struct ProofVerifier<T>(sp_std::marker::PhantomData<T>);

// Proof verifier implementation block
impl<T: frame_system::Config> ProofVerifier<T> {
	pub fn new() -> Self {
		ProofVerifier(sp_std::marker::PhantomData)
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
	fn hash_of(a: Self::Hash, b: Self::Hash) -> Self::Hash {
		sort_hash_of::<Self>(a, b)
	}

	// Initial matches calculation.
	fn initial_matches(&self, doc_root: Self::Hash) -> Option<Vec<Self::Hash>> {
		// TODO: be sure it is okay what to pass here
		Some(vec![doc_root])
	}
}
