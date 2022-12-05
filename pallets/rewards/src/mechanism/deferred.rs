use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
pub use pallet::*;
use sp_runtime::{
	traits::{One, Saturating, Zero},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand,
};

use super::{base, MechanismError, RewardMechanism};

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<T: Config> {
	base: base::Group<T::Balance, T::Rate>,
	prev_total_stake: T::Balance,
	last_rate: T::Rate,
	lost_rewarded_stake: T::Balance,
	distribution_id: T::DistributionId,
}

impl<T: Config> Default for Group<T> {
	fn default() -> Self {
		Self {
			base: base::Group::default(),
			prev_total_stake: T::Balance::zero(),
			last_rate: T::Rate::zero(),
			lost_rewarded_stake: T::Balance::zero(),
			distribution_id: T::DistributionId::default(),
		}
	}
}

impl<T: Config> Group<T> {
	fn correct_last_rate(&self, currency: &Currency<T>) -> T::Rate {
		if self.distribution_id == currency.next_distribution_id {
			currency.prev_last_rate
		} else {
			self.last_rate
		}
	}
}

pub struct InitialGroup;
impl<T: Config> Get<Group<T>> for InitialGroup {
	fn get() -> Group<T> {
		LastDistributionId::<T>::try_mutate(|distribution_id| -> Result<_, DispatchError> {
			distribution_id.ensure_add_assign(One::one())?;

			Ok(Group {
				base: base::Group::default(),
				prev_total_stake: Default::default(),
				last_rate: Default::default(),
				lost_rewarded_stake: Default::default(),
				distribution_id: distribution_id.clone(),
			})
		})
		.unwrap() //TODO: Check arithmetic overflow without lose invariants: how?
	}
}

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
	fn reward_tally_correction(
		&self,
		group: &Group<T>,
		currency: &Currency<T>,
	) -> Result<T::Balance, DispatchError> {
		if self.distribution_id != group.distribution_id
			&& (self.distribution_id != currency.prev_distribution_id
				|| group.distribution_id != currency.next_distribution_id)
		{
			let delta_stake = self.base.stake.ensure_sub(self.rewarded_stake)?;
			let correct_rpt = RptHistory::<T>::get(self.distribution_id).ok_or(
				DispatchError::Other("'DistributionId' not found in 'RptHistory'"),
			);
			Ok(correct_rpt.ensure_mul_int(delta_stake)?)
		} else {
			Ok(Zero::zero())
		}
	}

	fn rewarded_stake_updated(&self, group: &Group<T>, currency: &Currency<T>) -> T::Balance {
		if self.distribution_id != group.distribution_id
			&& (self.distribution_id != currency.prev_distribution_id
				|| group.distribution_id != currency.next_distribution_id)
		{
			self.base.stake
		} else {
			self.rewarded_stake
		}
	}

	fn last_rewarded_stake(
		&self,
		group: &Group<T>,
		currency: &Currency<T>,
	) -> Result<T::Balance, ArithmeticError> {
		group
			.correct_last_rate(currency)
			.ensure_mul_int(self.rewarded_stake_updated(group, currency))
	}

	fn update(&mut self, group: &Group<T>, currency: &Currency<T>) -> Result<(), ArithmeticError> {
		let reward_tally_correction = self
			.reward_tally_correction(group, currency)?
			.ensure_into()?;

		let rewarded_stake = self.rewarded_stake_updated(group, currency);

		self.base
			.reward_tally
			.ensure_add_assign(reward_tally_correction)?;
		self.rewarded_stake = rewarded_stake;
		self.distribution_id = group.distribution_id;

		Ok(())
	}
}

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
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type RptHistory<T: Config> =
		StorageMap<_, Blake2_128Concat, T::DistributionId, T::Rate>;

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

		fn reward_group(
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> Result<Self::Balance, DispatchError> {
			let mut rpt_correction = T::Rate::zero();
			if group
				.prev_total_stake
				.ensure_sub(group.lost_rewarded_stake)?
				> T::Balance::zero()
			{
				rpt_correction = T::Rate::ensure_from_rational(
					group.last_rate.ensure_mul_int(group.lost_rewarded_stake)?,
					group
						.prev_total_stake
						.ensure_sub(group.lost_rewarded_stake)?,
				)?;
			}

			group.base.rpt.ensure_add_assign(rpt_correction)?;

			base::Mechanism::<T::Balance, T::IBalance, T::Rate, T::MaxCurrencyMovements>::reward_group(
				&mut group.base,
				amount,
			)?;

			RptHistory::<T>::insert(group.distribution_id, rpt_correction);

			group.last_rate = T::Rate::ensure_from_rational(amount, group.base.total_stake)?;
			group.lost_rewarded_stake = T::Balance::zero();
			group.prev_total_stake = group.base.total_stake;

			group.distribution_id = LastDistributionId::<T>::try_mutate(|distribution_id| {
				distribution_id.ensure_add_assign(One::one())?;
				Ok(*distribution_id)
			})?;

			Ok(amount) //TODO Fix in case everybody goes out.
		}

		fn deposit_stake(
			account: &mut Self::Account,
			currency: &mut Self::Currency,
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> Result<(), DispatchError> {
			account.update(group, currency)?;

			base::Mechanism::deposit_stake(
				&mut account.base,
				&mut currency.base,
				&mut group.base,
				amount,
			)?;

			Ok(())
		}

		fn withdraw_stake(
			account: &mut Self::Account,
			currency: &mut Self::Currency,
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> Result<(), DispatchError> {
			account.update(group, currency)?;

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
				.correct_last_rate(currency)
				.ensure_mul_int(rewarded_amount)?;

			account.rewarded_stake.ensure_sub_assign(rewarded_amount)?;
			account
				.base
				.reward_tally
				.ensure_add_assign(lost_reward.ensure_into()?)?;

			group
				.lost_rewarded_stake
				.ensure_add_assign(rewarded_amount)?;

			Ok(())
		}

		fn compute_reward(
			account: &Self::Account,
			currency: &Self::Currency,
			group: &Self::Group,
		) -> Result<Self::Balance, DispatchError> {
			base::Mechanism::compute_reward(&account.base, &currency.base, &group.base)?
				.ensure_sub(account.last_rewarded_stake(group, currency)?)?
				.ensure_add(account.reward_tally_correction(group, currency)?)
				.map_err(|e| e.into())
		}

		fn claim_reward(
			account: &mut Self::Account,
			currency: &Self::Currency,
			group: &Self::Group,
		) -> Result<Self::Balance, DispatchError> {
			let last_rewarded_stake = account.last_rewarded_stake(group, currency)?;
			let tally_correction = account.reward_tally_correction(group, currency)?;

			let reward =
				base::Mechanism::claim_reward(&mut account.base, &currency.base, &group.base)?
					.ensure_sub(last_rewarded_stake)?;

			account
				.base
				.reward_tally
				.ensure_sub_assign(last_rewarded_stake.ensure_into()?)?
				.ensure_add_assign(tally_correction.ensure_into()?)?;

			Ok(reward)
		}

		fn move_currency(
			currency: &mut Self::Currency,
			prev_group: &mut Self::Group,
			next_group: &mut Self::Group,
		) -> Result<(), MechanismError> {
			base::Mechanism::<_, T::IBalance, _, _>::move_currency(
				&mut currency.base,
				&mut prev_group.base,
				&mut next_group.base,
			)?;

			// Only if there was a distribution from last move, we update the previous related data.
			if currency.next_distribution_id != prev_group.distribution_id {
				currency.prev_distribution_id = prev_group.distribution_id;
				currency.prev_last_rate = prev_group.last_rate;
			}
			currency.next_distribution_id = next_group.distribution_id;

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
