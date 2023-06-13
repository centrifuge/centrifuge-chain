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
use cfg_primitives::Balance;
use cfg_types::tokens::CurrencyId;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

pub type UpgradeCentrifuge1020 = ();


mod currency_id {
	use cfg_types::{tokens as v1, tokens::CustomMetadata};
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::{pallet_prelude::OptionQuery, storage_alias, Twox64Concat};
	use frame_support::traits::OnRuntimeUpgrade;
	use frame_support::weights::Weight;
	use orml_traits::asset_registry::AssetMetadata;
	#[cfg(feature = "try-runtime")]
	use sp_std::vec::Vec;


	use super::*;
	use crate::VERSION;

	/// Migrate all the balances under `CurrencyId::KSM` and `CurrencyId::AUSD` to their respective
	/// new `CurrencyId::ForeignAsset(_)` ids.
	pub struct CurrencyIdRefactorMigration;

	// The old orml_asset_registry Metadata storage using v0::CustomMetadata
	#[storage_alias]
	type Metadata<T: orml_asset_registry::Config> = StorageMap<
		orml_asset_registry::Pallet<T>,
		Twox64Concat,
		CurrencyId,
		AssetMetadata<Balance, v0::CustomMetadata>,
		OptionQuery,
	>;

	impl OnRuntimeUpgrade for CurrencyIdRefactorMigration {
		fn on_runtime_upgrade() -> Weight {
			if VERSION.spec_version != 1028 {
				return Weight::zero();
			}

			// orml_asset_registry::Metadata::<Runtime>::translate(
			// 	|asset_id: CurrencyId, old_metadata: AssetMetadata<Balance, v0::CustomMetadata>| {
			// 		match asset_id {
			// 			CurrencyId::Staking(_) => None,
			// 			CurrencyId::Tranche(_, _) => Some(to_metadata_v1(
			// 				old_metadata,
			// 				v1::CrossChainTransferability::Connectors,
			// 			)),
			// 			_ => Some(to_metadata_v1(
			// 				old_metadata.clone(),
			// 				v1::CrossChainTransferability::Xcm(old_metadata.additional.xcm),
			// 			)),
			// 		}
			// 	},
			// );

			let n = orml_asset_registry::Metadata::<Runtime>::iter().count() as u64;
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(n, n)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			// use codec::Encode;
			//
			// let old_state: Vec<(CurrencyId, AssetMetadata<Balance, v0::CustomMetadata>)> =
			// 	Metadata::<Runtime>::iter().collect::<Vec<_>>();
			//
			// Ok(old_state.encode())

			todo!("nuno")
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(old_state_encoded: Vec<u8>) -> Result<(), &'static str> {
			use codec::Decode;

			use crate::OrmlAssetRegistry;
			//
			// let old_state = sp_std::vec::Vec::<(
			// 	CurrencyId,
			// 	AssetMetadata<Balance, v0::CustomMetadata>,
			// )>::decode(&mut old_state_encoded.as_ref())
			// 	.map_err(|_| "Error decoding pre-upgrade state")?;
			//
			// for (asset_id, old_metadata) in old_state {
			// 	let new_metadata = OrmlAssetRegistry::metadata(asset_id)
			// 		.ok_or_else(|| "New state lost the metadata of an asset")?;
			//
			// 	match asset_id {
			// 		CurrencyId::Tranche(_, _) => ensure!(new_metadata == to_metadata_v1(
			// 			old_metadata,
			// 			v1::CrossChainTransferability::Connectors,
			// 		), "The metadata of a tranche token wasn't just updated by setting `transferability` to `Connectors `"),
			// 		_ => ensure!(new_metadata == to_metadata_v1(
			// 			old_metadata.clone(),
			// 			v1::CrossChainTransferability::Xcm(old_metadata.additional.xcm),
			// 		), "The metadata of a NON tranche token wasn't just updated by setting `transferability` to `Xcm`"),
			// 	}
			// }

			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use cfg_primitives::TrancheId;
	use cfg_types::{tokens as before, tokens::StakingCurrency};
	use codec::Encode;
	use hex::FromHex;

	mod after {
		use cfg_primitives::{PoolId, TrancheId};
		use cfg_types::tokens::{ForeignAssetId, StakingCurrency};
		use codec::{Decode, Encode, MaxEncodedLen};
		use scale_info::TypeInfo;

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
			/// The Native token, representing AIR in Altair and CFG in
			/// Centrifuge.
			#[codec(index = 0)]
			Native,

			/// A Tranche token
			#[codec(index = 1)]
			Tranche(PoolId, TrancheId),

			/// A foreign asset
			#[codec(index = 4)]
			ForeignAsset(ForeignAssetId),

			/// A staking token
			#[codec(index = 5)]
			Staking(StakingCurrency),
		}
	}

	#[test]
	fn encode_equality() {
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