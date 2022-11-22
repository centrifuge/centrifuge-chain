// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::types::{PoolId, TrancheId};
use cfg_traits::TrancheCurrency as TrancheCurrencyT;
use codec::{Decode, Encode, MaxEncodedLen};
pub use orml_asset_registry::AssetMetadata;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::xcm::XcmMetadata;

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

// A way to generate different currencies from a number.
// Can be used in tests/benchmarks to generate different currencies.
impl From<u32> for CurrencyId {
	fn from(value: u32) -> Self {
		CurrencyId::ForeignAsset(value)
	}
}

/// A Currency that is solely used by tranches.
///
/// We distinguish here between the enum variant CurrencyId::Tranche(PoolId, TranchId)
/// in order to be able to have a clear separation of concerns. This enables us
/// to use the `TrancheCurrency` type separately where solely this enum variant would be
/// relevant. Most notably, in the `struct Tranche`.
#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TrancheCurrency {
	pub(crate) pool_id: PoolId,
	pub(crate) tranche_id: TrancheId,
}

impl From<TrancheCurrency> for CurrencyId {
	fn from(x: TrancheCurrency) -> Self {
		CurrencyId::Tranche(x.pool_id, x.tranche_id)
	}
}

impl TrancheCurrencyT<PoolId, TrancheId> for TrancheCurrency {
	fn generate(pool_id: PoolId, tranche_id: TrancheId) -> Self {
		Self {
			pool_id,
			tranche_id,
		}
	}

	fn of_pool(&self) -> PoolId {
		self.pool_id
	}

	fn of_tranche(&self) -> TrancheId {
		self.tranche_id
	}
}

/// A type describing our custom additional metadata stored in the OrmlAssetRegistry.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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
	pub xcm: XcmMetadata,

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
