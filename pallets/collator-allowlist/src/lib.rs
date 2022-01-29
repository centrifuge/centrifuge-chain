//! Collator Allowlist Pallet
//!
//! This pallet provides two extrinsics, one that allows sudo to
//! add collator ids to an allowlist, and another one that allows
//! sudo to remove them.
//!
//! We have this pallet implementing `ValidatorRegistration`, which,
//! in addition to the default `Session` pallet implementation, also
//! checks for the presence of a collator id in this allowlist.
//!
//! We do that to have tighter control over which collators get selected
//! per time windows, to avoid it defaulting to a FCFS setup until we
//! have chosen the right staking mechanism.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock;

mod benchmarking;

use frame_support::{dispatch::DispatchResult, traits::ValidatorRegistration};

use frame_system::ensure_root;

pub use pallet::*;

pub mod weights;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;

		/// The Validator Id type
		type ValidatorId: Member + Parameter + MaybeSerializeDeserialize;

		/// Type representing the underlying validator registration center.
		/// It offers us the API we need to check whether a collator
		/// is ready for its duties in the upcoming session.
		type ValidatorRegistration: ValidatorRegistration<Self::ValidatorId>;
	}

	// The genesis config type.
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub initial_state: Vec<T::ValidatorId>,
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
			self.initial_state
				.iter()
				.for_each(|id| <Allowlist<T>>::insert(id, ()));
		}
	}

	/// The collator's allowlist.
	/// Note: We implement it as a close-enough HashSet: Map<ValidatorId, ()>.
	#[pallet::storage]
	#[pallet::getter(fn get_allowlisted)]
	pub(super) type Allowlist<T: Config> = StorageMap<_, Blake2_256, T::ValidatorId, ()>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A collator has been added to the allowlist
		CollatorAdded(T::ValidatorId),

		/// A collator has been removed from the allowlist
		CollatorRemoved(T::ValidatorId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The collator has already been added to the allowlist.
		CollatorAlreadyAllowed,

		/// The collator is not ready yet following to the underlying `T::ValidatorRegistration`
		CollatorNotReady,

		/// The provided collator was not found in the storage.
		CollatorNotPresent,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add the given `collator_id` to the allowlist.
		/// Fails if
		///   - `origin` fails the `ensure_root` check
		///   - `collator_id` did not yet load their keys into the session pallet
		///   - `collator_id` is already part of the allowlist
		#[pallet::weight(<T as pallet::Config>::WeightInfo::add())]
		pub fn add(origin: OriginFor<T>, collator_id: T::ValidatorId) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(
				Self::collator_is_ready(&collator_id),
				Error::<T>::CollatorNotReady
			);

			ensure!(
				!<Allowlist<T>>::contains_key(&collator_id),
				Error::<T>::CollatorAlreadyAllowed
			);

			<Allowlist<T>>::insert(collator_id.clone(), ());
			Self::deposit_event(Event::CollatorAdded(collator_id));

			Ok(())
		}

		/// Remove a `collator_id` from the allowlist.
		/// Fails if
		///   - `origin` fails the `ensure_root` check
		///   - `collator_id` is not part of the allowlist
		#[pallet::weight(<T as pallet::Config>::WeightInfo::remove())]
		pub fn remove(origin: OriginFor<T>, collator_id: T::ValidatorId) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(
				<Allowlist<T>>::contains_key(&collator_id),
				Error::<T>::CollatorNotPresent
			);
			<Allowlist<T>>::remove(collator_id.clone());
			Self::deposit_event(Event::CollatorRemoved(collator_id));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Check whether the collator is ready to be called to duty.
	/// We use this indirection to provide a more natural and clear
	/// language that better matches our use case.
	fn collator_is_ready(collator_id: &T::ValidatorId) -> bool {
		T::ValidatorRegistration::is_registered(collator_id)
	}
}

/// Custom `ValidatorRegistration` implementation.
impl<T: Config> ValidatorRegistration<T::ValidatorId> for Pallet<T> {
	/// Check whether a validator is registered according to the pallet.
	/// True iff
	///   - the validator id is present in the allowlist and
	///   - the validator id is registered in the underlying validator registration center
	fn is_registered(id: &T::ValidatorId) -> bool {
		<Allowlist<T>>::contains_key(id) && T::ValidatorRegistration::is_registered(id)
	}
}
