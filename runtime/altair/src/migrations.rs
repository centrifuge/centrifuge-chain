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
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

/// The migration set for Altair 1034 @ Kusama. It includes all the migrations
/// that have to be applied on that chain.
pub type UpgradeAltair1034 = (
	// FIXME: This migration fails to decode 4 entries against Altair
	// orml_tokens_migration::CurrencyIdRefactorMigration,
	// At minimum, bumps storage version from 1 to 2
	runtime_common::migrations::nuke::Migration<crate::Loans, crate::RocksDbWeight, 1>,
	// At minimum, bumps storage version from 0 to 3
	runtime_common::migrations::nuke::Migration<crate::InterestAccrual, crate::RocksDbWeight, 0>,
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::Migration<crate::PoolSystem, crate::RocksDbWeight, 0>,
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::Migration<crate::Investments, crate::RocksDbWeight, 0>,
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
);

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
						},
					},
				),
			]
		}
	}
}

mod orml_tokens_migration {
	use cfg_primitives::{AccountId, Balance};
	use cfg_types::tokens::CurrencyId;
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::traits::tokens::{Fortitude, Precision};
	use orml_tokens::AccountData;
	use parity_scale_codec::{Decode, Encode};
	#[cfg(feature = "try-runtime")]
	use sp_runtime::DispatchError;
	use sp_std::vec::Vec;

	use super::*;
	use crate::Runtime;

	const DEPRECATED_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::AUSD;
	const NEW_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2);

	/// As we dropped `CurrencyId::KSM` and `CurrencyId::AUSD`, we need to
	/// migrate the balances under the dropped variants in favour of the new,
	/// corresponding `CurrencyId::ForeignAsset`. We have never transferred KSM
	/// so we only need to deal with AUSD.
	pub struct CurrencyIdRefactorMigration;

	#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
	pub struct OldState {
		pub total_issuance: Balance,
		pub entries: Vec<(AccountId, AccountData<Balance>)>,
	}

	impl OnRuntimeUpgrade for CurrencyIdRefactorMigration {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
			let total_issuance =
				orml_tokens::TotalIssuance::<Runtime>::get(DEPRECATED_AUSD_CURRENCY_ID);
			let entries: Vec<(AccountId, AccountData<Balance>)> =
				orml_tokens::Accounts::<Runtime>::iter()
					.filter(|(_, old_currency_id, _)| {
						*old_currency_id == DEPRECATED_AUSD_CURRENCY_ID
					})
					.map(|(account, _, account_data)| (account, account_data))
					.collect::<_>();

			Ok(OldState {
				total_issuance,
				entries,
			}
			.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), DispatchError> {
			use crate::OrmlTokens;

			let old_state = OldState::decode(&mut state.as_ref())
				.map_err(|_| "Error decoding pre-upgrade state")?;

			let new_total_issuance =
				orml_tokens::TotalIssuance::<Runtime>::get(NEW_AUSD_CURRENCY_ID);

			ensure!(
				old_state.total_issuance == new_total_issuance,
				"The old AUSD issuance differs from the new one"
			);

			for (account, account_data) in old_state.entries {
				ensure!(
					OrmlTokens::accounts(&account, NEW_AUSD_CURRENCY_ID) == account_data.clone(),
					"The account data under the new AUSD Currency does NOT match the old one"
				);
			}

			Ok(())
		}

		fn on_runtime_upgrade() -> Weight {
			use frame_support::traits::tokens::fungibles::Mutate;

			let mut migrated_entries: u64 = 0;

			// Burn all AUSD tokens under the old CurrencyId and mint them under the new one
			orml_tokens::Accounts::<Runtime>::iter()
				.filter(|(_, old_currency_id, _)| *old_currency_id == DEPRECATED_AUSD_CURRENCY_ID)
				.for_each(|(account, _, account_data)| {
					let balance = account_data.free;
					// Burn the amount under the old, hardcoded CurrencyId
					<orml_tokens::Pallet<Runtime> as Mutate<AccountId>>::burn_from(
						DEPRECATED_AUSD_CURRENCY_ID,
						&account,
						balance,
						Precision::Exact,
						Fortitude::Force,
					)
					.map_err(|e| {
						log::error!(
							"Failed to call burn_from({:?}, {:?}, {balance}): {:?}",
							DEPRECATED_AUSD_CURRENCY_ID,
							account,
							e
						)
					})
					.ok();
					// Now mint the amount under the new CurrencyID
					<orml_tokens::Pallet<Runtime> as Mutate<AccountId>>::mint_into(
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

			// Approximate weight given for every entry migration there are two calls being
			// made, so counting the reads and writes for each call.
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(
				migrated_entries.saturating_mul(5),
				migrated_entries.saturating_mul(4),
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
