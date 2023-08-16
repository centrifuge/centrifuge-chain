use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
pub use pallet::*;
use sp_runtime::{
	traits::{
		EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureFrom, EnsureInto, EnsureSub,
		EnsureSubAssign, One, Saturating, Zero,
	},
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
	fn was_distribution(&self, group: &Group<T>, currency: &Currency<T>) -> bool {
		self.distribution_id != group.distribution_id
			|| (self.last_currency_movement as usize) < currency.rpt_changes.len()
	}

	fn reward_tally_updated(
		&self,
		group: &Group<T>,
		currency: &Currency<T>,
	) -> Result<T::IBalance, DispatchError> {
		let reward_tally = if self.was_distribution(group, currency) {
			let correct_rpt = RptHistory::<T>::get(self.distribution_id).ok_or(
				DispatchError::Other("'DistributionId' not found in 'RptHistory'"),
			)?;

			self.reward_tally.ensure_add(
				correct_rpt
					.ensure_mul_int(self.pending_stake)?
					.ensure_into()?,
			)?
		} else {
			self.reward_tally
		};

		let tally_rpt_changes = self.get_tally_from_rpt_changes(group, currency)?;
		Ok(reward_tally.ensure_add(tally_rpt_changes)?)
	}

	fn stake_updated(
		&self,
		group: &Group<T>,
		currency: &Currency<T>,
	) -> Result<T::Balance, ArithmeticError> {
		if self.was_distribution(group, currency) {
			self.stake.ensure_add(self.pending_stake)
		} else {
			Ok(self.stake)
		}
	}

	fn update(&mut self, group: &Group<T>, currency: &Currency<T>) -> Result<(), DispatchError> {
		if self.was_distribution(group, currency) {
			let stake = self.stake_updated(group, currency)?;
			let reward_tally = self.reward_tally_updated(group, currency)?;

			self.stake = stake;
			self.reward_tally = reward_tally;
			self.pending_stake = T::Balance::zero();
		}

		self.last_currency_movement = currency.rpt_changes.len().ensure_into()?;
		self.distribution_id = group.distribution_id;

		Ok(())
	}

	fn get_tally_from_rpt_changes(
		&self,
		group: &Group<T>,
		currency: &Currency<T>,
	) -> Result<T::IBalance, ArithmeticError> {
		let rpt_to_apply = &currency.rpt_changes[self.last_currency_movement as usize..]
			.iter()
			.try_fold(T::Rate::zero(), |a, b| a.ensure_add(*b))?;

		let stake = self.stake_updated(group, currency)?;

		rpt_to_apply.ensure_mul_int(T::IBalance::ensure_from(stake)?)
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<T: Config> {
	total_stake: T::Balance,
	rpt_changes: BoundedVec<T::Rate, T::MaxCurrencyMovements>,
}

impl<T: Config> Default for Currency<T> {
	fn default() -> Self {
		Self {
			total_stake: T::Balance::zero(),
			rpt_changes: BoundedVec::default(),
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

		type MaxCurrencyMovements: Get<u32>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type RptHistory<T: Config> =
		StorageMap<_, Blake2_128Concat, T::DistributionId, T::Rate>;

	#[pallet::storage]
	pub(super) type LastDistributionId<T: Config> = StorageValue<_, T::DistributionId, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		// Emits when a currency is moved but any account associated has pending stake.
		// Currency movement is only allowed after a distribution, with no deposit/withdraw stake
		// from any participant.
		TryMovementAfterPendingState,
	}

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
			group.total_stake > Self::Balance::zero()
		}

		fn reward_group(
			group: &mut Self::Group,
			amount: Self::Balance,
		) -> Result<Self::Balance, DispatchError> {
			let mut reward_used = Self::Balance::zero();

			if group.total_stake > T::Balance::zero() {
				let rate = T::Rate::ensure_from_rational(amount, group.total_stake)?;
				group.rpt.ensure_add_assign(rate)?;

				reward_used = amount;
			}

			group
				.total_stake
				.ensure_add_assign(group.pending_total_stake)?;
			group.pending_total_stake = T::Balance::zero();

			RptHistory::<T>::insert(group.distribution_id, group.rpt);

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
			account.update(group, currency)?;

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
		) -> DispatchResult {
			account.update(group, currency)?;

			let pending_amount = amount.min(account.pending_stake);

			account.pending_stake.ensure_sub_assign(pending_amount)?;
			group
				.pending_total_stake
				.ensure_sub_assign(pending_amount)?;

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
		) -> Result<Self::Balance, DispatchError> {
			let stake = account.stake_updated(group, currency)?;
			let reward_tally = account.reward_tally_updated(group, currency)?;

			T::IBalance::ensure_from(group.rpt.ensure_mul_int(stake)?)?
				.ensure_sub(reward_tally)?
				.ensure_into()
				.map_err(|e| e.into())
		}

		fn claim_reward(
			account: &mut Self::Account,
			currency: &Self::Currency,
			group: &Self::Group,
		) -> Result<Self::Balance, DispatchError> {
			account.update(group, currency)?;

			let reward = Self::compute_reward(account, currency, group)?;

			account
				.reward_tally
				.ensure_add_assign(reward.ensure_into()?)?;

			Ok(reward)
		}

		fn move_currency(
			currency: &mut Self::Currency,
			from_group: &mut Self::Group,
			to_group: &mut Self::Group,
		) -> Result<(), MoveCurrencyError> {
			if from_group.pending_total_stake > T::Balance::zero() {
				Err(DispatchError::from(
					Error::<T>::TryMovementAfterPendingState,
				))?;
			}

			let rpt_change = to_group.rpt.ensure_sub(from_group.rpt)?;

			currency
				.rpt_changes
				.try_push(rpt_change)
				.map_err(|_| MoveCurrencyError::MaxMovements)?;

			from_group
				.total_stake
				.ensure_sub_assign(currency.total_stake)?;

			to_group
				.total_stake
				.ensure_add_assign(currency.total_stake)?;

			Ok(())
		}

		fn account_stake(account: &Self::Account) -> Self::Balance {
			account.stake.saturating_add(account.pending_stake)
		}

		fn group_stake(group: &Self::Group) -> Self::Balance {
			group.total_stake.saturating_add(group.pending_total_stake)
		}
	}
}
