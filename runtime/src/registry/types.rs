//pub use crate::nft::AssetId;
use frame_support::dispatch;
use codec::{Decode, Encode};
use sp_std::{vec::Vec, fmt::Debug};
use sp_core::U256;
use crate::proofs;

/// A vector of bytes, conveniently named like it is in Solidity.
pub type Bytes = Vec<u8>;

/// Registries are identified using a nonce in storage.
pub type RegistryId = U256;

/// A cryptographic salt to be combined with a value before hashing.
pub type Salt = Bytes;

/// An arbitrary byte string chosen by the minter of an asset.
pub type AssetId = U256;

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
    pub registry_id: RegistryId,
    pub asset_id: AssetId,
    // metadata: scale encoded
}

/// Registry id must be a field within the data of an asset because
/// its registry is part of its uniquely identifying information.
impl InRegistry for AssetInfo {
    fn registry_id(&self) -> RegistryId {
        self.registry_id
    }
}

/// Asset data must contain an id for identifying documents on and off-chain.
impl HasId for AssetInfo {
    fn id(&self) -> &AssetId {
        &self.asset_id
    }
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
        p.property.extend(p.salt);
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
