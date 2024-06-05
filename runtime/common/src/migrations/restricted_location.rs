use cfg_primitives::AccountId;
use cfg_types::{locations::RestrictedTransferLocation, tokens::FilterCurrency};
use frame_support::{
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
};
use pallet_transfer_allowlist::AccountCurrencyTransferAllowance;
use parity_scale_codec::Encode;
use sp_arithmetic::traits::SaturatedConversion;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Hash};
use sp_std::vec::Vec;
use staging_xcm::{v4, VersionedLocation};

mod old {
	use cfg_primitives::AccountId;
	use cfg_types::{domain_address::DomainAddress, tokens::FilterCurrency};
	use frame_support::{pallet_prelude::*, storage_alias};
	use frame_system::pallet_prelude::*;
	use pallet_transfer_allowlist::AllowanceDetails;
	use sp_core::H256;
	use staging_xcm::v3;

	#[derive(
		Clone,
		RuntimeDebugNoBound,
		Encode,
		parity_scale_codec::Decode,
		Eq,
		PartialEq,
		MaxEncodedLen,
		TypeInfo,
	)]
	pub enum RestrictedTransferLocation {
		Local(AccountId),
		Xcm(H256),
		Address(DomainAddress),
	}

	#[storage_alias]
	pub type AccountCurrencyTransferAllowance<T: pallet_transfer_allowlist::Config> = StorageNMap<
		pallet_transfer_allowlist::Pallet<T>,
		(
			NMapKey<Twox64Concat, AccountId>,
			NMapKey<Twox64Concat, FilterCurrency>,
			NMapKey<Blake2_128Concat, RestrictedTransferLocation>,
		),
		AllowanceDetails<BlockNumberFor<T>>,
		OptionQuery,
	>;

	pub fn location_v3_created_by_apps(account_id: AccountId) -> v3::Location {
		// Ref: https://github.com/centrifuge/apps/blob/b59bdd34561a4ccd90e0d803c14a3729fc2f3a6d/centrifuge-app/src/utils/usePermissions.tsx#L386
		// for account_id == "4dTeMxuPJCK7zQGhFcgCivSJqBs9Wo2SuMSQeYCCuVJ9xrE2"
		v3::Location::new(
			1,
			v3::Junctions::X2(
				v3::Junction::Parachain(1000), // AssetHub
				v3::Junction::AccountId32 {
					network: None,
					id: account_id.into(),
				},
			),
		)
	}
}

const LOG_PREFIX: &str = "MigrateRestrictedTransferLocation:";

pub struct MigrateRestrictedTransferLocation<T, AccountMap>(
	sp_std::marker::PhantomData<(T, AccountMap)>,
);
impl<T, AccountMap> OnRuntimeUpgrade for MigrateRestrictedTransferLocation<T, AccountMap>
where
	T: pallet_transfer_allowlist::Config<
		AccountId = AccountId,
		CurrencyId = FilterCurrency,
		Location = RestrictedTransferLocation,
	>,
	AccountMap: Get<Vec<(AccountId, AccountId)>>,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!("{LOG_PREFIX} Check keys to migrate...");

		let mut weight = T::DbWeight::get().reads(
			old::AccountCurrencyTransferAllowance::<T>::iter_keys()
				.count()
				.saturated_into(),
		);

		let key_translations = Self::get_key_translations();

		for (acc, currency, old_location, maybe_new_location) in key_translations {
			let old_key = (&acc, &currency, &old_location);
			log::info!("{LOG_PREFIX} Removing old key {old_key:?}");
			let value = old::AccountCurrencyTransferAllowance::<T>::get(old_key);
			old::AccountCurrencyTransferAllowance::<T>::remove(old_key);

			if let Some(new_location) = maybe_new_location {
				let new_key = (&acc, &currency, &new_location);
				log::info!("{LOG_PREFIX} Adding new key {new_key:?}");
				AccountCurrencyTransferAllowance::<T>::set(new_key, value);
			}

			weight.saturating_accrue(T::DbWeight::get().writes(2));
		}

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		let key_translations = Self::get_key_translations();
		assert!(
			key_translations
				.iter()
				.all(|(_, _, _, new_location)| new_location.is_some()),
			"At least one XCM location could not be translated"
		);

		let count: u64 = old::AccountCurrencyTransferAllowance::<T>::iter_keys()
			.count()
			.saturated_into();

		log::info!("{LOG_PREFIX} Pre checks done!");
		Ok(count.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		let count_pre: u64 = parity_scale_codec::Decode::decode(&mut pre_state.as_slice())
			.expect("pre_upgrade provides a valid state; qed");
		let count_post: u64 = AccountCurrencyTransferAllowance::<T>::iter_keys()
			.count()
			.saturated_into();
		assert_eq!(count_pre, count_post, "Number of keys in AccountCurrencyTransferAllowance changed during migration: pre {count_pre} vs post {count_post}");

		log::info!("{LOG_PREFIX} Post checks done!");
		Ok(())
	}
}

impl<T, AccountMap> MigrateRestrictedTransferLocation<T, AccountMap>
where
	T: pallet_transfer_allowlist::Config<
		AccountId = AccountId,
		CurrencyId = FilterCurrency,
		Location = RestrictedTransferLocation,
	>,
	AccountMap: Get<Vec<(AccountId, AccountId)>>,
{
	fn migrate_location_key(
		account_id: AccountId,
		hash: H256,
	) -> Option<RestrictedTransferLocation> {
		let old_location = old::location_v3_created_by_apps(account_id);
		if BlakeTwo256::hash(&old_location.encode()) == hash {
			match v4::Location::try_from(old_location) {
				Ok(location) => {
					log::info!("{LOG_PREFIX} Hash: '{hash}' migrated!");
					let new_restricted_location =
						RestrictedTransferLocation::Xcm(VersionedLocation::V4(location));

					Some(new_restricted_location)
				}
				Err(_) => {
					log::error!("{LOG_PREFIX} Non isometric location v3 -> v4");
					None
				}
			}
		} else {
			log::error!("{LOG_PREFIX} Hash can not be recovered");
			None
		}
	}

	fn get_key_translations() -> Vec<(
		AccountId,
		FilterCurrency,
		old::RestrictedTransferLocation,
		Option<RestrictedTransferLocation>,
	)> {
		let accounts = AccountMap::get();

		old::AccountCurrencyTransferAllowance::<T>::iter_keys()
			.filter_map(|(account_id, currency_id, old_restricted_location)| {
				match (accounts.iter().find(|(key, _)| key == &account_id), old_restricted_location.clone()) {
					(Some((_, transfer_address)), old::RestrictedTransferLocation::Xcm(hash)) => Some((
						account_id.clone(),
						currency_id,
						old_restricted_location,
						Self::migrate_location_key(transfer_address.clone(), hash),
					)),
					(None, old::RestrictedTransferLocation::Xcm(_)) => {
						log::warn!("{LOG_PREFIX} Account {account_id:?} missing in AccountMap despite old XCM location storage");
						Some((
							account_id.clone(),
							currency_id,
							old_restricted_location,
							// Leads to storage entry removal
							// TODO: Discuss whether we are fine with removing such storage entries or whether we want to keep the old undecodable ones or maybe just use same account id per default instead of removing?
							None,
						))
					}
					_ => None,
				}
			})
			.collect::<Vec<_>>()
	}
}
