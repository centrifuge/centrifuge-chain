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

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct GroupDetails<Balance, Rate> {
	total_staked: Balance,
	reward_per_token: Rate,
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct StakedDetails<Balance> {
	amount: Balance,
	reward_tally: Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::traits::tokens::Balance;
	use frame_system::pallet_prelude::*;

	use sp_runtime::{FixedPointNumber, FixedPointOperand};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		#[pallet::constant]
		type BlockPerEpoch: Get<Self::BlockNumber>;

		type Balance: Balance + MaxEncodedLen + FixedPointOperand;

		type Rate: FixedPointNumber<Inner = Self::Balance>
			+ TypeInfo
			+ MaxEncodedLen
			+ Encode
			+ Decode;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// --------------------------
	//          Storage
	// --------------------------

	#[pallet::storage]
	pub type ActiveEpoch<T: Config> = StorageValue<_, EpochDetails<T::BlockNumber, T::Balance>>;

	#[pallet::storage]
	pub type NextTotalReward<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

	#[pallet::storage]
	pub type Group<T: Config> = StorageValue<_, GroupDetails<T::Balance, T::Rate>, ValueQuery>;

	#[pallet::storage]
	pub type Staked<T: Config> =
		StorageMap<_, Blake2_256, T::AccountId, StakedDetails<T::Balance>, ValueQuery>;

	// --------------------------

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(current_block: T::BlockNumber) -> Weight {
			let active_epoch = ActiveEpoch::<T>::get().unwrap_or(EpochDetails {
				ends_on: current_block,
				total_reward: NextTotalReward::<T>::get(),
			});

			if active_epoch.ends_on != current_block {
				return 0; //FIXME
			}

			Group::<T>::mutate(|group| {
				if group.total_staked > T::Balance::default() {
					let rate = T::Rate::saturating_from_rational(
						active_epoch.total_reward,
						group.total_staked,
					);
					group.reward_per_token = group.reward_per_token + rate;
				}
			});

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
		pub fn stake(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Group::<T>::mutate(|group| {
				group.total_staked += amount;

				Staked::<T>::mutate(who, |staked| {
					staked.amount += amount;
					staked.reward_tally += group.reward_per_token.saturating_mul_int(amount);
				});
			});

			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn unstake(_origin: OriginFor<T>) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)]
		pub fn claim(_origin: OriginFor<T>) -> DispatchResult {
			todo!()
		}
	}
}
