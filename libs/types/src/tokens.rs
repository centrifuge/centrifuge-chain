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

use core::marker::PhantomData;

use cfg_primitives::types::{PoolId, TrancheId};
use cfg_traits::TrancheCurrency as TrancheCurrencyT;
use codec::{Decode, Encode, MaxEncodedLen};
pub use orml_asset_registry::AssetMetadata;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Get, DispatchError, TokenError};

use crate::xcm::XcmMetadata;

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
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	// The Native token, representing AIR in Altair and CFG in Centrifuge.
	#[default]
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

	/// A staking token
	Staking(StakingCurrency),
}

#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum StakingCurrency {
	/// An emulated internal, non-transferrable currency
	/// Its issuance and holding is handled inherently
	BlockRewards,
}

pub type ForeignAssetId = u32;

// A way to generate different currencies from a number.
// Can be used in tests/benchmarks to generate different currencies.
impl From<u32> for CurrencyId {
	fn from(value: u32) -> Self {
		CurrencyId::ForeignAsset(value)
	}
}

// A shortcut anchoring the assumption made about `StakingCurrency`.
impl From<StakingCurrency> for CurrencyId {
	fn from(inner: StakingCurrency) -> Self {
		CurrencyId::Staking(inner)
	}
}

/// A general index wrapper for a given currency representation which is the
/// concatenation of the generic prefix and the identifier of the respective
/// currency.
pub struct GeneralCurrencyIndex<Index, Prefix> {
	pub index: Index,
	_phantom: PhantomData<Prefix>,
}

impl<Index, Prefix> TryInto<GeneralCurrencyIndex<Index, Prefix>> for CurrencyId
where
	Index: From<u128>,
	Prefix: Get<[u8; 12]>,
{
	type Error = DispatchError;

	fn try_into(self) -> Result<GeneralCurrencyIndex<Index, Prefix>, Self::Error> {
		let mut bytes = [0u8; 16];
		bytes[..12].copy_from_slice(&Prefix::get());

		let currency_bytes: [u8; 4] = match &self {
			CurrencyId::ForeignAsset(id32) => Ok(id32.to_be_bytes()),
			_ => Err(DispatchError::Token(TokenError::Unsupported)),
		}?;
		bytes[12..].copy_from_slice(&currency_bytes[..]);

		Ok(GeneralCurrencyIndex {
			index: u128::from_be_bytes(bytes).into(),
			_phantom: Default::default(),
		})
	}
}

impl<Index, Prefix> TryFrom<GeneralCurrencyIndex<Index, Prefix>> for CurrencyId
where
	Index: Into<u128>,
	Prefix: Get<[u8; 12]>,
{
	type Error = DispatchError;

	fn try_from(value: GeneralCurrencyIndex<Index, Prefix>) -> Result<Self, Self::Error> {
		let bytes: [u8; 16] = value.index.into().to_be_bytes();
		let currency_bytes: [u8; 4] = bytes[12..]
			.try_into()
			// should never throw but lets be safe
			.map_err(|_| DispatchError::Corruption)?;

		Ok(CurrencyId::ForeignAsset(u32::from_be_bytes(currency_bytes)))
	}
}

impl<Index, Prefix> From<u128> for GeneralCurrencyIndex<Index, Prefix>
where
	Index: From<u128>,
	Prefix: Get<[u8; 12]>,
{
	fn from(value: u128) -> Self {
		GeneralCurrencyIndex {
			index: value.into(),
			_phantom: Default::default(),
		}
	}
}

/// A Currency that is solely used by tranches.
///
/// We distinguish here between the enum variant CurrencyId::Tranche(PoolId,
/// TranchId) in order to be able to have a clear separation of concerns. This
/// enables us to use the `TrancheCurrency` type separately where solely this
/// enum variant would be relevant. Most notably, in the `struct Tranche`.
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

/// A type describing our custom additional metadata stored in the
/// OrmlAssetRegistry.
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

impl cfg_traits::CurrencyInspect for CurrencyId {
	type CurrencyId = CurrencyId;

	fn is_tranche_token(currency: Self::CurrencyId) -> bool {
		matches!(currency, CurrencyId::Tranche(_, _))
	}
}

#[cfg(test)]
mod tests {
	use frame_support::parameter_types;

	use super::*;

	const FOREIGN: CurrencyId = CurrencyId::ForeignAsset(1u32);

	parameter_types! {
		pub const ZeroPrefix: [u8; 12] = [0u8; 12];
		pub const NonZeroPrefix: [u8; 12] = *b"TestPrefix12";
	}

	#[test]
	fn zero_prefix_general_index_conversion() {
		let general_index: GeneralCurrencyIndex<u128, ZeroPrefix> = FOREIGN.try_into().unwrap();
		assert_eq!(general_index.index, 1u128);

		// check identity condition on reverse conversion
		let reconvert = CurrencyId::try_from(general_index).unwrap();
		assert_eq!(reconvert, CurrencyId::ForeignAsset(1u32));
	}

	#[test]
	fn non_zero_prefix_general_index_conversion() {
		let general_index: GeneralCurrencyIndex<u128, NonZeroPrefix> = FOREIGN.try_into().unwrap();
		assert_eq!(
			general_index.index,
			112181915321113319688489505016241979393u128
		);

		// check identity condition on reverse conversion
		let reconvert = CurrencyId::try_from(general_index).unwrap();
		assert_eq!(reconvert, CurrencyId::ForeignAsset(1u32));
	}

	#[test]
	fn non_foreign_asset_general_index_conversion() {
		assert!(
			TryInto::<GeneralCurrencyIndex<u128, ZeroPrefix>>::try_into(CurrencyId::Native)
				.is_err()
		);
		assert!(
			TryInto::<GeneralCurrencyIndex<u128, ZeroPrefix>>::try_into(CurrencyId::Tranche(
				2, [1u8; 16]
			))
			.is_err()
		);
		assert!(
			TryInto::<GeneralCurrencyIndex<u128, ZeroPrefix>>::try_into(CurrencyId::KSM).is_err()
		);
		assert!(
			TryInto::<GeneralCurrencyIndex<u128, ZeroPrefix>>::try_into(CurrencyId::AUSD).is_err()
		);
		assert!(
			TryInto::<GeneralCurrencyIndex<u128, ZeroPrefix>>::try_into(CurrencyId::Staking(
				StakingCurrency::BlockRewards
			))
			.is_err()
		);
	}
}
