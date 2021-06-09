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

//! Centrifuge Chain Primitives
//!
//! ## Overview
//! This library implements basic building blocks used by the Centrifuge
//! chain project. Among other, it provides with common traits for assets
//! manipulation (whether fungible or not) and proofs.
//!
//! ## Credits
//! The Centrifugians Tribe <tribe@centrifuge.io>
//!
//! ## License
//! GNU General Public License, Version 3, 29 June 2007 <https://www.gnu.org/licenses/gpl-3.0.html>

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{
    Decode, 
    Encode,
};

use sp_core::{U256, H160};

use frame_support::dispatch::DispatchError;

use sp_std::{
    convert::TryInto,
    fmt::Debug,
    vec::Vec, 
};

// Import proof module
mod proofs;

// Re-export proofs
pub use proofs::*;


/// Represents the protobuf encoding - "NFTS". All Centrifuge documents are formatted in this way.
/// These are pre/appended to the registry id before being set as a [RegistryInfo] field in [create_registry].
pub const NFTS_PREFIX: &'static [u8] = &[1, 0, 0, 0, 0, 0, 0, 20];

/// A vector of bytes, conveniently named like it is in Solidity.
pub type Bytes = Vec<u8>;

/// Registries are identified using a nonce in storage.
pub type RegistryId = H160;

/// A cryptographic salt to be combined with a value before hashing.
pub type Salt = [u8; 32];

/// The id of an asset as it corresponds to the "token id" of a Centrifuge document.
/// A registry id is needed as well to uniquely identify an asset on-chain.
pub type TokenId = U256;

/// A global identifier for an nft/asset on-chain. Composed of a registry and token id.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct AssetId(pub RegistryId, pub TokenId);

/// Holds references to its component parts.
pub struct AssetIdRef<'a>(pub &'a RegistryId, pub &'a TokenId);

impl AssetId {
    pub fn destruct(self) -> (RegistryId, TokenId) {
        (self.0, self.1)
    }
}

impl<'a> From<&'a AssetId> for AssetIdRef<'a> {
    fn from(id: &'a AssetId) -> Self {
        AssetIdRef(&id.0, &id.1)
    }
}

impl<'a> AssetIdRef<'a> {
    pub fn destruct(self) -> (&'a RegistryId, &'a TokenId) {
        (self.0, self.1)
    }
}

/* TODO: implement in Bridge
impl From<bridge::Address> for RegistryId {
    fn from(a: bridge::Address) -> Self {
        H160::from_slice(&a.0[..20])
    }
}
*/

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

/// All data for an instance of an NFT.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct AssetInfo {
    pub metadata: Bytes,
}

/// A complete proof that a value for a given property of a document is the real value.
/// Proven by hashing hash(value + property + salt) into a leaf hash of the document
/// merkle tree, then hashing with the given hashes to generate the merkle root.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct Proof<Hash> {
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
impl From<Proof<sp_core::H256>> for proofs::Proof {
    fn from(mut p: Proof<sp_core::H256>) -> Self {
        // Generate leaf hash from property ++ value ++ salt
        p.property.extend(p.value);
        p.property.extend(&p.salt);
        let leaf_hash = sp_io::hashing::keccak_256(&p.property).into();

        proofs::Proof::new(leaf_hash, p.hashes)
    }
}

/// Data needed to provide proofs during a mint.
#[derive(Encode, Decode, Clone, PartialEq, Default, Debug)]
pub struct MintInfo<T, Hash> {
    /// Unique ID to an anchor document.
    pub anchor_id: T,
    /// The three hashes [DataRoot, SignatureRoot, DocRoot] *MUST* be in this order.
    /// These are used to validate the respective branches of the merkle tree, and
    /// to generate the final document root hash.
    pub static_hashes: [Hash; 3],
    /// Each element of the list is a proof that a certain property of a
    /// document has the specified value.
    pub proofs: Vec<Proof<Hash>>,
}

/// An implementor of this trait *MUST* be an asset of a registry.
/// The registry id that an asset is a member of can be determined
/// when this trait is implemented.
pub trait InRegistry {
    /// Returns the registry id that the self is a member of.
    fn registry_id(&self) -> RegistryId;
}

/// An implementor has an associated asset id that will be used as a
/// unique id within a registry for an asset. Asset ids *MUST* be unique
/// within a registry. Corresponds to a token id in a Centrifuge document.
pub trait HasId {
    /// Returns unique asset id.
    fn id(&self) -> &AssetId;
}

/// A general interface for registries that require some sort of verification to mint their
/// underlying NFTs. A substrate module can implement this trait.
pub trait VerifierRegistry {
    /// This should typically match the implementing substrate Module trait's AccountId type.
    type AccountId;

    /// The id type of a registry.
    type RegistryId;

    /// Metadata for an instance of a registry.
    type RegistryInfo;
    
    /// The id type of an NFT.
    type AssetId;
    
    /// The data that defines the NFT held by a registry. Asset info must contain its
    /// associated registry id.
    type AssetInfo;
    
    /// All data necessary to determine if a requested mint is valid or not.
    type MintInfo;

    /// Create a new instance of a registry with the associated registry info.
    fn create_registry(caller: Self::AccountId, info: Self::RegistryInfo) -> Result<Self::RegistryId, DispatchError>;

    /// Use the mint info to verify whether the mint is a valid action.
    /// If so, use the asset info to mint an asset.
    fn mint(caller: &Self::AccountId,
            owner_account: &Self::AccountId,
            asset_id: &Self::AssetId,
            asset_info: Self::AssetInfo,
            mint_info: Self::MintInfo,
    ) -> Result<(), DispatchError>;
}


// --- Extracted from Bridge ---

/// A generic representation of a local address. A resource id points to this. It may be a
/// registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just
/// as an upper bound to store efficiently.
#[derive(Encode, Clone, PartialEq, Eq, Default, Debug)]
pub struct Address(pub Bytes32);

/// Length of an [Address] type
const ADDR_LEN: usize = 32;

type Bytes32 = [u8; ADDR_LEN];

impl From<RegistryId> for Address {
    fn from(r: RegistryId) -> Self {
        // Pad 12 bytes to the registry id - total 32 bytes
        let padded = r.to_fixed_bytes().iter().copied()
            .chain([0; 12].iter().copied()).collect::<Vec<u8>>()[..ADDR_LEN]
            .try_into().expect("RegistryId is 20 bytes. 12 are padded. Converting to a 32 byte array should never fail");

        Address( padded )
    }
}

// In order to be generic into T::Address
impl From<Bytes32> for Address {
    fn from(v: Bytes32) -> Self {
        Address( v[..ADDR_LEN].try_into().expect("Address wraps a 32 byte array") )
    }
}

impl From<Address> for Bytes32 {
    fn from(a: Address) -> Self {
        a.0
    }
}