//! # Epoch pallet
//!
//! Utility pallet without extrinsics that handle the concept of epoch and how it changes during the time.
//! Supports different implementations in the same runtime.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::pallet_prelude::*;
use sp_runtime::traits::{BlockNumberProvider, CheckedAdd};

/// Trait to represent the epoch behavior.
pub trait Epoch<BlockNumber, AssociatedType> {
	/// Updates the epoch system with a new current block.
	/// If the given block is higher than the current finalizing block,
	/// a new epoch is created, and the callback is called with the finalized epoch.
	/// This method should be be called in an `on_initialized` hook.
	fn update_epoch<R>(
		current_block: BlockNumber,
		finish: impl FnOnce(&EpochDetails<BlockNumber, AssociatedType>) -> R,
	) -> Option<R>;

	/// Updates the associated epoch data for the incoming epoch.
	fn update_next_associated_data<R, E>(
		mutate: impl FnOnce(&mut AssociatedType) -> Result<R, E>,
	) -> Result<R, E>;
}

/// Struct that contains the epoch information.
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochDetails<BlockNumber, AssociatedType> {
	/// Block when the epoch finalizes.
	pub ends_on: BlockNumber,

	/// Associated data to the epoch.
	pub associated_data: AssociatedType,
}

/// Type used to initialize the first epoch with the correct block number
pub struct FirstEpochDetails<P>(std::marker::PhantomData<P>);
impl<Provider, BlockNumber, AssociatedType> Get<EpochDetails<BlockNumber, AssociatedType>>
	for FirstEpochDetails<Provider>
where
	Provider: BlockNumberProvider<BlockNumber = BlockNumber>,
	AssociatedType: Default,
{
	fn get() -> EpochDetails<BlockNumber, AssociatedType> {
		EpochDetails {
			ends_on: Provider::current_block_number(),
			associated_data: AssociatedType::default(),
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;

		/// Type that represent the user attached data to the epoch.
		type AssociatedType: Default + codec::FullCodec + scale_info::TypeInfo + MaxEncodedLen;

		/// Epoch interval.
		/// Each BlockPerEpoch, an epoch finalizes and a new epoch starts.
		#[pallet::constant]
		type BlockPerEpoch: Get<Self::BlockNumber>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// Dispatched when a the current epoch finalizes and a new epoch starts.
		NewEpoch { ends_on: T::BlockNumber },
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {}

	// --------------------------
	//          Storage
	// --------------------------

	/// Current epoch information
	#[pallet::storage]
	pub type ActiveEpoch<T: Config<I>, I: 'static = ()> = StorageValue<
		_,
		EpochDetails<T::BlockNumber, T::AssociatedType>,
		ValueQuery,
		FirstEpochDetails<frame_system::Pallet<T>>,
	>;

	/// Stores the associated epoch data for the incoming epoch.
	/// The current epoch is consider inmutable once it starts.
	/// This storage allows to modify the the associated data used in the next epoch.
	#[pallet::storage]
	pub type NextAssociatedData<T: Config<I>, I: 'static = ()> =
		StorageValue<_, T::AssociatedType, ValueQuery>;

	// --------------------------

	impl<T: Config<I>, I: 'static> Epoch<T::BlockNumber, T::AssociatedType> for Pallet<T, I> {
		fn update_epoch<R>(
			current_block: T::BlockNumber,
			finish: impl FnOnce(&EpochDetails<T::BlockNumber, T::AssociatedType>) -> R,
		) -> Option<R> {
			let current_epoch = ActiveEpoch::<T, I>::get();
			if current_epoch.ends_on > current_block {
				return None;
			}

			let ends_on = current_epoch
				.ends_on
				.checked_add(&T::BlockPerEpoch::get())?;

			ActiveEpoch::<T, I>::put(EpochDetails {
				ends_on,
				associated_data: NextAssociatedData::<T, I>::get(),
			});

			Self::deposit_event(Event::NewEpoch { ends_on });

			Some(finish(&current_epoch))
		}

		fn update_next_associated_data<R, E>(
			mutate: impl FnOnce(&mut T::AssociatedType) -> Result<R, E>,
		) -> Result<R, E> {
			NextAssociatedData::<T, I>::try_mutate(mutate)
		}
	}
}
