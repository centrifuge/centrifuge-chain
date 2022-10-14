#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod types;

use cfg_traits::ops::{EnsureAdd, EnsureSub};
use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, ReservableCurrency},
	PalletId,
};
use num_traits::Signed;
use sp_runtime::{
	traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, Zero},
	ArithmeticError, FixedPointNumber, FixedPointOperand, TokenError,
};
use sp_std::iter::Sum;
use types::{CurrencyInfo, Group, StakeAccount};

pub trait Rewards<AccountId> {
	type Balance: AtLeast32BitUnsigned + FixedPointOperand + Sum;
	type GroupId;
	type CurrencyId;

	fn distribute_reward<Rate, It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Self::Balance, DispatchError>
	where
		Rate: FixedPointNumber,
		It: IntoIterator<Item = Self::GroupId>,
		It::IntoIter: Clone,
	{
		Self::distribute_reward_with_weights::<Rate, _, _>(
			reward,
			groups.into_iter().map(|group_id| (group_id, 1u64)),
		)
	}

	fn distribute_reward_with_weights<Rate, Weight, It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Self::Balance, DispatchError>
	where
		Rate: FixedPointNumber,
		Weight: AtLeast32BitUnsigned + Sum + FixedPointOperand,
		It: IntoIterator<Item = (Self::GroupId, Weight)>,
		It::IntoIter: Clone,
	{
		let groups = groups.into_iter();
		let total_weight: Weight = groups.clone().map(|(_, weight)| weight).sum();

		groups
			.map(|(group_id, weight)| {
				let reward_rate = Rate::checked_from_rational(weight, total_weight)
					.ok_or(ArithmeticError::DivisionByZero)?;

				Self::reward_group(
					reward_rate
						.checked_mul_int(reward)
						.ok_or(ArithmeticError::Overflow)?,
					group_id,
				)
			})
			.sum::<Result<Self::Balance, DispatchError>>()
	}

	fn reward_group(
		reward: Self::Balance,
		group_id: Self::GroupId,
	) -> Result<Self::Balance, DispatchError>;

	fn deposit_stake(
		account_id: &AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
	) -> DispatchResult;

	fn withdraw_stake(
		account_id: &AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
	) -> DispatchResult;

	fn compute_reward(
		account_id: &AccountId,
		currency_id: Self::CurrencyId,
	) -> Result<Self::Balance, DispatchError>;

	fn claim_reward(
		account_id: &AccountId,
		currency_id: Self::CurrencyId,
	) -> Result<Self::Balance, DispatchError>;

	fn group_stake(group_id: Self::GroupId) -> Self::Balance;
	fn account_stake(account_id: &AccountId, currency_id: Self::CurrencyId) -> Self::Balance;

	fn attach_currency(currency_id: Self::CurrencyId, group_id: Self::GroupId) -> DispatchResult;
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
			+ codec::FullCodec
			+ Copy
			+ Default
			+ scale_info::TypeInfo
			+ MaxEncodedLen
			+ Signed
			+ CheckedSub
			+ CheckedAdd;

		type Rate: FixedPointNumber + TypeInfo + MaxEncodedLen + Encode + Decode;

		type GroupId: codec::FullCodec + scale_info::TypeInfo + MaxEncodedLen + Copy;

		type CurrencyId: codec::FullCodec + scale_info::TypeInfo + MaxEncodedLen + Copy;

		#[pallet::constant]
		type MaxCurrencyMovements: Get<u32> + scale_info::TypeInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// --------------------------
	//          Storage
	// --------------------------

	#[pallet::storage]
	pub type Currencies<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CurrencyId,
		CurrencyInfo<BalanceOf<T>, T::Rate, T::GroupId, T::MaxCurrencyMovements>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type Groups<T: Config> =
		StorageMap<_, Blake2_128Concat, T::GroupId, Group<BalanceOf<T>, T::Rate>, ValueQuery>;

	#[pallet::storage]
	pub type StakeAccounts<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::CurrencyId,
		StakeAccount<BalanceOf<T>, T::SignedBalance>,
		ValueQuery,
	>;

	// --------------------------

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)] // TODO
	pub enum Event<T> {}

	#[pallet::error]
	pub enum Error<T> {
		// Emits when a currency is used but it has no a related group.
		CurrencyWithoutGroup,

		// Emits when a currency is moved more than `MaxCurrencyMovements` times.
		CurrencyMaxMovementsReached,
	}

	impl<T: Config> Rewards<T::AccountId> for Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand + Sum + EnsureAdd + EnsureSub + TryFrom<T::SignedBalance>,
		T::SignedBalance: FixedPointOperand + EnsureAdd + EnsureSub,
		<T::Rate as FixedPointNumber>::Inner: Signed,
	{
		type Balance = BalanceOf<T>;
		type CurrencyId = T::CurrencyId;
		type GroupId = T::GroupId;

		fn reward_group(
			reward: Self::Balance,
			group_id: Self::GroupId,
		) -> Result<Self::Balance, DispatchError> {
			Groups::<T>::try_mutate(group_id, |group| {
				if group.total_staked() > Self::Balance::zero() {
					group.distribute_reward(reward)?;

					T::Currency::deposit_creating(
						&T::PalletId::get().into_account_truncating(),
						reward,
					);

					return Ok(reward);
				}

				Ok(Self::Balance::zero())
			})
		}

		fn deposit_stake(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
			amount: Self::Balance,
		) -> DispatchResult {
			Currencies::<T>::try_mutate(currency_id, |currency| {
				let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;

				Groups::<T>::try_mutate(group_id, |group| {
					StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
						staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
						staked.add_amount(amount, group.reward_per_token())?;

						group.add_amount(amount)?;
						currency.add_amount(amount)?;

						T::Currency::reserve(&account_id, amount)
					})
				})
			})
		}

		fn withdraw_stake(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
			amount: Self::Balance,
		) -> DispatchResult {
			if T::Currency::reserved_balance(&account_id) < amount {
				Err(TokenError::NoFunds)?;
			}

			Currencies::<T>::try_mutate(currency_id, |currency| {
				let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;

				Groups::<T>::try_mutate(group_id, |group| {
					StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
						staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
						staked.sub_amount(amount, group.reward_per_token())?;

						group.sub_amount(amount)?;
						currency.sub_amount(amount)?;

						T::Currency::unreserve(&account_id, amount);

						Ok(())
					})
				})
			})
		}

		fn compute_reward(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
		) -> Result<Self::Balance, DispatchError> {
			let currency = Currencies::<T>::get(currency_id);
			let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;
			let group = Groups::<T>::get(group_id);

			StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
				staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
				let reward = staked.compute_reward(group.reward_per_token())?;

				Ok(reward)
			})
		}

		fn claim_reward(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
		) -> Result<Self::Balance, DispatchError> {
			let currency = Currencies::<T>::get(currency_id);
			let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;
			let group = Groups::<T>::get(group_id);

			StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
				staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
				let reward = staked.claim_reward(group.reward_per_token())?;

				T::Currency::transfer(
					&T::PalletId::get().into_account_truncating(),
					&account_id,
					reward,
					ExistenceRequirement::KeepAlive,
				)?;

				Ok(reward)
			})
		}

		fn group_stake(group_id: Self::GroupId) -> Self::Balance {
			Groups::<T>::get(group_id).total_staked()
		}

		fn account_stake(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
		) -> Self::Balance {
			StakeAccounts::<T>::get(account_id, currency_id).staked()
		}

		fn attach_currency(
			currency_id: Self::CurrencyId,
			next_group_id: Self::GroupId,
		) -> DispatchResult {
			Currencies::<T>::try_mutate(currency_id, |currency| {
				if let Some(prev_group_id) = currency.group_id {
					Groups::<T>::try_mutate(prev_group_id, |prev_group| -> DispatchResult {
						Groups::<T>::try_mutate(next_group_id, |next_group| {
							let rpt_tally = next_group
								.reward_per_token()
								.checked_sub(&prev_group.reward_per_token())
								.ok_or(ArithmeticError::Underflow)?;

							currency
								.add_rpt_tally(rpt_tally)
								.map_err(|_| Error::<T>::CurrencyMaxMovementsReached)?;

							prev_group.sub_amount(currency.total_staked())?;
							next_group.add_amount(currency.total_staked())?;

							Ok(())
						})
					})?;
				}

				currency.group_id = Some(next_group_id);

				Ok(())
			})
		}
	}
}
