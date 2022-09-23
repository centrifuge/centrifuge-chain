#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

//#[cfg(feature = "runtime-benchmarks")]
//mod benchmarking;

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, ReservableCurrency},
	transactional, PalletId,
};
use frame_system::pallet_prelude::*;

use sp_runtime::{
	traits::{AccountIdConversion, BlockNumberProvider},
	ArithmeticError, FixedPointNumber, FixedPointOperand, TokenError,
};

use num_traits::{NumAssignOps, NumOps, Signed};

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochDetails<BlockNumber, Balance> {
	ends_on: BlockNumber,
	total_reward: Balance,
}

/// Type used to initialize the first epoch with the correct block number
pub struct FirstEpochDetails<P>(std::marker::PhantomData<P>);
impl<P, N, B: Default> Get<EpochDetails<N, B>> for FirstEpochDetails<P>
where
	P: BlockNumberProvider<BlockNumber = N>,
{
	fn get() -> EpochDetails<N, B> {
		EpochDetails {
			ends_on: P::current_block_number(),
			total_reward: B::default(),
		}
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct GroupDetails<Balance, Rate> {
	total_staked: Balance,
	reward_per_token: Rate,
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct StakedDetails<Balance, SignedBalance> {
	amount: Balance,
	reward_tally: SignedBalance,
}

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
			+ Signed;

		type Rate: FixedPointNumber<Inner = BalanceOf<Self>>
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

			if active_epoch.ends_on != current_block {
				return T::DbWeight::get().reads(1);
			}

			Group::<T>::mutate(|group| {
				if group.total_staked > BalanceOf::<T>::default() {
					T::Currency::deposit_creating(
						&T::PalletId::get().into_account_truncating(),
						active_epoch.total_reward,
					);

					let rate_increment = T::Rate::saturating_from_rational(
						active_epoch.total_reward,
						group.total_staked,
					);
					group.reward_per_token = group.reward_per_token + rate_increment;
				}
			});

			ActiveEpoch::<T>::put(EpochDetails {
				ends_on: current_block + T::BlockPerEpoch::get(),
				total_reward: NextTotalReward::<T>::get(),
			});

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

			Group::<T>::mutate(|group| {
				Staked::<T>::mutate(&who, |staked| {
					staked.amount += amount;
					staked.reward_tally += group.reward_per_token.saturating_mul_int(amount).into();
				});

				group.total_staked += amount;
			});

			T::Currency::reserve(&who, amount)
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
					staked.amount -= amount;
					staked.reward_tally -= group.reward_per_token.saturating_mul_int(amount).into();
				});

				group.total_staked -= amount;
			});

			T::Currency::unreserve(&who, amount);

			Ok(())
		}

		#[pallet::weight(10_000)] //TODO
		#[transactional]
		pub fn claim(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let group = Group::<T>::get();

			let reward: BalanceOf<T> = Staked::<T>::try_mutate(&who, |staked| {
				let reward: T::SignedBalance = group
					.reward_per_token
					.saturating_mul_int(staked.amount)
					.into();

				let rectified_reward = (reward - staked.reward_tally)
					.try_into()
					.map_err(|_| DispatchError::Arithmetic(ArithmeticError::Underflow));

				staked.reward_tally = reward;

				rectified_reward
			})?;

			T::Currency::transfer(
				&T::PalletId::get().into_account_truncating(),
				&who,
				reward,
				ExistenceRequirement::KeepAlive,
			)
		}
	}
}
