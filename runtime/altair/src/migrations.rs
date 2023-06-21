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
use sp_std::{vec, vec::Vec};
use crate::Runtime;

pub type UpgradeAltair1028 = (asset_registry::CrossChainTransferabilityMigration,);

mod asset_registry {
	use cfg_types::{tokens as v1, tokens::CustomMetadata};
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::{pallet_prelude::OptionQuery, storage_alias, Twox64Concat};
	use orml_traits::asset_registry::AssetMetadata;
	#[cfg(feature = "try-runtime")]
	use sp_std::vec::Vec;

	use super::*;
	use crate::VERSION;

	/// Migrate all the registered asset's metadata to the new version of
	/// `CustomMetadata` which contains a `CrossChainTransferability` property.
	/// At this point in time, the `transferability` of Tranche tokens should be
	/// set to `CrossChainTransferability::Xcm` and for all other tokens to
	/// `CrossChainTransferability::Xcm`, with the exception of
	/// `Currency::Staking` tokens which are not registered in the first place.
	pub struct CrossChainTransferabilityMigration;

	// The old orml_asset_registry Metadata storage using v0::CustomMetadata
	#[storage_alias]
	type Metadata<T: orml_asset_registry::Config> = StorageMap<
		orml_asset_registry::Pallet<T>,
		Twox64Concat,
		CurrencyId,
		AssetMetadata<Balance, v0::CustomMetadata>,
		OptionQuery,
	>;

	impl OnRuntimeUpgrade for CrossChainTransferabilityMigration {
		fn on_runtime_upgrade() -> Weight {
			if VERSION.spec_version != 1028 {
				return Weight::zero();
			}

			orml_asset_registry::Metadata::<Runtime>::translate(
				|asset_id: CurrencyId, old_metadata: AssetMetadata<Balance, v0::CustomMetadata>| {
					match asset_id {
						CurrencyId::Staking(_) => None,
						CurrencyId::Tranche(_, _) => Some(to_metadata_v1(
							old_metadata,
							v1::CrossChainTransferability::Connectors,
						)),
						_ => Some(to_metadata_v1(
							old_metadata.clone(),
							v1::CrossChainTransferability::Xcm(old_metadata.additional.xcm),
						)),
					}
				},
			);

			let n = orml_asset_registry::Metadata::<Runtime>::iter().count() as u64;
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(n, n)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			use codec::Encode;

			let old_state: Vec<(CurrencyId, AssetMetadata<Balance, v0::CustomMetadata>)> =
				Metadata::<Runtime>::iter().collect::<Vec<_>>();

			Ok(old_state.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(old_state_encoded: Vec<u8>) -> Result<(), &'static str> {
			use codec::Decode;

			use crate::OrmlAssetRegistry;

			let old_state = sp_std::vec::Vec::<(
				CurrencyId,
				AssetMetadata<Balance, v0::CustomMetadata>,
			)>::decode(&mut old_state_encoded.as_ref())
			.map_err(|_| "Error decoding pre-upgrade state")?;

			for (asset_id, old_metadata) in old_state {
				let new_metadata = OrmlAssetRegistry::metadata(asset_id)
					.ok_or_else(|| "New state lost the metadata of an asset")?;

				match asset_id {
                    CurrencyId::Tranche(_, _) => ensure!(new_metadata == to_metadata_v1(
						old_metadata,
						v1::CrossChainTransferability::Connectors,
					), "The metadata of a tranche token wasn't just updated by setting `transferability` to `Connectors `"),
                    _ => ensure!(new_metadata == to_metadata_v1(
						old_metadata.clone(),
						v1::CrossChainTransferability::Xcm(old_metadata.additional.xcm),
					), "The metadata of a NON tranche token wasn't just updated by setting `transferability` to `Xcm`"),
                }
			}

			Ok(())
		}
	}

	mod v0 {
		use cfg_types::xcm::XcmMetadata;
		use codec::{Decode, Encode, MaxEncodedLen};
		use scale_info::TypeInfo;
		#[cfg(feature = "std")]
		use serde::{Deserialize, Serialize};

		// The `CustomMetadata` type as it was prior to adding the `transferability`
		// field and prior to removing the `xcm` field.
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
			pub xcm: XcmMetadata,
			pub mintable: bool,
			pub permissioned: bool,
			pub pool_currency: bool,
		}
	}

	fn to_metadata_v1(
		old: AssetMetadata<Balance, v0::CustomMetadata>,
		transferability: v1::CrossChainTransferability,
	) -> AssetMetadata<Balance, v1::CustomMetadata> {
		AssetMetadata {
			decimals: old.decimals,
			name: old.name,
			symbol: old.symbol,
			existential_deposit: old.existential_deposit,
			location: old.location,
			additional: CustomMetadata {
				mintable: old.additional.mintable,
				permissioned: old.additional.permissioned,
				pool_currency: old.additional.pool_currency,
				transferability,
			},
		}
	}
}

mod orml_tokens_migration {
	use cfg_primitives::AccountId;
	use cfg_types::tokens::before;
	use frame_support::{pallet_prelude::ValueQuery, storage_alias, Blake2_128Concat, Twox64Concat, ensure};
	use orml_tokens::AccountData;

	use super::*;

	/// As we dropped `CurrencyId::KSM` and `CurrencyId::AUSD`, we need to
	/// migrate the balances under the dropped variants in favour of the new,
	/// corresponding `CurrencyId::ForeignAsset`. We have never transferred KSM
	/// so we only need to deal with AUSD.
	pub struct CurrencyIdRefactorMigration;

	const ALTAIR_DEPRECATED_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::AUSD;
	const ALTAIR_NEW_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2);

	impl OnRuntimeUpgrade for CurrencyIdRefactorMigration {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			use codec::Encode;

			let old_ausd_entries: Vec<(AccountId, AccountData<Balance>)> = orml_tokens::Accounts::<Runtime>::iter()
				.filter(|(account, old_currency_id, account_data)| *old_currency_id == ALTAIR_DEPRECATED_AUSD_CURRENCY_ID)
				.map(|(account, _, account_data)| (account, account_data))
				.collect::<_>();

			Ok(old_ausd_entries.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
			use codec::Decode;
			use crate::OrmlTokens;

			let old_state = sp_std::vec::Vec::<(
				AccountId,
				AccountData<Balance>,
			)>::decode(&mut state.as_ref())
				.map_err(|_| "Error decoding pre-upgrade state")?;

			for (account, account_data) in old_state {
				ensure!(OrmlTokens::accounts(&account, ALTAIR_NEW_AUSD_CURRENCY_ID) == account_data.clone(), "The account data under the new AUSD Currency does NOT match the old one");
			};

			return Ok(());
		}


		fn on_runtime_upgrade() -> Weight {
			use frame_support::traits::tokens::fungibles::Mutate;

			let mut migrated_entries = 0;

			// Burn all AUSD tokens under the old CurrencyId and mint them under the new one
			orml_tokens::Accounts::<Runtime>::iter()
				.filter(|(account, old_currency_id, balance)| *old_currency_id == ALTAIR_DEPRECATED_AUSD_CURRENCY_ID)
				.for_each(|(account, _, account_data)| {
					let balance = account_data.free;
					// Burn the amount under the old, hardcoded CurrencyId
					<orml_tokens::Pallet<Runtime> as Mutate<AccountId>>::burn_from(ALTAIR_DEPRECATED_AUSD_CURRENCY_ID, &account, balance.clone())
						.map_err(|e| log::error!("Failed to call burn_from({:?}, {:?}, {balance}): {:?}", ALTAIR_DEPRECATED_AUSD_CURRENCY_ID, account, e))
						.ok();
					// // Now mint the amount under the new CurrencyID
					<orml_tokens::Pallet<Runtime> as Mutate<AccountId>>::mint_into(ALTAIR_NEW_AUSD_CURRENCY_ID, &account, balance)
						.map_err(|e| log::error!("Failed to mint_into burn_from({:?}, {:?}, {balance}): {:?}", ALTAIR_NEW_AUSD_CURRENCY_ID, account, e))
						.ok();

					migrated_entries += 1;
				});

			// Approximate weight given for every entry migration there are two calls being
			// made, so counting one read and one write per call. Given that we have very
			// few entries to be migrated, the specific value isn't quite relevant and we
			// could just set this to a high-enough value.
			<Runtime as frame_system::Config>::DbWeight::get()
				.reads_writes(migrated_entries * 2, migrated_entries * 2)
		}
	}
}
