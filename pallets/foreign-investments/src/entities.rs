//! Types with Config access. This module does not mutate FI storage

use cfg_traits::investments::Investment;
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest, Swap,
};
use frame_support::{dispatch::DispatchResult, ensure};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureSub, EnsureSubAssign, Saturating,
		Zero,
	},
	DispatchError,
};

use crate::{
	pallet::{Config, Error},
	pool_currency_of,
	swaps::Swaps,
	Action, SwapOf,
};

/// Hold the base information of a foreign investment/redemption
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct BaseInfo<T: Config> {
	pub foreign_currency: T::CurrencyId,
	pub collected: CollectedAmount<T::Balance>,
}

impl<T: Config> BaseInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			foreign_currency,
			collected: CollectedAmount::default(),
		})
	}

	pub fn ensure_same_foreign(&self, foreign_currency: T::CurrencyId) -> DispatchResult {
		ensure!(
			self.foreign_currency == foreign_currency,
			Error::<T>::MismatchedForeignCurrency
		);

		Ok(())
	}
}

/// Hold the information of a foreign investment
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InvestmentInfo<T: Config> {
	/// General info
	pub base: BaseInfo<T>,

	/// Foreign amount that has been increased but not decreased or collected.
	/// It's the foreign amount that is consider in the system to make the
	/// investment.
	pub pending_foreign_amount: T::Balance,

	/// Total swapped amount pending to execute for decreasing the investment.
	/// Measured in foreign currency
	pub decrease_swapped_amount: T::Balance,
}

impl<T: Config> InvestmentInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(foreign_currency)?,
			pending_foreign_amount: T::Balance::default(),
			decrease_swapped_amount: T::Balance::default(),
		})
	}

	/// This method is performed before applying the swap.
	pub fn pre_increase_swap(
		&mut self,
		investment_id: T::InvestmentId,
		foreign_amount: T::Balance,
	) -> Result<SwapOf<T>, DispatchError> {
		let pool_currency = pool_currency_of::<T>(investment_id)?;

		self.pending_foreign_amount
			.ensure_add_assign(foreign_amount)?;

		Ok(Swap {
			currency_in: pool_currency,
			currency_out: self.base.foreign_currency,
			amount_out: foreign_amount,
		})
	}

	/// Decrease an investment taking into account that a previous increment
	/// could be pending.
	/// This method is performed before applying the swap.
	pub fn pre_decrease_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::Balance,
	) -> Result<SwapOf<T>, DispatchError> {
		let pool_currency = pool_currency_of::<T>(investment_id)?;

		let pending_foreign_amount_increment = Swaps::<T>::pending_amount_for(
			who,
			investment_id,
			Action::Investment,
			self.base.foreign_currency,
		);

		// We do not want to decrease the whole `foreign_amount` from the investment
		// amount if there is a pending investment swap.
		let foreign_decrement = foreign_amount.saturating_sub(pending_foreign_amount_increment);
		if !foreign_decrement.is_zero() {
			let invested_pool_amount = T::Investment::investment(who, investment_id)?;

			// Get the proportion of pool_amount of this foreign decrement.
			let pool_decrement = T::BalanceRatio::ensure_from_rational(
				foreign_decrement,
				self.pending_foreign_amount,
			)
			.map_err(|_| Error::<T>::TooMuchDecrease)?
			.ensure_mul_int(invested_pool_amount)?;

			T::Investment::update_investment(
				who,
				investment_id,
				invested_pool_amount
					.ensure_sub(pool_decrement)
					.map_err(|_| Error::<T>::TooMuchDecrease)?,
			)?;
		}

		self.pending_foreign_amount
			.ensure_sub_assign(foreign_amount)?;

		Ok(Swap {
			currency_in: self.base.foreign_currency,
			currency_out: pool_currency,
			amount_out: foreign_amount,
		})
	}

	/// Increase an investment taking into account that a previous decrement
	/// could be pending.
	/// This method is performed after resolve the swap.
	pub fn post_increase_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		pool_amount: T::Balance,
	) -> DispatchResult {
		self.decrease_swapped_amount = T::Balance::default();

		if !pool_amount.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_add(pool_amount)?,
			)?;
		}

		Ok(())
	}

	/// This method is performed after resolve the swap.
	#[allow(clippy::type_complexity)]
	pub fn post_decrease_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_amount: T::Balance,
		pending_amount: T::Balance,
	) -> Result<Option<ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>>, DispatchError> {
		self.decrease_swapped_amount
			.ensure_add_assign(swapped_amount)?;

		if pending_amount.is_zero() {
			let amount_decreased = sp_std::mem::take(&mut self.decrease_swapped_amount);

			let msg = ExecutedForeignDecreaseInvest {
				amount_decreased,
				foreign_currency: self.base.foreign_currency,
				amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
			};

			return Ok(Some(msg));
		}

		Ok(None)
	}

	/// This method is performed after a collect
	pub fn post_collect(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::Balance>,
	) -> Result<ExecutedForeignCollect<T::Balance, T::CurrencyId>, DispatchError> {
		self.base.collected.increase(&collected)?;

		let pending_foreign_amount_increment = Swaps::<T>::pending_amount_for(
			who,
			investment_id,
			Action::Investment,
			self.base.foreign_currency,
		);

		// Get the proportion of foreign_amount of the pool collected amount.
		let collected_foreign_amount = T::BalanceRatio::ensure_from_rational(
			collected.amount_payment,
			T::Investment::investment(who, investment_id)?,
		)
		.map_err(|_| Error::<T>::TooMuchDecrease)?
		.ensure_mul_int(
			self.pending_foreign_amount
				.ensure_sub(pending_foreign_amount_increment)?,
		)?;

		self.pending_foreign_amount
			.ensure_sub_assign(collected_foreign_amount)?;

		let msg = ExecutedForeignCollect {
			currency: self.base.foreign_currency,
			amount_currency_payout: collected_foreign_amount,
			amount_tranche_tokens_payout: collected.amount_collected,
			amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
		};

		Ok(msg)
	}

	fn remaining_foreign_amount(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		let decrease_pending_amount = Swaps::<T>::pending_amount_for(
			who,
			investment_id,
			Action::Investment,
			self.base.foreign_currency,
		);

		Ok(self
			.pending_foreign_amount
			.ensure_add(self.decrease_swapped_amount)?
			.ensure_add(decrease_pending_amount)?)
	}

	pub fn is_completed(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<bool, DispatchError> {
		Ok(self.remaining_foreign_amount(who, investment_id)?.is_zero())
	}
}

/// Hold the information of an foreign redemption
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RedemptionInfo<T: Config> {
	/// General info
	pub base: BaseInfo<T>,

	/// Total swapped amount pending to execute.
	pub swapped_amount: T::Balance,
}

impl<T: Config> RedemptionInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(foreign_currency)?,
			swapped_amount: T::Balance::default(),
		})
	}

	pub fn increase(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::Balance,
	) -> DispatchResult {
		T::Investment::update_redemption(
			who,
			investment_id,
			T::Investment::redemption(who, investment_id)?.ensure_add(tranche_tokens_amount)?,
		)
	}

	pub fn decrease(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::Balance,
	) -> DispatchResult {
		T::Investment::update_redemption(
			who,
			investment_id,
			T::Investment::redemption(who, investment_id)?.ensure_sub(tranche_tokens_amount)?,
		)
	}

	/// This method is performed after a collect and before applying the swap
	pub fn post_collect_and_pre_swap(
		&mut self,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::Balance>,
	) -> Result<SwapOf<T>, DispatchError> {
		self.base.collected.increase(&collected)?;

		Ok(Swap {
			currency_in: self.base.foreign_currency,
			currency_out: pool_currency_of::<T>(investment_id)?,
			amount_out: collected.amount_collected,
		})
	}

	/// This method is performed after resolve the swap.
	#[allow(clippy::type_complexity)]
	pub fn post_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_amount: T::Balance,
		pending_amount: T::Balance,
	) -> Result<Option<ExecutedForeignCollect<T::Balance, T::CurrencyId>>, DispatchError> {
		self.swapped_amount.ensure_add_assign(swapped_amount)?;
		if pending_amount.is_zero() {
			let msg = ExecutedForeignCollect {
				currency: self.base.foreign_currency,
				amount_currency_payout: self.swapped_amount,
				amount_tranche_tokens_payout: self.collected_tranche_tokens(),
				amount_remaining: T::Investment::redemption(who, investment_id)?,
			};

			self.base.collected = CollectedAmount::default();
			self.swapped_amount = T::Balance::default();

			return Ok(Some(msg));
		}

		Ok(None)
	}

	fn collected_tranche_tokens(&self) -> T::Balance {
		self.base.collected.amount_payment
	}

	pub fn is_completed(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<bool, DispatchError> {
		Ok(T::Investment::redemption(who, investment_id)?.is_zero()
			&& self.collected_tranche_tokens().is_zero())
	}
}
