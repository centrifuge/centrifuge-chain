use crate::proofs::Proof;
use crate::nft::{InRegistry, CommodityId};
use frame_support::dispatch;
use codec::{Decode, Encode};
use sp_std::{vec::Vec, fmt::Debug};

// Registries are identified using a nonce in storage
pub type RegistryId = u128;

// A vector of bytes, conveniently named like it is in Solidity
pub type bytes = Vec<u8>;

// A convenience rename from pallet_nft's id type
pub type AssetId<T> = CommodityId<T>;

// Metadata for a registry instance
#[derive(Encode, Decode, Clone, PartialEq, Default, Debug)]
/// Metadata for an instance of a registry.
pub struct RegistryInfo {
    /// A configuration option that will enable a user to burn their own tokens
    /// in the [burn] method.
    pub owner_can_burn: bool,
    /// Names of fields required to be provided for verification during a [mint].
    /// These *MUST* be compact encoded.
    pub fields: Vec<bytes>,
}

/// All data for an instance of an NFT.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
pub struct AssetInfo {
    pub registry_id: RegistryId,
    // TODO: Other generic fields ..
}

// Registry id must be a field within the data, because an assets id
// is a hash of its content, and its registry is part of its uniquely
// identifying information.
impl InRegistry for AssetInfo {
    fn registry_id(&self) -> RegistryId {
        self.registry_id
    }
}

/// Data needed to provide proofs during a mint.
#[derive(Encode, Decode, Clone, PartialEq, Default, Debug)]
pub struct MintInfo<Hash> {
    /// Unique ID to an anchor document.
    pub anchor_id: Hash,
    /// Proofs should match to corresponding values. A value-leaf-hash
    /// merkelized with its proof will be the root hash of the anchor
    /// document when valid.
    pub proofs: Vec<Proof>,
    /// Values correspond with fields specified by a registry.
    pub values: Vec<bytes>,
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
    type AssetInfo: InRegistry;
    /// All data necessary to determine if a requested mint is valid or not.
    type MintInfo;

    /// Create a new instance of a registry with the associated registry info.
    fn create_registry(info: &Self::RegistryInfo) -> Result<Self::RegistryId, dispatch::DispatchError>;

    /// Use the mint info to verify whether the mint is a valid action.
    /// If so, use the asset info to mint an asset.
    fn mint(owner_account: Self::AccountId,
            asset_info: Self::AssetInfo,
            mint_info: Self::MintInfo,
    ) -> Result<Self::AssetId, dispatch::DispatchError>;
}
