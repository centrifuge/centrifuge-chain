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
	traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub},
	FixedPointNumber, FixedPointOperand, TokenError,
};
use types::{Group, StakeAccount};

pub trait Rewards<AccountId> {
	type Balance: AtLeast32BitUnsigned + FixedPointOperand;

	fn distribute_reward(amount: Self::Balance) -> DispatchResult;
	fn deposit_stake(account_id: &AccountId, amount: Self::Balance) -> DispatchResult;
	fn withdraw_stake(account_id: &AccountId, amount: Self::Balance) -> DispatchResult;
	fn compute_reward(account_id: &AccountId) -> Result<Self::Balance, DispatchError>;
	fn claim_reward(account_id: &AccountId) -> Result<Self::Balance, DispatchError>;

	fn group_stake() -> Self::Balance;
	fn account_stake(account_id: &AccountId) -> Self::Balance;
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
			+ Signed
			+ CheckedSub
			+ CheckedAdd;

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
	pub type Groups<T: Config> = StorageValue<_, Group<BalanceOf<T>, T::Rate>, ValueQuery>;

	#[pallet::storage]
	pub type StakeAccounts<T: Config> = StorageMap<
		_,
		Blake2_256,
		T::AccountId,
		StakeAccount<BalanceOf<T>, T::SignedBalance>,
		ValueQuery,
	>;

	// --------------------------

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)] // TODO
	pub enum Event<T> {}

	#[pallet::error]
	pub enum Error<T> {}

	impl<T: Config> Rewards<T::AccountId> for Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		type Balance = BalanceOf<T>;

		fn distribute_reward(amount: Self::Balance) -> DispatchResult {
			Groups::<T>::try_mutate(|group| group.distribute_reward(amount))?;

			T::Currency::deposit_creating(&T::PalletId::get().into_account_truncating(), amount);

			Ok(())
		}

		fn deposit_stake(account_id: &T::AccountId, amount: Self::Balance) -> DispatchResult {
			T::Currency::reserve(&account_id, amount)?;

			Groups::<T>::try_mutate(|group| {
				StakeAccounts::<T>::try_mutate(account_id, |staked| {
					staked.add_amount(amount, group.reward_per_token())
				})?;

				group.add_amount(amount)
			})?;

			Ok(())
		}

		fn withdraw_stake(account_id: &T::AccountId, amount: Self::Balance) -> DispatchResult {
			if T::Currency::reserved_balance(&account_id) < amount {
				return Err(DispatchError::Token(TokenError::NoFunds));
			}

			Groups::<T>::try_mutate(|group| {
				StakeAccounts::<T>::try_mutate(account_id, |staked| {
					staked.sub_amount(amount, group.reward_per_token())
				})?;

				group.sub_amount(amount)
			})?;

			T::Currency::unreserve(&account_id, amount);

			Ok(())
		}

		fn compute_reward(account_id: &T::AccountId) -> Result<Self::Balance, DispatchError> {
			let group = Groups::<T>::get();
			let staked = StakeAccounts::<T>::get(account_id);
			let reward = staked.compute_reward(group.reward_per_token())?;

			Ok(reward)
		}

		fn claim_reward(account_id: &T::AccountId) -> Result<Self::Balance, DispatchError> {
			let group = Groups::<T>::get();
			let reward = StakeAccounts::<T>::try_mutate(account_id, |staked| {
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

		fn group_stake() -> Self::Balance {
			Groups::<T>::get().total_staked()
		}

		fn account_stake(account_id: &T::AccountId) -> Self::Balance {
			StakeAccounts::<T>::get(account_id).staked()
		}
	}
}
