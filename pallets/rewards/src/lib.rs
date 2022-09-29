#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod types;

//#[cfg(feature = "runtime-benchmarks")]
//mod benchmarking;
use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, ReservableCurrency},
	transactional, PalletId,
};
use frame_system::pallet_prelude::*;
use num_traits::{NumAssignOps, NumOps, Signed};
use sp_runtime::{
	traits::{AccountIdConversion, Saturating, Zero},
	FixedPointNumber, FixedPointOperand, TokenError,
};
use types::{EpochDetails, FirstEpochDetails, GroupDetails, StakedDetails};

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type BlockPerEpoch: Get<Self::BlockNumber>;

		type Currency: ReservableCurrency<Self::AccountId>;

		type SignedBalance: From<BalanceOf<Self>>
			+ TryInto<BalanceOf<Self>>
			+ codec::FullCodec
			+ Copy
			+ Default
			+ scale_info::TypeInfo
			+ MaxEncodedLen
			+ NumOps
			+ NumAssignOps
			+ Saturating
			+ Signed
			+ Zero;

		type Rate: FixedPointNumber<Inner = BalanceOf<Self>>
			+ TypeInfo
			+ MaxEncodedLen
			+ Saturating
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
	pub type ActiveEpoch<T: Config> = StorageValue<
		_,
		EpochDetails<T::BlockNumber, BalanceOf<T>>,
		ValueQuery,
		FirstEpochDetails<frame_system::Pallet<T>>,
	>;

	#[pallet::storage]
	pub type NextTotalReward<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	#[pallet::storage]
	pub type Group<T: Config> = StorageValue<_, GroupDetails<BalanceOf<T>, T::Rate>, ValueQuery>;

	#[pallet::storage]
	pub type Staked<T: Config> = StorageMap<
		_,
		Blake2_256,
		T::AccountId,
		StakedDetails<BalanceOf<T>, T::SignedBalance>,
		ValueQuery,
	>;

	// --------------------------

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)] // TODO
	pub enum Event<T> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		fn on_initialize(current_block: T::BlockNumber) -> Weight {
			let active_epoch = ActiveEpoch::<T>::get();

			if active_epoch.ends_on() != current_block {
				return T::DbWeight::get().reads(1);
			}

			Group::<T>::mutate(|group| {
				if group.distribute_reward(active_epoch.total_reward()) {
					T::Currency::deposit_creating(
						&T::PalletId::get().into_account_truncating(),
						active_epoch.total_reward(),
					);
				}
			});

			ActiveEpoch::<T>::put(
				active_epoch.next(T::BlockPerEpoch::get(), NextTotalReward::<T>::get()),
			);

			T::DbWeight::get().reads_writes(2, 2) // + deposit_creating weight // TODO
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		#[pallet::weight(10_000)] //TODO
		#[transactional]
		pub fn stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			T::Currency::reserve(&who, amount)?;

			Group::<T>::mutate(|group| {
				Staked::<T>::mutate(&who, |staked| {
					staked.add_amount(amount, group.reward_per_token());
				});

				group.add_amount(amount);
			});

			Ok(())
		}

		#[pallet::weight(10_000)] //TODO
		#[transactional]
		pub fn unstake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			if T::Currency::reserved_balance(&who) < amount {
				return Err(DispatchError::Token(TokenError::NoFunds));
			}

			Group::<T>::mutate(|group| {
				Staked::<T>::mutate(&who, |staked| {
					staked.sub_amount(amount, group.reward_per_token());
				});

				group.sub_amount(amount);
			});

			T::Currency::unreserve(&who, amount);

			Ok(())
		}

		#[pallet::weight(10_000)] //TODO
		#[transactional]
		pub fn claim(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let group = Group::<T>::get();

			let reward =
				Staked::<T>::mutate(&who, |staked| staked.claim_reward(group.reward_per_token()));

			T::Currency::transfer(
				&T::PalletId::get().into_account_truncating(),
				&who,
				reward,
				ExistenceRequirement::KeepAlive,
			)
		}
	}
}
