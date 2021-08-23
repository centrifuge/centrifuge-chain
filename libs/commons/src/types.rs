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

//! Common types definition.


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use codec::{
    Decode, 
    Encode
};

use sp_core::{
    H160,
    U256,
};


// ----------------------------------------------------------------------------
// Types definition
// ----------------------------------------------------------------------------

// A vector of bytes, conveniently named like it is in Solidity.
pub type Bytes = Vec<u8>;

// Registries are identified using a nonce in storage.
pub type RegistryId = H160;

// A cryptographic salt to be combined with a value before hashing.
pub type Salt = [u8; 32];

// The id of an asset as it corresponds to the "token id" of a Centrifuge document.
// A registry id is needed as well to uniquely identify an asset on-chain.
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

/// All data for an instance of an NFT.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct AssetInfo {
    pub metadata: Bytes,
}