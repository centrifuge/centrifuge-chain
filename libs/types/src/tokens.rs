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

use crate::{xcm::XcmMetadata, EVMChainId};

/// The type for all Currency ids that our chains handles.
/// Foreign assets gather all the tokens that are native to other chains, such
/// as DOT, AUSD, UDST, etc. NOTE: We MUST never change the `#[codec(index =
/// _)]`  below as doing so results in corrupted storage keys; if changing the
/// index value of a variant is mandatory, a storage migration must take place
/// to ensure that the values under an old codec-encoded key are moved to the
/// new key.
///
/// NOTE: It is vital to leave the `#[codec(index = _)]` marks immutable or else storage items under a `CurrencyId` key will be corrupted.
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
	#[codec(index = 0)]
	Native,

	/// A Tranche token
	#[codec(index = 1)]
	Tranche(PoolId, TrancheId),

	/// A foreign asset
	#[codec(index = 4)]
	ForeignAsset(ForeignAssetId),

	/// A staking currency
	#[codec(index = 5)]
	Staking(StakingCurrency),
}

#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum StakingCurrency {
	/// An emulated internal, non-transferable currency
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
	/// The ways, if any, this token is cross-chain transferable
	pub transferability: CrossChainTransferability,

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

/// The Cross Chain Transferability property of an asset describes the way(s),
/// if any, that said asset is cross-chain transferable. It may currently be
/// transferable through Xcm, Centrifuge Connectors, or All .
///
/// NOTE: Once set to `All`, the asset is automatically transferable through any
/// eventual new option added at a later stage. A migration might be required if
/// that's undesirable for any registered asset.
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
pub enum CrossChainTransferability {
	/// The asset is not transferable cross-chain
	#[default]
	None,

	/// The asset is only transferable through XCM
	Xcm(XcmMetadata),

	/// The asset is only transferable through Centrifuge Connectors
	Connectors,

	/// The asset is transferable through all available options
	All(XcmMetadata),
}

impl CrossChainTransferability {
	pub fn includes_xcm(self) -> bool {
		matches!(self, Self::Xcm(..) | Self::All(..))
	}
}

/// Connectors-wrapped tokens
///
/// Currently, Connectors are only deployed on EVM-based chains and therefore
/// we only support EVM tokens. In the far future, we might support wrapped
/// tokens from other chains such as Cosmos based ones.
#[derive(
	Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum ConnectorsWrappedToken {
	/// An EVM-native token
	EVM {
		/// The EVM chain id where the token is deployed
		chain_id: EVMChainId,
		/// The token contract address
		address: [u8; 20],
	},
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

	#[cfg(test)]
	mod tests {
		use cfg_primitives::TrancheId;
		use crate::tokens as after;
		use super::{ForeignAssetId, StakingCurrency};
		use codec::Encode;
		use hex::FromHex;

		mod before {
			use cfg_primitives::{PoolId, TrancheId};
			use codec::{Decode, Encode, MaxEncodedLen};
			use scale_info::TypeInfo;
			use crate::tokens::{ForeignAssetId, StakingCurrency};

			/// The old definition of `CurrencyId` which included `AUSD` and `KSM` as hardcoded variants.
			#[derive(
			Clone,
			Copy,
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
			pub enum CurrencyId {
				// The Native token, representing AIR in Altair and CFG in Centrifuge.
				#[codec(index = 0)]
				Native,

				/// A Tranche token
				#[codec(index = 1)]
				Tranche(PoolId, TrancheId),

				/// Karura KSM
				#[codec(index = 2)]
				KSM,

				/// Acala Dollar
				/// In Altair, it represents AUSD in Kusama;
				/// In Centrifuge, it represents AUSD in Polkadot;
				#[codec(index = 3)]
				AUSD,

				/// A foreign asset
				#[codec(index = 4)]
				ForeignAsset(ForeignAssetId),

				/// A staking currency
				#[codec(index = 5)]
				Staking(StakingCurrency),
			}
		}

		#[test]
		fn currency_id_refactor_encode_equality() {
			// Native
			assert_eq!(
				before::CurrencyId::Native.encode(),
				after::CurrencyId::Native.encode()
			);
			assert_eq!(after::CurrencyId::Native.encode(), vec![0]);

			// Tranche
			assert_eq!(
				before::CurrencyId::Tranche(33, default_tranche_id()).encode(),
				after::CurrencyId::Tranche(33, default_tranche_id()).encode()
			);
			assert_eq!(
				after::CurrencyId::Tranche(33, default_tranche_id()).encode(),
				vec![
					1, 33, 0, 0, 0, 0, 0, 0, 0, 129, 26, 205, 91, 63, 23, 192, 104, 65, 199, 228, 30,
					158, 4, 203, 27
				]
			);

			// KSM - deprecated
			assert_eq!(before::CurrencyId::KSM.encode(), vec![2]);

			// AUSD - deprecated
			assert_eq!(before::CurrencyId::AUSD.encode(), vec![3]);

			// ForeignAsset
			assert_eq!(
				before::CurrencyId::ForeignAsset(91).encode(),
				after::CurrencyId::ForeignAsset(91).encode()
			);
			assert_eq!(
				after::CurrencyId::ForeignAsset(91).encode(),
				vec![4, 91, 0, 0, 0]
			);

			// Staking
			assert_eq!(
				before::CurrencyId::Staking(StakingCurrency::BlockRewards).encode(),
				after::CurrencyId::Staking(StakingCurrency::BlockRewards).encode()
			);
			assert_eq!(
				after::CurrencyId::Staking(StakingCurrency::BlockRewards).encode(),
				vec![5, 0]
			);
		}

		fn default_tranche_id() -> TrancheId {
			<[u8; 16]>::from_hex("811acd5b3f17c06841c7e41e9e04cb1b")
				.expect("Should be valid tranche id")
		}
	}
}
