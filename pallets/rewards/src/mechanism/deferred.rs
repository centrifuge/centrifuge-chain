use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
pub use pallet::*;
use sp_runtime::{
	traits::{
		EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
		One, Saturating, Zero,
	},
	ArithmeticError, FixedPointNumber, FixedPointOperand,
};

use super::{base, MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<T: Config> {
	base: base::Group<T::Balance, T::Rate>,
	last_rate: T::Rate,
	lost_reward: T::Balance,
	distribution_id: T::DistributionId,
}

impl<T: Config> Default for Group<T> {
	fn default() -> Self {
		Self {
			base: base::Group::default(),
			last_rate: T::Rate::zero(),
			lost_reward: T::Balance::zero(),
			distribution_id: T::DistributionId::default(),
		}
	}
}

impl<T: Config> Group<T> {
	fn get_last_rate(&self, currency: &Currency<T>) -> T::Rate {
		if self.distribution_id == currency.next_distribution_id {
			currency.prev_last_rate
		} else {
			self.last_rate
		}
	}
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<T: Config> {
	base: base::Account<T::Balance, T::IBalance>,
	rewarded_stake: T::Balance,
	distribution_id: T::DistributionId,
}

impl<T: Config> Default for Account<T> {
	fn default() -> Self {
		Self {
			base: base::Account::default(),
			rewarded_stake: T::Balance::zero(),
			distribution_id: T::DistributionId::default(),
		}
	}
}

impl<T: Config> Account<T> {
	fn was_distribution(&self, group: &Group<T>, currency: &Currency<T>) -> bool {
		if self.base.last_currency_movement as usize == currency.base.rpt_changes.len() {
			self.distribution_id != group.distribution_id
		} else {
			self.distribution_id != currency.prev_distribution_id
				|| group.distribution_id != currency.next_distribution_id
		}
	}

	fn get_rewarded_stake(&self, group: &Group<T>, currency: &Currency<T>) -> T::Balance {
		if self.was_distribution(group, currency) {
			self.base.stake
		} else {
			self.rewarded_stake
		}
	}

	fn update_rewarded_stake(&mut self, group: &Group<T>, currency: &Currency<T>) {
		self.rewarded_stake = self.get_rewarded_stake(group, currency);
		self.distribution_id = group.distribution_id;
	}

	fn last_rewarded_stake(
		&self,
		group: &Group<T>,
		currency: &Currency<T>,
	) -> Result<T::Balance, ArithmeticError> {
		group
			.get_last_rate(currency)
			.ensure_mul_int(self.get_rewarded_stake(group, currency))
	}
}

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<T: Config> {
	base: base::Currency<T::Balance, T::Rate, T::MaxCurrencyMovements>,
	prev_distribution_id: T::DistributionId,
	next_distribution_id: T::DistributionId,
	prev_last_rate: T::Rate,
}

impl<T: Config> Default for Currency<T> {
	fn default() -> Self {
		Self {
			base: base::Currency::default(),
			prev_distribution_id: T::DistributionId::default(),
			next_distribution_id: T::DistributionId::default(),
			prev_last_rate: T::Rate::default(),
		}
	}
}

pub struct Mechanism<Balance, IBalance, Rate, MaxCurrencyMovements>(
	sp_std::marker::PhantomData<(Balance, IBalance, Rate, MaxCurrencyMovements)>,
);

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type DistributionId: PartialEq
			+ Copy
			+ codec::FullCodec
			+ MaxEncodedLen
			+ Default
			+ TypeInfo
			+ One
			+ EnsureAdd
			+ sp_std::fmt::Debug;

		type Balance: tokens::Balance
			+ FixedPointOperand
			+ TryFrom<Self::IBalance>
			+ codec::FullCodec
			+ TypeInfo
			+ MaxEncodedLen;

		type IBalance: FixedPointOperand
			+ TryFrom<Self::Balance>
			+ codec::FullCodec
			+ TypeInfo
			+ MaxEncodedLen
			+ EnsureAdd
			+ EnsureSub
			+ Copy
			+ Signed
			+ sp_std::fmt::Debug
			+ Default;

		type Rate: FixedPointNumber + codec::FullCodec + TypeInfo + MaxEncodedLen;

		type MaxCurrencyMovements: Get<u32> + sp_std::fmt::Debug;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type LastDistributionId<T: Config> = StorageValue<_, T::DistributionId, ValueQuery>;

	impl<T: Config> RewardMechanism for Pallet<T>
	where
		<T::Rate as FixedPointNumber>::Inner: Signed,
	{
		type Account = Account<T>;
		type Balance = T::Balance;
		type Currency = Currency<T>;
		type Group = Group<T>;
		type MaxCurrencyMovements = T::MaxCurrencyMovements;

		fn is_ready(group: &Self::Group) -> bool {
			group.base.total_stake > Self::Balance::zero()
		}

		fn reward_group(
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> Result<Self::Balance, DispatchError> {
			let mut reward_used = Self::Balance::zero();

			if group.base.total_stake > Self::Balance::zero() {
				let reward = amount.ensure_add(group.lost_reward)?;
				base::Mechanism::<T::Balance, T::IBalance, T::Rate, T::MaxCurrencyMovements>::reward_group(
                    &mut group.base,
                    reward,
                )?;

				group.lost_reward = T::Balance::zero();
				group.last_rate = T::Rate::ensure_from_rational(reward, group.base.total_stake)?;

				reward_used = reward
			}

			group.distribution_id = LastDistributionId::<T>::try_mutate(
				|distribution_id| -> Result<T::DistributionId, DispatchError> {
					distribution_id.ensure_add_assign(One::one())?;
					Ok(*distribution_id)
				},
			)?;

			Ok(reward_used)
		}

		fn deposit_stake(
			account: &mut Self::Account,
			currency: &mut Self::Currency,
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> DispatchResult {
			account.update_rewarded_stake(group, currency);

			base::Mechanism::deposit_stake(
				&mut account.base,
				&mut currency.base,
				&mut group.base,
				amount,
			)
		}

		fn withdraw_stake(
			account: &mut Self::Account,
			currency: &mut Self::Currency,
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> DispatchResult {
			account.update_rewarded_stake(group, currency);

			let rewarded_amount = {
				let unrewarded_stake = account.base.stake.saturating_sub(account.rewarded_stake);
				let unrewarded_amount = amount.min(unrewarded_stake);
				amount.ensure_sub(unrewarded_amount)
			}?;

			base::Mechanism::withdraw_stake(
				&mut account.base,
				&mut currency.base,
				&mut group.base,
				amount,
			)?;

			let lost_reward = group
				.get_last_rate(currency)
				.ensure_mul_int(rewarded_amount)?;

			account.rewarded_stake.ensure_sub_assign(rewarded_amount)?;
			account
				.base
				.reward_tally
				.ensure_add_assign(lost_reward.ensure_into()?)?;

			group.lost_reward.ensure_add_assign(lost_reward)?;

			Ok(())
		}

		fn compute_reward(
			account: &Self::Account,
			currency: &Self::Currency,
			group: &Self::Group,
		) -> Result<Self::Balance, DispatchError> {
			base::Mechanism::compute_reward(&account.base, &currency.base, &group.base)?
				.ensure_sub(account.last_rewarded_stake(group, currency)?)
				.map_err(|e| e.into())
		}

		fn claim_reward(
			account: &mut Self::Account,
			currency: &Self::Currency,
			group: &Self::Group,
		) -> Result<Self::Balance, DispatchError> {
			let last_rewarded_stake = account.last_rewarded_stake(group, currency)?;

			let reward =
				base::Mechanism::claim_reward(&mut account.base, &currency.base, &group.base)?
					.ensure_sub(last_rewarded_stake)?;

			account
				.base
				.reward_tally
				.ensure_sub_assign(last_rewarded_stake.ensure_into()?)?;

			Ok(reward)
		}

		fn move_currency(
			currency: &mut Self::Currency,
			from_group: &mut Self::Group,
			to_group: &mut Self::Group,
		) -> Result<(), MoveCurrencyError> {
			base::Mechanism::<_, T::IBalance, _, _>::move_currency(
				&mut currency.base,
				&mut from_group.base,
				&mut to_group.base,
			)?;

			// Only if there was a distribution from last move, we update the previous
			// related data.
			if currency.next_distribution_id != from_group.distribution_id {
				currency.prev_distribution_id = from_group.distribution_id;
				currency.prev_last_rate = from_group.last_rate;
			}
			currency.next_distribution_id = to_group.distribution_id;

			Ok(())
		}

		fn account_stake(account: &Self::Account) -> Self::Balance {
			account.base.stake
		}

		fn group_stake(group: &Self::Group) -> Self::Balance {
			group.base.total_stake
		}
	}
}
