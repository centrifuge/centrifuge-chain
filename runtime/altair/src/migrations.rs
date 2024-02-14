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

use cfg_primitives::{PoolId, TrancheId};
use cfg_types::tokens::{CurrencyId, CustomMetadata, ForeignAssetId, StakingCurrency};
use frame_support::{
	dispatch::{Decode, Encode, MaxEncodedLen, TypeInfo},
	traits::OnRuntimeUpgrade,
	weights::Weight,
	RuntimeDebugNoBound,
};
use orml_traits::asset_registry::AssetMetadata;

/// The migration set for Altair 1034 @ Kusama. It includes all the migrations
/// that have to be applied on that chain.
pub type UpgradeAltair1034 = (
	// Updates asset custom metadata from mid 2023 to latest (two fields missing/mismatching)
	translate_asset_metadata::Migration<super::Runtime>,
	// Removes hardcoded AUSD currency id and migrates balance
	ausd_to_foreign::CurrencyIdRefactorMigration<super::Runtime>,
	// At minimum, bumps storage version from 1 to 2
	runtime_common::migrations::nuke::ResetPallet<crate::Loans, crate::RocksDbWeight, 1>,
	// At minimum, bumps storage version from 0 to 3
	runtime_common::migrations::nuke::ResetPallet<crate::InterestAccrual, crate::RocksDbWeight, 0>,
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::ResetPallet<crate::PoolSystem, crate::RocksDbWeight, 0>,
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::ResetPallet<crate::Investments, crate::RocksDbWeight, 0>,
	// Funds pallet_rewards::Instance2 account with existential deposit
	pallet_rewards::migrations::new_instance::FundExistentialDeposit<
		crate::Runtime,
		pallet_rewards::Instance2,
		crate::NativeToken,
		crate::ExistentialDeposit,
	>,
	// Removes metadata containing xcm_v1 locations of registered assets and sets to hardcoded ones
	// containing xcm_v3 locations
	runtime_common::migrations::asset_registry_xcmv3::Migration<
		crate::Runtime,
		asset_registry::AltairAssets,
		5,
		5,
		2,
		9,
	>,
	// Low weight, mainly bumps storage version to latest (v1 to v2)
	crate::DmpQueue,
	// Low weight, mainly bumps storage version to latest (v2 to v3)
	crate::XcmpQueue,
	// Low weight, bumps uninitialized storage version from v0 to v1
	pallet_xcm::migration::v1::MigrateToV1<crate::Runtime>,
	// Sets currently unset safe XCM version to v2
	xcm_v2_to_v3::SetSafeXcmVersion,
	// Sets account codes for all precompiles
	runtime_common::migrations::precompile_account_codes::Migration<crate::Runtime>,
	// Migrates EpochExecution V1 to V2
	runtime_common::migrations::epoch_execution::Migration<crate::Runtime>,
	// Probably not needed, as storage is likely not populated. Migrates currency used in allowlist
	runtime_common::migrations::transfer_allowlist_currency::Migration<crate::Runtime>,
);

#[allow(clippy::upper_case_acronyms)]
#[derive(
	Clone,
	Copy,
	Default,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	RuntimeDebugNoBound,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen,
)]
pub enum OldCurrencyId {
	// The Native token, representing AIR in Altair and CFG in Centrifuge.
	#[default]
	#[codec(index = 0)]
	Native,

	/// A Tranche token
	#[codec(index = 1)]
	Tranche(PoolId, TrancheId),

	/// DEPRECATED - Will be removed in the next Altair RU 1034 when the
	/// orml_tokens' balances are migrated to the new CurrencyId for AUSD.
	#[codec(index = 2)]
	KSM,

	/// DEPRECATED - Will be removed in the next Altair RU 1034 when the
	/// orml_tokens' balances are migrated to the new CurrencyId for AUSD.
	#[codec(index = 3)]
	AUSD,

	/// A foreign asset
	#[codec(index = 4)]
	ForeignAsset(ForeignAssetId),

	/// A staking currency
	#[codec(index = 5)]
	Staking(StakingCurrency),
}

mod asset_registry {
	use cfg_primitives::Balance;
	use cfg_types::{
		tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
		xcm::XcmMetadata,
	};
	use sp_std::{vec, vec::Vec};
	use xcm::{v3::prelude::*, VersionedMultiLocation};

	pub struct AltairAssets;
	impl runtime_common::migrations::asset_registry_xcmv3::AssetsToMigrate for AltairAssets {
		fn get_assets_to_migrate() -> Vec<(
			CurrencyId,
			orml_asset_registry::AssetMetadata<Balance, CustomMetadata>,
		)> {
			let mut gk = [0u8; 32];
			gk[1] = 1;

			// 0x0081
			let mut gk_acala = [0u8; 32];
			gk_acala[..2].copy_from_slice(&[0, 129]);

			// Skipping AUSD since it seems that should be registered differently, lets do
			// it manually later on
			vec![
				(
					CurrencyId::Native,
					orml_asset_registry::AssetMetadata {
						decimals: 18,
						name: b"Altair".to_vec(),
						symbol: b"AIR".to_vec(),
						existential_deposit: 1_000_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X2(
								Parachain(crate::ParachainInfo::parachain_id().into()),
								GeneralKey {
									length: 2,
									data: gk,
								},
							),
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: false,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
							local_representation: None,
						},
					},
				),
				(
					CurrencyId::ForeignAsset(1),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Tether USDT".to_vec(),
						symbol: b"USDT".to_vec(),
						existential_deposit: 10_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X3(Parachain(1000), PalletInstance(50), GeneralIndex(1984)),
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: true,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
							local_representation: None,
						},
					},
				),
				(
					CurrencyId::ForeignAsset(2),
					orml_asset_registry::AssetMetadata {
						decimals: 12,
						name: b"Acala Dollar".to_vec(),
						symbol: b"aUSD".to_vec(),
						existential_deposit: 10_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X2(
								Parachain(2000),
								GeneralKey {
									length: 2,
									data: gk_acala,
								},
							),
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: true,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
							local_representation: None,
						},
					},
				),
				(
					CurrencyId::ForeignAsset(3),
					orml_asset_registry::AssetMetadata {
						decimals: 12,
						name: b"Kusama".to_vec(),
						symbol: b"KSM".to_vec(),
						existential_deposit: 10_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::Here,
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: false,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
							local_representation: None,
						},
					},
				),
			]
		}
	}
}

pub mod translate_asset_metadata {
	use cfg_primitives::Balance;
	use frame_support::{
		dispatch::{Decode, Encode, MaxEncodedLen, TypeInfo},
		storage_alias,
		traits::Get,
		Twox64Concat,
	};
	#[cfg(feature = "try-runtime")]
	use sp_runtime::DispatchError;
	#[cfg(feature = "try-runtime")]
	use sp_std::vec::Vec;

	use super::*;

	const LOG_PREFIX: &str = "TranslateMetadata";

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
	pub struct XcmMetadata {
		pub fee_per_second: Option<Balance>,
	}

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
	struct OldCustomMetadata {
		pub xcm: XcmMetadata,
		pub mintable: bool,
		pub permissioned: bool,
		pub pool_currency: bool,
	}

	#[storage_alias]
	type Metadata<T: orml_asset_registry::Config> = StorageMap<
		orml_asset_registry::Pallet<T>,
		Twox64Concat,
		OldCurrencyId,
		AssetMetadata<Balance, OldCustomMetadata>,
	>;

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: orml_asset_registry::Config<
			CustomMetadata = CustomMetadata,
			AssetId = CurrencyId,
			Balance = Balance,
		>,
	{
		fn on_runtime_upgrade() -> Weight {
			let mut weight = Weight::zero();
			orml_asset_registry::Metadata::<T>::translate::<
				AssetMetadata<Balance, OldCustomMetadata>,
				_,
			>(|_, meta| {
				weight.saturating_accrue(T::DbWeight::get().writes(1));
				Some(AssetMetadata {
					decimals: meta.decimals,
					name: meta.name,
					symbol: meta.symbol,
					existential_deposit: meta.existential_deposit,
					location: meta.location,
					additional: CustomMetadata {
						mintable: meta.additional.mintable,
						permissioned: meta.additional.permissioned,
						pool_currency: meta.additional.pool_currency,
						..Default::default()
					},
				})
			});
			log::info!("{LOG_PREFIX} Done translating asset metadata");

			weight
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
			let num_assets = Metadata::<T>::iter_keys().count() as u32;
			log::info!(
				"{LOG_PREFIX} PRE UPGRADE: Finished with {} registered assets",
				num_assets
			);

			Ok(num_assets.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), DispatchError> {
			let n_pre: u32 = Decode::decode(&mut pre_state.as_slice())
				.expect("pre_ugprade provides a valid state; qed");
			let n = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;
			assert_eq!(n_pre, n);

			log::info!("{LOG_PREFIX} POST UPGRADE: Finished");

			Ok(())
		}
	}
}

mod ausd_to_foreign {
	use cfg_primitives::{AccountId, Balance};
	use cfg_types::tokens::CurrencyId;
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::{
		pallet_prelude::ValueQuery, storage_alias, Blake2_128Concat, Twox64Concat,
	};
	use orml_tokens::AccountData;
	use parity_scale_codec::{Decode, Encode};
	#[cfg(feature = "try-runtime")]
	use sp_runtime::traits::Zero;
	#[cfg(feature = "try-runtime")]
	use sp_runtime::DispatchError;
	use sp_std::vec::Vec;

	use super::*;
	use crate::Runtime;

	const DEPRECATED_AUSD_CURRENCY_ID: OldCurrencyId = OldCurrencyId::AUSD;
	const NEW_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2);
	const LOG_PREFIX: &str = "MigrateAUSD";

	/// As we dropped `CurrencyId::KSM` and `CurrencyId::AUSD`, we need to
	/// migrate the balances under the dropped variants in favour of the new,
	/// corresponding `CurrencyId::ForeignAsset`. We have never transferred KSM
	/// so we only need to deal with AUSD.
	pub struct CurrencyIdRefactorMigration<T>(sp_std::marker::PhantomData<T>);

	#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
	pub struct OldState {
		pub total_issuance: Balance,
		pub entries: Vec<(AccountId, AccountData<Balance>)>,
	}

	#[storage_alias]
	type TotalIssuance<T: orml_tokens::Config> =
		StorageMap<orml_tokens::Pallet<T>, Twox64Concat, OldCurrencyId, Balance, ValueQuery>;

	#[storage_alias]
	type Accounts<T: orml_tokens::Config> = StorageDoubleMap<
		orml_tokens::Pallet<T>,
		Blake2_128Concat,
		AccountId,
		Twox64Concat,
		OldCurrencyId,
		AccountData<Balance>,
		ValueQuery,
	>;

	impl<T> OnRuntimeUpgrade for CurrencyIdRefactorMigration<T>
	where
		T: orml_asset_registry::Config<AssetId = CurrencyId, Balance = Balance>
			+ orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>
			+ frame_system::Config<AccountId = AccountId>,
	{
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
			let total_issuance = TotalIssuance::<T>::get(DEPRECATED_AUSD_CURRENCY_ID);
			let entries: Vec<(AccountId, AccountData<Balance>)> = Accounts::<T>::iter()
				.filter(|(_, old_currency_id, _)| *old_currency_id == DEPRECATED_AUSD_CURRENCY_ID)
				.map(|(account, _, account_data)| (account, account_data))
				.collect::<_>();

			log::info!(
				"{LOG_PREFIX} PRE-UPGRADE: Counted accounts to be migrated is {} with total issuance of {total_issuance}", entries.iter().count()
			);

			Ok(OldState {
				total_issuance,
				entries,
			}
			.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), DispatchError> {
			let old_state = OldState::decode(&mut state.as_ref())
				.map_err(|_| "Error decoding pre-upgrade state")?;

			let new_total_issuance = orml_tokens::TotalIssuance::<T>::get(NEW_AUSD_CURRENCY_ID);

			ensure!(
				old_state.total_issuance == new_total_issuance,
				"The old AUSD issuance differs from the new one"
			);
			ensure!(
				TotalIssuance::<T>::get(DEPRECATED_AUSD_CURRENCY_ID).is_zero(),
				"The total issuance of old AUSD is not zero!"
			);

			for (account, account_data) in old_state.entries {
				ensure!(
					orml_tokens::Pallet::<T>::accounts(&account, NEW_AUSD_CURRENCY_ID)
						== account_data.clone(),
					"The account data under the new AUSD Currency does NOT match the old one"
				);
				ensure!(
					Accounts::<T>::get(&account, DEPRECATED_AUSD_CURRENCY_ID) == Default::default(),
					"The account data for old AUSD is not cleared"
				);
			}

			log::info!("{LOG_PREFIX} POST-UPGRADE: Done");

			Ok(())
		}

		fn on_runtime_upgrade() -> Weight {
			use frame_support::traits::tokens::fungibles::Mutate;

			let mut migrated_entries: u64 = 0;

			// Burn all AUSD tokens under the old CurrencyId and mint them under the new one
			Accounts::<T>::iter()
				.filter(|(_, old_currency_id, _)| *old_currency_id == DEPRECATED_AUSD_CURRENCY_ID)
				.for_each(|(account, _, account_data)| {
					log::info!(
						"{LOG_PREFIX} Migrating account with balance: {}",
						account_data.free
					);

					let balance = account_data.free;

					// Remove account data and reduce total issuance manually at the end
					Accounts::<T>::remove(account.clone(), DEPRECATED_AUSD_CURRENCY_ID);

					// Now mint the amount under the new CurrencyID
					<orml_tokens::Pallet<T> as Mutate<AccountId>>::mint_into(
						NEW_AUSD_CURRENCY_ID,
						&account,
						balance,
					)
					.map_err(|e| {
						log::error!(
							"Failed to mint_into burn_from({:?}, {:?}, {balance}): {:?}",
							NEW_AUSD_CURRENCY_ID,
							account,
							e
						)
					})
					.ok();

					migrated_entries += 1;
				});
			log::info!(
				"{LOG_PREFIX} Number of migrated accounts: {:?} ",
				migrated_entries
			);

			TotalIssuance::<T>::remove(DEPRECATED_AUSD_CURRENCY_ID);
			log::info!("{LOG_PREFIX} Done");

			// Approximate weight given for every entry migration there are two calls being
			// made, so counting the reads and writes for each call.
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(
				migrated_entries.saturating_mul(5),
				migrated_entries.saturating_mul(4).saturating_add(1),
			)
		}
	}
}

mod xcm_v2_to_v3 {
	use super::*;
	use crate::{PolkadotXcm, RuntimeOrigin};

	pub struct SetSafeXcmVersion;

	impl OnRuntimeUpgrade for SetSafeXcmVersion {
		fn on_runtime_upgrade() -> Weight {
			// Unfortunately, SafeXcmVersion storage is not leaked to runtime, so we can't
			// do any pre- or post-upgrade checks
			PolkadotXcm::force_default_xcm_version(
				RuntimeOrigin::root(),
				Some(cfg_primitives::SAFE_XCM_VERSION),
			)
			.unwrap_or_else(|_| log::error!("Failed to set safe XCM version on runtime upgrade, requires manual call via governance"));

			crate::RocksDbWeight::get().writes(1)
		}
	}
}
