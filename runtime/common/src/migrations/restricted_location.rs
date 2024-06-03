use cfg_primitives::AccountId;
use cfg_types::{locations::RestrictedTransferLocation, tokens::CurrencyId};
use frame_support::{
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
};
use pallet_transfer_allowlist::AccountCurrencyTransferAllowance;
use parity_scale_codec::Encode;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Hash};
use staging_xcm::v4;

mod old {
	use cfg_primitives::AccountId;
	use cfg_types::{domain_address::DomainAddress, tokens::CurrencyId};
	use frame_support::{pallet_prelude::*, storage_alias};
	use frame_system::pallet_prelude::*;
	use pallet_transfer_allowlist::AllowanceDetails;
	use sp_core::H256;
	use staging_xcm::v3;

	#[derive(
		Clone, RuntimeDebugNoBound, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo,
	)]
	pub enum RestrictedTransferLocation {
		Local(AccountId),
		XCM(H256),
		Address(DomainAddress),
	}

	#[storage_alias]
	pub type AccountCurrencyTransferAllowance<T: pallet_transfer_allowlist::Config> = StorageNMap<
		pallet_transfer_allowlist::Pallet<T>,
		(
			NMapKey<Twox64Concat, AccountId>,
			NMapKey<Twox64Concat, CurrencyId>,
			NMapKey<Blake2_128Concat, RestrictedTransferLocation>,
		),
		AllowanceDetails<BlockNumberFor<T>>,
		OptionQuery,
	>;

	pub fn create_apps_location_v3(account_id: &AccountId) -> v3::Location {
		// Ref: https://github.com/centrifuge/apps/blob/b59bdd34561a4ccd90e0d803c14a3729fc2f3a6d/centrifuge-app/src/utils/usePermissions.tsx#L386
		//v3::Location::new(1, v3::Junctions::X2((), ()))
		todo!()
	}
}

const LOG_PREFIX: &str = "MigrateRestrictedTransferLocation:";

pub struct MigrateRestrictedTransferLocation<T>(sp_std::marker::PhantomData<T>);
impl<T> OnRuntimeUpgrade for MigrateRestrictedTransferLocation<T>
where
	T: pallet_transfer_allowlist::Config<
		AccountId = AccountId,
		CurrencyId = CurrencyId,
		Location = RestrictedTransferLocation,
	>,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!("{LOG_PREFIX} Check keys to migrate...");

		let mut weight = Weight::zero();

		let key_translations = old::AccountCurrencyTransferAllowance::<T>::iter_keys()
			.filter_map(|(account_id, currency_id, old_restricted_location)| {
				weight.saturating_accrue(T::DbWeight::get().reads(1));
				match old_restricted_location {
					old::RestrictedTransferLocation::XCM(hash) => {
						migrate_location_key(&account_id, hash).map(|new_restricted_location| {
							(
								(account_id.clone(), currency_id, old_restricted_location),
								(account_id, currency_id, new_restricted_location),
							)
						})
					}
					_ => None,
				}
			})
			.collect::<Vec<_>>();

		for (old_key, new_key) in key_translations {
			log::info!("{LOG_PREFIX} Remove {old_key:?} and add {new_key:?}");

			let value = old::AccountCurrencyTransferAllowance::<T>::get(&old_key);
			old::AccountCurrencyTransferAllowance::<T>::remove(old_key);
			AccountCurrencyTransferAllowance::<T>::set(new_key, value);

			weight.saturating_accrue(T::DbWeight::get().writes(2));
		}

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		todo!()
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(pre_state: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		todo!()
	}
}

fn migrate_location_key(account_id: &AccountId, hash: H256) -> Option<RestrictedTransferLocation> {
	let old_location = old::create_apps_location_v3(account_id);
	if BlakeTwo256::hash(&old_location.encode()) == hash {
		match v4::Location::try_from(old_location) {
			Ok(location) => {
				log::info!("{LOG_PREFIX} Hash: '{hash}' migrated!");
				let new_restricted_location = RestrictedTransferLocation::XCM(location);

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
