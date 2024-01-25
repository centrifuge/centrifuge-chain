//! Types with Config access. This module does not mutate FI storage

use cfg_traits::{
	investments::{Investment, TrancheCurrency},
	IdentityCurrencyConversion, PoolInspect,
};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest, Swap,
};
use frame_support::{dispatch::DispatchResult, ensure};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureAdd, EnsureAddAssign, EnsureSub, EnsureSubAssign, Saturating, Zero},
	ArithmeticError, DispatchError,
};

use crate::{
	pallet::{Config, Error},
	swaps::Swaps,
	Action, SwapOf,
};

/// Get the pool currency associated to a investment_id
fn pool_currency_of<T: Config>(
	investment_id: T::InvestmentId,
) -> Result<T::CurrencyId, DispatchError> {
	T::PoolInspect::currency_for(investment_id.of_pool()).ok_or(Error::<T>::PoolNotFound.into())
}

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

	/// Total amount of pool currency increased for this investment
	pub increased_pool_amount: T::Balance,

	/// Total swapped amount pending to execute for decreasing the investment.
	/// Measured in foreign currency
	pub decrease_swapped_amount: T::Balance,
}

impl<T: Config> InvestmentInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(foreign_currency)?,
			increased_pool_amount: T::Balance::default(),
			decrease_swapped_amount: T::Balance::default(),
		})
	}

	pub fn pre_increase_swap(
		&mut self,
		investment_id: T::InvestmentId,
		foreign_amount: T::Balance,
	) -> Result<SwapOf<T>, DispatchError> {
		// NOTE: This line will be removed with market ratios
		let pool_amount = T::CurrencyConverter::stable_to_stable(
			pool_currency_of::<T>(investment_id)?,
			self.base.foreign_currency,
			foreign_amount,
		)?;

		self.increased_pool_amount.ensure_add_assign(pool_amount)?;

		Ok(Swap {
			currency_in: pool_currency_of::<T>(investment_id)?,
			currency_out: self.base.foreign_currency,
			amount_in: pool_amount,
		})
	}

	/// Decrease an investment taking into account that a previous increment
	/// could be pending
	pub fn pre_decrease_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::Balance,
	) -> Result<SwapOf<T>, DispatchError> {
		// NOTE: This line will be removed with market ratios
		let pool_amount = T::CurrencyConverter::stable_to_stable(
			pool_currency_of::<T>(investment_id)?,
			self.base.foreign_currency,
			foreign_amount,
		)?;

		self.increased_pool_amount
			.ensure_sub_assign(pool_amount)
			.map_err(|_| Error::<T>::TooMuchDecrease)?;

		let pool_currency = pool_currency_of::<T>(investment_id)?;
		let pending_pool_amount_increment =
			Swaps::<T>::pending_swap_amount(who, investment_id, pool_currency, Action::Investment);

		let decrement = pool_amount.saturating_sub(pending_pool_amount_increment);
		if !decrement.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_sub(decrement)?,
			)?;
		}

		Ok(Swap {
			currency_in: self.base.foreign_currency,
			currency_out: pool_currency_of::<T>(investment_id)?,
			amount_in: foreign_amount,
		})
	}

	/// Increase an investment taking into account that a previous decrement
	/// could be pending
	pub fn post_increase_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		pool_amount: T::Balance,
	) -> DispatchResult {
		self.decrease_swapped_amount = T::Balance::default();

		if !pool_amount.is_zero() {
			T::Investment::update_investment(
				&who,
				investment_id,
				T::Investment::investment(&who, investment_id)?.ensure_add(pool_amount)?,
			)?;
		}

		Ok(())
	}

	pub fn post_decrease_swap(
		&mut self,
		investment_id: T::InvestmentId,
		swapped_amount: T::Balance,
		pending_amount: T::Balance,
	) -> Result<Option<ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>>, DispatchError> {
		self.decrease_swapped_amount
			.ensure_add_assign(swapped_amount)?;

		if pending_amount.is_zero() {
			// NOTE: How make this works with market ratios?
			let remaining_foreign_amount = T::CurrencyConverter::stable_to_stable(
				self.base.foreign_currency,
				pool_currency_of::<T>(investment_id)?,
				self.remaining_pool_amount()?,
			)?;

			let msg = ExecutedForeignDecreaseInvest {
				amount_decreased: self.decrease_swapped_amount,
				foreign_currency: self.base.foreign_currency,
				amount_remaining: remaining_foreign_amount,
			};

			self.decrease_swapped_amount = T::Balance::default();

			return Ok(Some(msg));
		}

		Ok(None)
	}

	pub fn post_collect(
		&mut self,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::Balance>,
	) -> Result<ExecutedForeignCollect<T::Balance, T::CurrencyId>, DispatchError> {
		self.base.collected.increase(&collected)?;

		// NOTE: How make this works with market ratios?
		let remaining_foreign_amount = T::CurrencyConverter::stable_to_stable(
			self.base.foreign_currency,
			pool_currency_of::<T>(investment_id)?,
			self.remaining_pool_amount()?,
		)?;

		// NOTE: How make this works with market ratios?
		let collected_foreign_amount = T::CurrencyConverter::stable_to_stable(
			self.base.foreign_currency,
			pool_currency_of::<T>(investment_id)?,
			collected.amount_payment,
		)?;

		let msg = ExecutedForeignCollect {
			currency: self.base.foreign_currency,
			amount_currency_payout: collected_foreign_amount,
			amount_tranche_tokens_payout: collected.amount_collected,
			amount_remaining: remaining_foreign_amount,
		};

		Ok(msg)
	}

	pub fn is_completed(&self) -> Result<bool, DispatchError> {
		Ok(self.decrease_swapped_amount.is_zero() && self.remaining_pool_amount()?.is_zero())
	}

	pub fn remaining_pool_amount(&self) -> Result<T::Balance, ArithmeticError> {
		self.increased_pool_amount
			.ensure_sub(self.base.collected.amount_payment)
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

	pub fn post_collect_and_pre_swap(
		&mut self,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::Balance>,
	) -> Result<SwapOf<T>, DispatchError> {
		self.base.collected.increase(&collected)?;

		Ok(Swap {
			currency_in: self.base.foreign_currency,
			currency_out: pool_currency_of::<T>(investment_id)?,
			amount_in: collected.amount_collected,
		})
	}

	pub fn post_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_amount: T::Balance,
		pending_amount: T::Balance,
	) -> Result<Option<ExecutedForeignCollect<T::Balance, T::CurrencyId>>, DispatchError> {
		self.swapped_amount.ensure_add_assign(swapped_amount)?;
		if pending_amount.is_zero() {
			let redemption = T::Investment::redemption(&who, investment_id)?;

			let msg = ExecutedForeignCollect {
				currency: self.base.foreign_currency,
				amount_currency_payout: self.swapped_amount,
				amount_tranche_tokens_payout: self.collected_tranche_tokens(),
				amount_remaining: redemption,
			};

			self.base.collected = CollectedAmount::default();
			self.swapped_amount = T::Balance::default();

			return Ok(Some(msg));
		}

		Ok(None)
	}

	pub fn is_completed(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<bool, DispatchError> {
		Ok(T::Investment::redemption(&who, investment_id)?.is_zero()
			&& self.collected_tranche_tokens().is_zero())
	}

	pub fn collected_tranche_tokens(&self) -> T::Balance {
		self.base.collected.amount_payment
	}
}
