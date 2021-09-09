#![cfg_attr(not(feature = "std"), no_std)]

pub use apis::*;
pub use constants::*;
pub use impls::*;
pub use traits::*;
pub use types::*;

mod impls;

pub mod apis {
	use node_primitives::{BlockNumber, Hash};
	use pallet_anchors::AnchorData;
	use sp_api::decl_runtime_apis;

	decl_runtime_apis! {
		/// The API to query anchoring info.
		pub trait AnchorApi {
			fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>>;
		}
	}
}

/// Common types for all runtimes
pub mod types {
	use sp_core::{H160, U256};
	use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, Verify};
	use sp_std::vec::Vec;

	/// An index to a block.
	pub type BlockNumber = u32;

	/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
	pub type Signature = sp_runtime::MultiSignature;

	/// Some way of identifying an account on the chain. We intentionally make it equivalent
	/// to the public key of our transaction signing scheme.
	pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

	/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
	/// never know...
	pub type AccountIndex = u32;

	/// The address format for describing accounts.
	pub type Address = sp_runtime::MultiAddress<AccountId, ()>;

	/// Balance of an account.
	pub type Balance = u128;

	/// Index of a transaction in the chain.
	pub type Index = u32;

	/// A hash of some data used by the chain.
	pub type Hash = sp_core::H256;

	/// Block header type as expected by this runtime.
	pub type Header = sp_runtime::generic::Header<BlockNumber, BlakeTwo256>;

	/// Digest item type.
	pub type DigestItem = sp_runtime::generic::DigestItem<Hash>;

	/// Aura consensus authority.
	pub type AuraId = sp_consensus_aura::sr25519::AuthorityId;

	/// Moment type
	pub type Moment = u64;

	// A vector of bytes, conveniently named like it is in Solidity.
	pub type Bytes = Vec<u8>;

	// A sized vector of 32 bytes
	pub type Bytes32 = [u8; 32];

	// Registries are identified using a nonce in storage.
	pub type RegistryId = H160;

	// A cryptographic salt to be combined with a value before hashing.
	pub type Salt = [u8; 32];

	// The id of an asset as it corresponds to the "token id" of a Centrifuge document.
	// A registry id is needed as well to uniquely identify an asset on-chain.
	pub type TokenId = U256;
}

/// Common constants for all runtimes
pub mod constants {
	use super::types::BlockNumber;
	use frame_support::weights::{constants::WEIGHT_PER_SECOND, Weight};
	use node_primitives::Balance;
	use sp_runtime::Perbill;

	/// This determines the average expected block time that we are targeting. Blocks will be
	/// produced at a minimum duration defined by `SLOT_DURATION`. `SLOT_DURATION` is picked up by
	/// `pallet_timestamp` which is in turn picked up by `pallet_aura` to implement `fn
	/// slot_duration()`.
	///
	/// Change this to adjust the block time.
	pub const MILLISECS_PER_BLOCK: u64 = 12000;
	pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

	// Time is measured by number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;

	/// Milliseconds per day
	pub const MILLISECS_PER_DAY: u64 = 86400000;

	/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
	/// used to limit the maximal weight of a single extrinsic.
	pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);
	/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
	/// Operational  extrinsics.
	pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

	/// We allow for 0.5 seconds of compute with a 6 second average block time.
	pub const MAXIMUM_BLOCK_WEIGHT: Weight = WEIGHT_PER_SECOND / 2;

	pub const MICRO_CFG: Balance = 1_000_000_000_000; // 10−6 	0.000001
	pub const MILLI_CFG: Balance = 1_000 * MICRO_CFG; // 10−3 	0.001
	pub const CENTI_CFG: Balance = 10 * MILLI_CFG; // 10−2 	0.01
	pub const CFG: Balance = 100 * CENTI_CFG;

	/// Additional fee charged when moving native tokens to target chains (in CFGs).
	pub const NATIVE_TOKEN_TRANSFER_FEE: Balance = 2000 * CFG;

	/// Additional fee charged when moving NFTs to target chains (in CFGs).
	pub const NFT_TOKEN_TRANSFER_FEE: Balance = 20 * CFG;

	/// Additional fee charged when validating NFT proofs
	pub const NFT_PROOF_VALIDATION_FEE: Balance = 10 * CFG;

	// Represents the protobuf encoding - "NFTS". All Centrifuge documents are formatted in this way.
	/// These are pre/appended to the registry id before being set as a [RegistryInfo] field in [create_registry].
	pub const NFTS_PREFIX: &'static [u8] = &[1, 0, 0, 0, 0, 0, 0, 20];

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 15 * CENTI_CFG + (bytes as Balance) * 6 * CENTI_CFG
	}
}

pub mod traits {

	use super::impls::AssetId;

	/// An implementor of this trait *MUST* be an asset of a registry.
	/// The registry id that an asset is a member of can be determined
	/// when this trait is implemented.
	pub trait InRegistry {
		type RegistryId;

		/// Returns the registry id that the self is a member of.
		fn registry_id(&self) -> Self::RegistryId;
	}

	/// An implementor has an associated asset id that will be used as a
	/// unique id within a registry for an asset. Asset ids *MUST* be unique
	/// within a registry. Corresponds to a token id in a Centrifuge document.
	pub trait HasId {
		/// Returns unique asset id.
		fn id(&self) -> &AssetId;
	}
}
