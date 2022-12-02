use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureFrom, EnsureInto, EnsureSub,
	EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
pub use pallet::*;
use sp_runtime::{
	traits::{One, Zero},
	ArithmeticError, FixedPointNumber, FixedPointOperand,
};

use super::{MoveCurrencyError, RewardMechanism};

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<T: Config> {
	total_stake: T::Balance,
	pending_total_stake: T::Balance,
	rpt: T::Rate,
	distribution_id: T::DistributionId,
}

impl<T: Config> Default for Group<T> {
	fn default() -> Self {
		Self {
			total_stake: T::Balance::zero(),
			pending_total_stake: T::Balance::zero(),
			rpt: T::Rate::zero(),
			distribution_id: T::DistributionId::default(),
		}
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<T: Config> {
	stake: T::Balance,
	reward_tally: T::IBalance,
	pending_stake: T::Balance,
	distribution_id: T::DistributionId,
	last_currency_movement: u16,
}

impl<T: Config> Default for Account<T> {
	fn default() -> Self {
		Self {
			stake: T::Balance::zero(),
			reward_tally: T::IBalance::zero(),
			pending_stake: T::Balance::zero(),
			distribution_id: T::DistributionId::default(),
			last_currency_movement: 0,
		}
	}
}

impl<T: Config> Account<T> {
	fn reward_tally_correction(
		&self,
		group: &Group<T>,
		currency: &Currency<T>,
	) -> Result<T::Balance, ArithmeticError> {
		if self.distribution_id != group.distribution_id
			&& (self.distribution_id != currency.prev_distribution_id
				|| group.distribution_id != currency.next_distribution_id)
		{
			println!(
				"get => account: {:?}, group: {:?}, prev: {:?}, next: {:?}",
				self.distribution_id,
				group.distribution_id,
				currency.prev_distribution_id,
				currency.next_distribution_id
			);
			let correct_rpt = RptHistory::<T>::get(self.distribution_id).unwrap();
			Ok(correct_rpt.ensure_mul_int(self.pending_stake)?)
		} else {
			Ok(Zero::zero())
		}
	}

	fn stake_updated(
		&self,
		group: &Group<T>,
		currency: &Currency<T>,
	) -> Result<T::Balance, ArithmeticError> {
		if self.distribution_id != group.distribution_id
			&& (self.distribution_id != currency.prev_distribution_id
				|| group.distribution_id != currency.next_distribution_id)
		{
			self.stake.ensure_add(self.pending_stake)
		} else {
			Ok(self.stake)
		}
	}

	fn update(&mut self, group: &Group<T>, currency: &Currency<T>) -> Result<(), ArithmeticError> {
		let reward_tally_correction = self
			.reward_tally_correction(group, currency)?
			.ensure_into()?;

		let stake = self.stake_updated(group, currency)?;

		self.reward_tally
			.ensure_add_assign(reward_tally_correction)?;
		self.stake = stake;
		self.distribution_id = group.distribution_id;

		Ok(())
	}

	fn get_tally_from_rpt_changes(
		&self,
		rpt_changes: &[T::Rate],
	) -> Result<T::IBalance, ArithmeticError> {
		let rpt_to_apply = &rpt_changes[self.last_currency_movement as usize..]
			.iter()
			.try_fold(T::Rate::zero(), |a, b| a.ensure_add(*b))?;

		rpt_to_apply.ensure_mul_int(T::IBalance::ensure_from(self.stake)?)
	}

	fn apply_rpt_changes(&mut self, rpt_changes: &[T::Rate]) -> Result<(), ArithmeticError> {
		let tally_to_apply = self.get_tally_from_rpt_changes(rpt_changes)?;

		self.reward_tally.ensure_add_assign(tally_to_apply)?;
		self.last_currency_movement = rpt_changes
			.len()
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

		Ok(())
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<T: Config> {
	total_stake: T::Balance,
	pending_total_stake: T::Balance,
	rpt_changes: BoundedVec<T::Rate, T::MaxCurrencyMovements>,
	prev_distribution_id: T::DistributionId,
	next_distribution_id: T::DistributionId,
	distribution_id: T::DistributionId,
}

impl<T: Config> Default for Currency<T> {
	fn default() -> Self {
		Self {
			total_stake: T::Balance::zero(),
			pending_total_stake: T::Balance::zero(),
			rpt_changes: BoundedVec::default(),
			prev_distribution_id: T::DistributionId::default(),
			next_distribution_id: T::DistributionId::default(),
			distribution_id: T::DistributionId::default(),
		}
	}
}

impl<T: Config> Currency<T> {
	fn update(&mut self, group: &Group<T>) -> Result<(), ArithmeticError> {
		if self.distribution_id != group.distribution_id {
			self.total_stake
				.ensure_add_assign(self.pending_total_stake)?;
			self.pending_total_stake = T::Balance::zero();
			self.distribution_id = group.distribution_id;
		}

		Ok(())
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
		type InitialGroup = GetDefault;
		type MaxCurrencyMovements = T::MaxCurrencyMovements;

		fn reward_group(
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> Result<(), ArithmeticError> {
			if group.total_stake > T::Balance::zero() {
				let rate = T::Rate::ensure_from_rational(amount, group.total_stake)?;
				group.rpt.ensure_add_assign(rate)?;
			}

			group
				.total_stake
				.ensure_add_assign(group.pending_total_stake)?;
			group.pending_total_stake = T::Balance::zero();

			println!("insert: {:?}", group.distribution_id);
			RptHistory::<T>::insert(group.distribution_id, group.rpt);

			group.distribution_id = LastDistributionId::<T>::try_mutate(|distribution_id| {
				distribution_id.ensure_add_assign(One::one())?;
				Ok(*distribution_id)
			})?;

			Ok(())
		}

		fn deposit_stake(
			account: &mut Self::Account,
			currency: &mut Self::Currency,
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> Result<(), ArithmeticError> {
			account.update(group, currency)?;
			account.apply_rpt_changes(&currency.rpt_changes)?;
			currency.update(group)?;

			account.pending_stake.ensure_add_assign(amount)?;
			group.pending_total_stake.ensure_add_assign(amount)?;
			currency.total_stake.ensure_add_assign(amount)?;

			Ok(())
		}

		fn withdraw_stake(
			account: &mut Self::Account,
			currency: &mut Self::Currency,
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> Result<(), ArithmeticError> {
			account.update(group, currency)?;
			account.apply_rpt_changes(&currency.rpt_changes)?;
			currency.update(group)?;

			let pending_amount = amount.min(account.pending_stake);

			account.pending_stake.ensure_add_assign(pending_amount)?;
			group
				.pending_total_stake
				.ensure_add_assign(pending_amount)?;

			let computed_amount = amount.ensure_sub(pending_amount)?;

			account.stake.ensure_sub_assign(computed_amount)?;
			account
				.reward_tally
				.ensure_sub_assign(group.rpt.ensure_mul_int(computed_amount)?.ensure_into()?)?;
			group.total_stake.ensure_sub_assign(computed_amount)?;

			currency.total_stake.ensure_sub_assign(amount)?;

			Ok(())
		}

		fn compute_reward(
			account: &Self::Account,
			currency: &Self::Currency,
			group: &Self::Group,
		) -> Result<Self::Balance, ArithmeticError> {
			let stake = account.stake_updated(group, currency)?;
			let tally = account.reward_tally.ensure_add(
				account
					.reward_tally_correction(group, currency)?
					.ensure_into()?,
			)?;

			T::IBalance::ensure_from(group.rpt.ensure_mul_int(stake)?)?
				.ensure_add(tally)?
				.ensure_sub(account.get_tally_from_rpt_changes(&currency.rpt_changes)?)?
				.ensure_into()
		}

		fn claim_reward(
			account: &mut Self::Account,
			currency: &Self::Currency,
			group: &Self::Group,
		) -> Result<Self::Balance, ArithmeticError> {
			account.update(group, currency)?;
			account.apply_rpt_changes(&currency.rpt_changes)?;

			let reward = Self::compute_reward(&account, currency, group)?;

			account
				.reward_tally
				.ensure_add_assign(reward.ensure_into()?)?;

			Ok(reward)
		}

		fn move_currency(
			currency: &mut Self::Currency,
			prev_group: &mut Self::Group,
			next_group: &mut Self::Group,
		) -> Result<(), MoveCurrencyError> {
			if currency.distribution_id == prev_group.distribution_id {
				currency.distribution_id = next_group.distribution_id;
			} else {
				currency.update(next_group)?;
			}

			let rpt_change = next_group.rpt.ensure_sub(prev_group.rpt)?;

			currency
				.rpt_changes
				.try_push(rpt_change)
				.map_err(|_| MoveCurrencyError::MaxMovements)?;

			prev_group
				.total_stake
				.ensure_sub_assign(currency.total_stake)?;

			prev_group
				.pending_total_stake
				.ensure_sub_assign(currency.pending_total_stake)?;

			next_group
				.total_stake
				.ensure_add_assign(currency.total_stake)?;

			next_group
				.pending_total_stake
				.ensure_add_assign(currency.pending_total_stake)?;

			// Only if there was a distribution from last move, we update the previous related data.
			if currency.next_distribution_id != prev_group.distribution_id {
				currency.prev_distribution_id = prev_group.distribution_id;
			}
			currency.next_distribution_id = next_group.distribution_id;

			Ok(())
		}

		fn account_stake(account: &Self::Account) -> Self::Balance {
			account.stake + account.pending_stake //TODO: check arithmetics
		}

		fn group_stake(group: &Self::Group) -> Self::Balance {
			group.total_stake + group.pending_total_stake // TODO: check arithmetics
		}
	}
}
