//! # Migration pallet for runtime
//!
//! This pallet provides functionality for migrating a previous chain-state (possibly from a
//! stand-alone chain) to a new chain state (possbily a parachain now). This pallet is necessary due
//! to the exising boundaries that are put onto runtime upgrades from the relay-chain side.  
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::sp_runtime::traits::Zero;
use frame_support::weights::Weight;

pub use pallet::*;
pub use weights::*;

#[cfg(test)]
mod test_data;

pub mod weights;

#[cfg(test)]
pub mod tests;

#[cfg(test)]
pub mod mock;

pub mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::traits::{Convert, Saturating};
	use frame_support::sp_runtime::traits::{One, Zero};
	use frame_support::transactional;
	use frame_system::pallet_prelude::*;
	use sp_std::vec::Vec;
	use sp_version::RuntimeVersion;

	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;
	use frame_support::sp_std::fmt::Debug;

	pub type NumAccounts = u64;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ codec::Codec
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ Debug
			+ MaxEncodedLen;

		/// Maximum number of accounts that can be migrated at once
		#[pallet::constant]
		type MaxAccounts: Get<u64>;

		/// Associated type for Event enum
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// WeightInfo
		type WeightInfo: WeightInfo;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Pallet genesis configuration type declaration.
	///
	/// It allows to build genesis storage.
	#[pallet::genesis_config]
	pub struct GenesisConfig {}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self {}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Number of accounts that have been migrated
		MigratedSystemAccounts(u64),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Too many accounts in the vector for the call of `migrate_system_account`.
		/// [number_is, number_should_be]
		//TooManyAccounts(u64, u64),
		TooManyAccounts,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the given fee for the key
		#[pallet::weight(<T as pallet::Config>::WeightInfo::migrate_system_account(T::MaxAccounts::get()))]
		#[transactional]
		pub fn migrate_system_account(
			origin: OriginFor<T>,
			accounts: Vec<(Vec<u8>, Vec<u8>)>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let num_accounts = accounts.len();
			ensure!(
				// TODO: TryInto!
				accounts.len() <= T::MaxAccounts::get() as usize,
				//Error::<T>::TooManyAccounts(accounts.len(), MaxAccounts::<T>::get())
				Error::<T>::TooManyAccounts
			);

			for (key, value) in accounts {
				storage::unhashed::put_raw(key.as_slice(), value.as_slice());
			}

			// TODO: TryInto
			Self::deposit_event(Event::<T>::MigratedSystemAccounts(num_accounts as u64));

			Ok(().into())
		}
		#[pallet::weight(<T as pallet::Config>::WeightInfo::migrate_balances_issuance())]
		#[transactional]
		pub fn migrate_balances_issuance(
			origin: OriginFor<T>,
			total_issuance: T::Balance,
		) -> DispatchResultWithPostInfo {
			// TODO: Hard code key or get key for total issuance and inject
			// storage::unhashed::put_raw(key, total_issuance.encode())

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {}
