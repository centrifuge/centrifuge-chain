//! # Migration pallet for runtime
//!
//! This pallet provides functionality for migrating a previous chain-state (possibly from a
//! stand-alone chain) to a new chain state (possbily a parachain now). This pallet is necessary due
//! to the exising boundaries that are put onto runtime upgrades from the relay-chain side.  
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{Contains, Currency},
};
pub use pallet::*;
use scale_info::TypeInfo;
pub use weights::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod test_data;
#[cfg(feature = "runtime-benchmarks")]
mod test_data;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

type BalanceOf<T> = <<T as pallet_vesting::Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

#[derive(Encode, Decode, PartialEq, Clone, TypeInfo)]
pub enum MigrationStatus {
	Inactive,
	Ongoing,
	Complete,
}

#[frame_support::pallet]
pub mod pallet {
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_support::sp_std::convert::TryInto;
	use frame_support::transactional;
	use frame_system::pallet_prelude::*;
	use sp_std::vec::Vec;

	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::sp_runtime::traits::Saturating;
	use frame_support::sp_runtime::ArithmeticError;
	use frame_support::traits::VestingSchedule;
	use pallet_proxy::ProxyDefinition;
	use pallet_vesting::VestingInfo;

	pub type NumAccounts = u64;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_vesting::Config
		+ pallet_balances::Config
		+ pallet_proxy::Config
	{
		/// Maximum number of accounts that can be migrated at once
		#[pallet::constant]
		type MigrationMaxAccounts: Get<u32>;

		/// Maximum number of vestings that can be migrated at once
		#[pallet::constant]
		type MigrationMaxVestings: Get<u32>;

		/// Maximum number of vestings that can be migrated at once
		#[pallet::constant]
		type MigrationMaxProxies: Get<u32>;

		/// Associated type for Event enum
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// WeightInfo
		type WeightInfo: WeightInfo;

		/// The call filter that will be used when pallet is in an ongoing migration
		type OngoingFilter: Contains<<Self as frame_system::Config>::Call>;

		/// The call filter that will be used when pallet has finalize the migration
		type FinalizedFilter: Contains<<Self as frame_system::Config>::Call>;

		/// The call filter that will be used when pallet is inactive
		type InactiveFilter: Contains<<Self as frame_system::Config>::Call>;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::type_value]
	pub fn OnStatusEmpty() -> MigrationStatus {
		MigrationStatus::Inactive
	}

	#[pallet::storage]
	#[pallet::getter(fn status)]
	pub(super) type Status<T: Config> = StorageValue<_, MigrationStatus, ValueQuery, OnStatusEmpty>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Number of accounts that have been migrated
		MigratedSystemAccounts(u32),

		/// Number of vesting that have been migrated
		MigratedVestingAccounts(u32),

		/// Number of proxies that have been migrated
		MigratedProxyProxies(u32),

		/// The new and the old issuance after the migration of issuance.
		/// [`OldIssuance`, `NewIssuance`]
		MigratedTotalIssuance(T::Balance, T::Balance),

		/// This is an error that must be dispatched as an Event, as we do not want to fail the whole batch
		/// when one account fails. Should also not happen, as we take them from mainnet. But...
		FailedToMigrateVestingFor(T::AccountId),

		/// Defines the vesting we migrated
		MigratedVestingFor(
			T::AccountId,
			<<T as pallet_vesting::Config>::Currency as frame_support::traits::Currency<
				<T as frame_system::Config>::AccountId,
			>>::Balance,
			<<T as pallet_vesting::Config>::Currency as frame_support::traits::Currency<
				<T as frame_system::Config>::AccountId,
			>>::Balance,
			T::BlockNumber,
		),

		/// Indicates if a migration of proxy data failed, this should NEVER happen, and can only
		/// happen due to insufficient balances during reserve
		FailedToMigrateProxyDataFor(T::AccountId),

		/// Indicates that proxy data has been migrated succesfully for
		/// [`ProxiedAccount`, `DepositOnProxiesAccount`, `NumberOfProxies`]
		MigratedProxyDataFor(
			T::AccountId,
			<<T as pallet_proxy::Config>::Currency as frame_support::traits::Currency<
				<T as frame_system::Config>::AccountId,
			>>::Balance,
			u64,
		),

		/// Indicates that the migration is finished
		MigrationFinished,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Too many accounts in the vector for the call of `migrate_system_account`.
		TooManyAccounts,

		/// Too many vestingInfos in the vector for the call of `migrate_veting_vesting`.
		TooManyVestings,

		/// Too many proxies in the vector for the call of `migrate_proxy_proxies`.
		TooManyProxies,

		/// Indicates that a migration call happened, although the migration is already closed
		MigrationAlreadyCompleted,

		/// Indicates that a finalize call happened, although the migration pallet is not in an
		/// ongoing migration
		OnlyFinalizeOngoing,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Migrating the Account informations from frame_system.
		///
		/// This call takes the raw scale encoded key (= patricia-key for each account in the `Account` storage and inserts
		/// the provided scale encoded value (= `AccountInfo`) into the underlying DB.
		///
		/// Note: As we are converting from substrate-v2 to substrate-v3 we must do type-conversions. Those conversions are done
		/// off-chain.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::migrate_system_account(T::MigrationMaxAccounts::get()))]
		#[transactional]
		pub fn migrate_system_account(
			origin: OriginFor<T>,
			accounts: Vec<(Vec<u8>, Vec<u8>)>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			Self::activate_migration()?;

			let num_accounts = accounts.len();
			ensure!(
				accounts.len()
					<= T::MigrationMaxAccounts::get()
						.try_into()
						.map_err(|_| ArithmeticError::Overflow)?,
				Error::<T>::TooManyAccounts
			);

			for (key, value) in accounts {
				storage::unhashed::put_raw(key.as_slice(), value.as_slice());
			}

			// This is safe as MigrationMaxAccounts is a u32
			Self::deposit_event(Event::<T>::MigratedSystemAccounts(num_accounts as u32));

			Ok(
				Some(<T as pallet::Config>::WeightInfo::migrate_system_account(
					num_accounts as u32,
				))
				.into(),
			)
		}

		/// Migrates a the `TotalIssuance`.
		///
		/// The provide balance here, will be ADDED to the existing `TotalIssuance` of the system.
		/// Calley better be sure, that the total issuance matches the actual total issuance in the system,
		/// which means, that the `AccountInfo` from the frame_system is migrated afterwards.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::migrate_balances_issuance())]
		#[transactional]
		pub fn migrate_balances_issuance(
			origin: OriginFor<T>,
			additional_issuance: T::Balance,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			Self::activate_migration()?;

			let current_issuance = pallet_balances::Pallet::<T>::total_issuance();
			let total_issuance = current_issuance.saturating_add(additional_issuance);

			let key = <pallet_balances::pallet::TotalIssuance<T> as frame_support::storage::generator::StorageValue<T::Balance>>::storage_value_final_key();

			storage::unhashed::put_raw(&key[..], total_issuance.encode().as_slice());

			Self::deposit_event(Event::<T>::MigratedTotalIssuance(
				current_issuance,
				total_issuance,
			));

			Ok(().into())
		}

		/// Migrates vesting information to this system.
		///
		/// The `VestingInfo` is adapted off-chain, so that it represents the correct vesting information
		/// on this chain.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::migrate_vesting_vesting(T::MigrationMaxVestings::get()))]
		#[transactional]
		pub fn migrate_vesting_vesting(
			origin: OriginFor<T>,
			vestings: Vec<(T::AccountId, VestingInfo<BalanceOf<T>, T::BlockNumber>)>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			Self::activate_migration()?;

			ensure!(
				vestings.len()
					<= T::MigrationMaxVestings::get()
						.try_into()
						.map_err(|_| ArithmeticError::Overflow)?,
				Error::<T>::TooManyVestings
			);

			// This is safe as MigrationMaxVestings is a u32
			let mut trying = vestings.len() as u32;
			let num_vestings = trying;

			for (who, schedule) in vestings {
				let _not_care_here = pallet_vesting::Pallet::<T>::add_vesting_schedule(
					&who,
					schedule.locked(),
					schedule.per_block(),
					schedule.starting_block(),
				)
				.map_err(|_| {
					Self::deposit_event(Event::<T>::FailedToMigrateVestingFor(who.clone()));
					trying -= 1;
				})
				.map(|_| {
					Self::deposit_event(Event::<T>::MigratedVestingFor(
						who,
						schedule.locked(),
						schedule.per_block(),
						schedule.starting_block(),
					))
				});
			}

			Self::deposit_event(Event::<T>::MigratedVestingAccounts(trying));

			Ok(
				Some(<T as pallet::Config>::WeightInfo::migrate_vesting_vesting(
					num_vestings,
				))
				.into(),
			)
		}

		/// Migrates to `Proxies` storage from another chain.
		///
		/// As the `Proxies` storage changed between v2 and v3, a transformation for the v2 data is done off-chain.
		/// The input defines an array of of tuples, where each tuple defines, the proxied account, the reserve that
		/// must be done on this account and the proxies for this account.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::migrate_proxy_proxies(T::MigrationMaxProxies::get()))]
		#[transactional]
		pub fn migrate_proxy_proxies(
			origin: OriginFor<T>,
			proxies: Vec<(
				T::AccountId,
				<<T as pallet_proxy::Config>::Currency as frame_support::traits::Currency<
					<T as frame_system::Config>::AccountId,
				>>::Balance,
				(
					BoundedVec<
						ProxyDefinition<T::AccountId, T::ProxyType, T::BlockNumber>,
						<T as pallet_proxy::Config>::MaxProxies,
					>,
					<<T as pallet_proxy::Config>::Currency as frame_support::traits::Currency<
						<T as frame_system::Config>::AccountId,
					>>::Balance,
				),
			)>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			Self::activate_migration()?;

			ensure!(
				proxies.len()
					<= <T as Config>::MigrationMaxProxies::get()
						.try_into()
						.map_err(|_| ArithmeticError::Overflow)?,
				Error::<T>::TooManyProxies
			);

			// This is safe as MigrationMaxProxies is a u32
			let mut trying = proxies.len() as u32;
			let num_proxies = trying;

			for (account_id, reserve, (data, deposit)) in proxies {
				let _not_care_result = <<T as pallet_proxy::Config>::Currency
					as frame_support::traits::ReservableCurrency<T::AccountId>>::reserve(&account_id, reserve)
						.map_err(|_| {
							Self::deposit_event(Event::<T>::FailedToMigrateProxyDataFor(account_id.clone()));
							trying -= 1;
						})
						.map(|_| {
							let len = data.len() as u64;
							let val = (data, deposit);
							pallet_proxy::Proxies::<T>::insert(&account_id, val);

							Self::deposit_event(Event::<T>::MigratedProxyDataFor(
								account_id,
								deposit,
								len
							));
							()
						});
			}

			Self::deposit_event(Event::<T>::MigratedProxyProxies(trying));

			Ok(
				Some(<T as pallet::Config>::WeightInfo::migrate_proxy_proxies(
					num_proxies,
				))
				.into(),
			)
		}

		/// This extrinsic disables the call-filter. After this has been called the chain will accept
		/// all calls again.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::finalize())]
		#[transactional]
		pub fn finalize(origin: OriginFor<T>) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(
				<Status<T>>::get() == MigrationStatus::Ongoing,
				Error::<T>::OnlyFinalizeOngoing
			);

			<Status<T>>::set(MigrationStatus::Complete);

			Self::deposit_event(Event::<T>::MigrationFinished);

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn activate_migration() -> DispatchResult {
		let mut status = <Status<T>>::get();

		if status == MigrationStatus::Inactive {
			<Status<T>>::set(MigrationStatus::Ongoing);
			status = MigrationStatus::Ongoing;
		}

		ensure!(
			status == MigrationStatus::Ongoing,
			Error::<T>::MigrationAlreadyCompleted
		);

		Ok(())
	}
}

impl<T: Config> Contains<<T as frame_system::Config>::Call> for Pallet<T> {
	fn contains(c: &<T as frame_system::Config>::Call) -> bool {
		let status = <Status<T>>::get();
		match status {
			MigrationStatus::Inactive => T::InactiveFilter::contains(c),
			MigrationStatus::Ongoing => T::OngoingFilter::contains(c),
			MigrationStatus::Complete => T::FinalizedFilter::contains(c),
		}
	}
}
