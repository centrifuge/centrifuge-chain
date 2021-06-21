//! # Fees pallet for runtime
//!
//! This pallet provides functionality for setting and getting fees associated with an Hash key..
//! Fees are set by FeeOrigin or RootOrigin
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	traits::{Currency, EnsureOrigin, ExistenceRequirement, WithdrawReasons},
	weights::Weight,
};

pub use pallet::*;
pub mod weights;
pub use weights::*;

mod data;

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use crate::data::system_account::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			if Self::system_account() {
				upgrade_system_account()
			}
			let max_per_block = T::BlockWeights::get().max_block;

			max_per_block
		}

		fn on_initialize(_n: T::BlockNumber) -> frame_support::weights::Weight {}
	}

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

	#[pallet::type_value]
	pub fn OnSystemAccountEmpty() -> bool {
		false
	}
	/// Indicator if the Account map of frame-system has already been migrated
	#[pallet::storage]
	#[pallet::getter(fn system_account)]
	pub(super) type SystemAccount<T: Config> =
		StorageValue<_, bool, ValueQuery, OnSystemAccountEmpty>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SystemAccountUpgraded,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Fee associated to given key not found
		UpgradeToHeavy,
	}
}

impl<T: Config> Pallet<T> {
	pub fn upgrade_system_account() {
		for key_value in System_Account {
			sp_io::storage::set(key_value.key, key_value.value);
		}
	}
}
