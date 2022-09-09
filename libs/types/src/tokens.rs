use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use cfg_primitives::types::{PoolId, TrancheId};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	// The Native token, representing AIR in Altair and CFG in Centrifuge.
	Native,

	// A Tranche token
	Tranche(PoolId, TrancheId),

	/// Karura KSM
	KSM,

	/// Acala Dollar
	/// In Altair, it represents AUSD in Kusama;
	/// In Centrifuge, it represents AUSD in Polkadot;
	AUSD,

	/// A foreign asset
	ForeignAsset(ForeignAssetId),
}

pub type ForeignAssetId = u32;

impl Default for CurrencyId {
	fn default() -> Self {
		CurrencyId::Native
	}
}

/// A type that can create a TrancheToken from a PoolId and a TrancheId
pub struct TrancheToken;

impl cfg_traits::TrancheToken<PoolId, TrancheId, CurrencyId> for TrancheToken {
	fn tranche_token(pool: PoolId, tranche: TrancheId) -> CurrencyId {
		CurrencyId::Tranche(pool, tranche)
	}
}

/// A type describing our custom additional metadata stored in the OrmlAssetRegistry.
#[derive(
	Clone,
	Copy,
	Default,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct CustomMetadata {
	/// XCM-related metadata.
	pub xcm: super::XcmMetadata,

	/// Whether an asset can be minted.
	/// When `true`, the right permissions will checked in the permissions
	/// pallet to authorize asset minting by an origin.
	pub mintable: bool,

	/// Whether an asset is _permissioned_, i.e., whether the asset can only
	/// be transferred from and to whitelisted accounts. When `true`, the
	/// right permissions will checked in the permissions pallet to authorize
	/// transfer between mutually allowed from and to accounts.
	pub permissioned: bool,

	/// Whether an asset can be used as a currency to fund Centrifuge Pools.
	pub pool_currency: bool,
}
