//! # Fees pallet for runtime
//!
//! This pallet provides functionality for setting and getting fees associated with an Hash key..
//! Fees are set by FeeOrigin or RootOrigin
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	traits::{ValidatorRegistration},
};

use frame_system::ensure_root;

pub use pallet::*;

pub mod weights;
use scale_info::TypeInfo;
pub use weights::*;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, )]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub enum CollatorStatus {
	Whitelisted,
}

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_session::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
	}

	// The genesis config type.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub initial_state: Vec<(T::ValidatorId, CollatorStatus)>,
	}

	// The default value for the genesis config type.
	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				initial_state: vec![],
			}
		}
	}

	// The build of genesis for the pallet.
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			self.initial_state.iter().for_each(|(id, status)| <Status<T>>::insert(id, status));
		}
	}

	/// Stores the status associated with a collator Id
	#[pallet::storage]
	#[pallet::getter(fn status)]
	pub(super) type Status<T: Config> =
		StorageMap<_, Blake2_256, T::ValidatorId, CollatorStatus>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CollatorWhitelisted(T::ValidatorId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Placeholder variant for now
		CollatorAlreadyWhitelisted,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// TODO(nuno): use the right weight here
		#[pallet::weight(<T as pallet::Config>::WeightInfo::whitelist())]
		pub fn whitelist(
			_origin: OriginFor<T>,
			collator_id: T::ValidatorId,
		) -> DispatchResult {
			// TODO(nuno): ensure origin is sudo

			<Status<T>>::insert(collator_id.clone(), CollatorStatus::Whitelisted);
			
			Self::deposit_event(Event::CollatorWhitelisted(collator_id));
			Ok(())
		}
	}
}

impl<T: Config + pallet_session::Config> ValidatorRegistration<T::ValidatorId> for Pallet<T> {
	fn is_registered(id: &T::ValidatorId) -> bool {
		Self::status(id) == Some(CollatorStatus::Whitelisted)
			&& pallet_session::Pallet::<T>::is_registered(id)
	}
}