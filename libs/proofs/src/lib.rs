//! # Optimized Merkle proof verifier and bundle hasher
//!
//! This pallet provides functionality of verifying optimized merkle proofs and
//! bundle hasher.

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::{fmt::Debug, vec::Vec};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock;

/// Deposit address
pub type DepositAddress = [u8; 20];

#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Proof<Hash> {
	pub leaf_hash: Hash,
	pub sorted_hashes: Vec<Hash>,
}

impl<Hash> Proof<Hash> {
	pub fn new(hash: Hash, sorted_hashes: Vec<Hash>) -> Self {
		Self {
			leaf_hash: hash,
			sorted_hashes,
		}
	}

	pub fn len(&self) -> usize {
		self.sorted_hashes.len()
	}

	pub fn is_empty(&self) -> bool {
		self.sorted_hashes.is_empty()
	}
}

pub trait Hasher: Sized {
	/// Hash type we deal with
	type Hash: Default + AsRef<[u8]> + Copy + PartialEq + PartialOrd + Debug;

	/// Hashes given data to 32 byte u8. Ex: blake256, Keccak
	fn hash(data: &[u8]) -> Self::Hash;
}

pub trait Verifier: Hasher {
	/// Computes hash of the a + b using `hash` function
	fn hash_of(a: Self::Hash, b: Self::Hash) -> Self::Hash;

	/// Returns the initial set of hashes to verify proofs.
	/// `None` implies a failed proof verification
	fn initial_matches(&self, doc_root: Self::Hash) -> Option<Vec<Self::Hash>>;

	/// Verifies each proof and return true if all the proofs are valid else
	/// returns false
	fn verify_proofs(&self, doc_root: Self::Hash, proofs: &[Proof<Self::Hash>]) -> bool {
		if proofs.is_empty() {
			return false;
		}

		let mut matches = match Self::initial_matches(self, doc_root) {
			Some(matches) => matches,
			None => return false,
		};

		proofs
			.iter()
			.map(|proof| inner::verify_proof::<Self>(&mut matches, proof))
			.all(|b| b)
	}

	/// Verifies the proof and returns true if valid
	fn verify_proof(&self, doc_root: Self::Hash, proof: &Proof<Self::Hash>) -> bool {
		let mut matches = match Self::initial_matches(self, doc_root) {
			Some(matches) => matches,
			None => return false,
		};

		inner::verify_proof::<Self>(&mut matches, proof)
	}
}
mod inner {
	use super::*;
	use crate::{Proof, Verifier};

	/// This is an optimized Merkle proof checker. It caches all valid leaves in
	/// an array called matches. If a proof is validated, all the intermediate
	/// hashes will be added to the array. When validating a subsequent proof,
	/// that proof will stop being validated as soon as a hash has been computed
	/// that has been a computed hash in a previously validated proof.
	///
	/// When submitting a list of proofs, the client can thus choose to chop of
	/// all the already proven nodes when submitting multiple proofs.
	///
	/// matches: matches will have a pre computed hashes provided by the client
	/// and document root of the reference anchor. static proofs are used to
	/// computed the pre computed hashes and the result is checked against
	/// document root provided.
	pub fn verify_proof<V: Verifier>(matches: &mut Vec<V::Hash>, proof: &Proof<V::Hash>) -> bool {
		let Proof {
			leaf_hash,
			sorted_hashes,
		} = proof.clone();

		// if leaf_hash is already cached/computed earlier
		if matches.contains(&leaf_hash) {
			return true;
		}

		let mut hash = leaf_hash;
		for proof in sorted_hashes {
			matches.push(proof);
			hash = V::hash_of(hash, proof);
			if matches.contains(&hash) {
				return true;
			}
			matches.push(hash);
		}

		false
	}
}

pub mod hashing {
	use sp_std::vec::Vec;

	use crate::{DepositAddress, Hasher, Proof};

	/// computes sorted hash of the a and b
	/// if a < b: hash(a+b)
	/// else: hash(b+a)
	pub fn sort_hash_of<H: Hasher>(a: H::Hash, b: H::Hash) -> H::Hash {
		if a < b {
			return hash_of::<H>(a, b);
		}

		hash_of::<H>(b, a)
	}

	/// computes hash of the a + b
	pub fn hash_of<H: Hasher>(a: H::Hash, b: H::Hash) -> H::Hash {
		let data = [a.as_ref(), b.as_ref()].concat();
		H::hash(&data)
	}

	/// Return a bundled hash from a list of hashes.
	///
	/// This function appends `deposit_address` and all the given `hashes` from
	/// the proofs and returns the result hash
	pub fn bundled_hash<H: Hasher>(
		hashes: Vec<H::Hash>,
		deposit_address: DepositAddress,
	) -> H::Hash {
		let data = hashes
			.into_iter()
			.fold(deposit_address.to_vec(), |acc, hash| {
				[acc.as_slice(), hash.as_ref()].concat()
			});
		H::hash(data.as_slice())
	}

	/// Return a bundled hash from a list of proofs.
	///
	/// Same as [`bundled_hash()`] function, but here a list of proofs is given.
	/// This function the appends `deposit_address`
	/// and all leaf hashes from given `proofs` and returns a bundled hash.
	pub fn bundled_hash_from_proofs<H: Hasher>(
		proofs: Vec<Proof<H::Hash>>,
		deposit_address: DepositAddress,
	) -> H::Hash {
		// Extract (leaf) hashes from proofs
		let hashes = proofs.iter().map(|proof| proof.leaf_hash).collect();

		// Compute the resulting bundled hash
		bundled_hash::<H>(hashes, deposit_address)
	}
}
