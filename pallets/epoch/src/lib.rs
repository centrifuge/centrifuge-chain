#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::pallet_prelude::*;
use sp_runtime::traits::{BlockNumberProvider, CheckedAdd, Zero};

pub trait Epoch<BlockNumber, AssociatedType> {
	fn update_epoch<R>(
		current_block: BlockNumber,
		finish: impl FnOnce(&EpochDetails<BlockNumber, AssociatedType>) -> R,
	) -> Option<R>;

	fn update_next_associated_data<R, E>(
		mutate: impl FnOnce(&mut AssociatedType) -> Result<R, E>,
	) -> Result<R, E>;
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochDetails<BlockNumber, AssociatedType> {
	pub number: u64,
	pub ends_on: BlockNumber,
	pub associated_data: AssociatedType,
}

/// Type used to initialize the first epoch with the correct block number
pub struct FirstEpochDetails<P>(std::marker::PhantomData<P>);
impl<Provider, BlockNumber, AssociatedType> Get<EpochDetails<BlockNumber, AssociatedType>>
	for FirstEpochDetails<Provider>
where
	BlockNumber: Zero,
	Provider: BlockNumberProvider<BlockNumber = BlockNumber>,
	AssociatedType: Default,
{
	fn get() -> EpochDetails<BlockNumber, AssociatedType> {
		EpochDetails {
			number: Zero::zero(),
			ends_on: Provider::current_block_number(),
			associated_data: AssociatedType::default(),
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type AssociatedType: Default + codec::FullCodec + scale_info::TypeInfo + MaxEncodedLen;

		#[pallet::constant]
		type BlockPerEpoch: Get<Self::BlockNumber>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// --------------------------
	//          Storage
	// --------------------------

	#[pallet::storage]
	pub type ActiveEpoch<T: Config> = StorageValue<
		_,
		EpochDetails<T::BlockNumber, T::AssociatedType>,
		ValueQuery,
		FirstEpochDetails<frame_system::Pallet<T>>,
	>;

	#[pallet::storage]
	pub type NextAssociatedData<T: Config> = StorageValue<_, T::AssociatedType, ValueQuery>;

	// --------------------------

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)] // TODO
	pub enum Event<T> {}

	#[pallet::error]
	pub enum Error<T> {}

	impl<T: Config> Epoch<T::BlockNumber, T::AssociatedType> for Pallet<T>
	where
		T::BlockNumber: CheckedAdd,
	{
		fn update_epoch<R>(
			current_block: T::BlockNumber,
			finish: impl FnOnce(&EpochDetails<T::BlockNumber, T::AssociatedType>) -> R,
		) -> Option<R> {
			let current_epoch = ActiveEpoch::<T>::get();
			if current_epoch.ends_on > current_block {
				return None;
			}

			ActiveEpoch::<T>::put(EpochDetails {
				number: current_epoch.number.checked_add(1)?,
				ends_on: current_epoch
					.ends_on
					.checked_add(&T::BlockPerEpoch::get())?,
				associated_data: NextAssociatedData::<T>::get(),
			});

			Some(finish(&current_epoch))
		}

		fn update_next_associated_data<R, E>(
			mutate: impl FnOnce(&mut T::AssociatedType) -> Result<R, E>,
		) -> Result<R, E> {
			NextAssociatedData::<T>::try_mutate(mutate)
		}
	}
}
