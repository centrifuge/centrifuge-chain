#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod types;

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, ReservableCurrency},
	PalletId,
};
use num_traits::Signed;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, CheckedSub, Saturating},
	FixedPointNumber, FixedPointOperand, TokenError,
};
use types::{GroupDetails, StakedDetails};

pub trait Rewards<AccountId, Balance> {
	fn distribute_reward(amount: Balance) -> DispatchResult;
	fn deposit_stake(account_id: &AccountId, amount: Balance) -> DispatchResult;
	fn withdraw_stake(account_id: &AccountId, amount: Balance) -> DispatchResult;
	fn compute_reward(account_id: &AccountId) -> Result<Balance, DispatchError>;
	fn claim_reward(account_id: &AccountId) -> Result<Balance, DispatchError>;
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

		type Currency: ReservableCurrency<Self::AccountId>;

		type SignedBalance: From<BalanceOf<Self>>
			+ TryInto<BalanceOf<Self>>
			+ codec::FullCodec
			+ Copy
			+ Default
			+ scale_info::TypeInfo
			+ MaxEncodedLen
			+ Saturating
			+ Signed
			+ CheckedSub
			+ CheckedAdd;

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

	impl<T: Config> Rewards<T::AccountId, BalanceOf<T>> for Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		fn distribute_reward(amount: BalanceOf<T>) -> DispatchResult {
			Group::<T>::try_mutate(|group| group.distribute_reward(amount))?;

			T::Currency::deposit_creating(&T::PalletId::get().into_account_truncating(), amount);

			Ok(())
		}

		fn deposit_stake(account_id: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
			T::Currency::reserve(&account_id, amount)?;

			Group::<T>::try_mutate(|group| {
				Staked::<T>::try_mutate(account_id, |staked| {
					staked.add_amount(amount, group.reward_per_token())
				})?;

				group.add_amount(amount)
			})?;

			Ok(())
		}

		fn withdraw_stake(account_id: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
			if T::Currency::reserved_balance(&account_id) < amount {
				return Err(DispatchError::Token(TokenError::NoFunds));
			}

			Group::<T>::try_mutate(|group| {
				Staked::<T>::try_mutate(account_id, |staked| {
					staked.sub_amount(amount, group.reward_per_token())
				})?;

				group.sub_amount(amount)
			})?;

			T::Currency::unreserve(&account_id, amount);

			Ok(())
		}

		fn compute_reward(account_id: &T::AccountId) -> Result<BalanceOf<T>, DispatchError> {
			let group = Group::<T>::get();
			let staked = Staked::<T>::get(account_id);
			let reward = staked.compute_reward(group.reward_per_token())?;

			Ok(reward)
		}

		fn claim_reward(account_id: &T::AccountId) -> Result<BalanceOf<T>, DispatchError> {
			let group = Group::<T>::get();
			let reward = Staked::<T>::try_mutate(account_id, |staked| {
				staked.claim_reward(group.reward_per_token())
			})?;

			T::Currency::transfer(
				&T::PalletId::get().into_account_truncating(),
				&account_id,
				reward,
				ExistenceRequirement::KeepAlive,
			)?;

			Ok(reward)
		}
	}
}
