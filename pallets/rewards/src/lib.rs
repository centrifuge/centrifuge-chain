#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

//#[cfg(feature = "runtime-benchmarks")]
//mod benchmarking;

use frame_support::pallet_prelude::*;

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochDetails<BlockNumber, Balance> {
	ends_on: BlockNumber,
	total_reward: Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::traits::tokens::Balance;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type BlockPerEpoch: Get<Self::BlockNumber>;
		type Balance: Balance + MaxEncodedLen;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type ActiveEpoch<T: Config> = StorageValue<_, EpochDetails<T::BlockNumber, T::Balance>>;

	#[pallet::storage]
	pub type NextTotalReward<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(current_block: T::BlockNumber) -> Weight {
			if let Some(active_epoch) = ActiveEpoch::<T>::get() {
				if active_epoch.ends_on != current_block {
					return 0; //FIXME
				}
			}

			ActiveEpoch::<T>::put(EpochDetails {
				ends_on: current_block + T::BlockPerEpoch::get(),
				total_reward: NextTotalReward::<T>::get(),
			});

			0 //FIXME
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn stake(origin: OriginFor<T>) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		pub fn unstake(origin: OriginFor<T>) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		pub fn claim(origin: OriginFor<T>) -> DispatchResult {
			todo!()
		}
	}
}
