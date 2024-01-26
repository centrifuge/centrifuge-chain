//! Types with Config access. This module does not mutate FI storage

use cfg_traits::{investments::Investment, IdentityCurrencyConversion};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest, Swap,
};
use frame_support::{dispatch::DispatchResult, ensure};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureAdd, EnsureAddAssign, EnsureSub, Saturating, Zero},
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

	/// Total swapped amount pending to execute for decreasing the investment.
	/// Measured in foreign currency
	pub decrease_swapped_amount: T::Balance,
}

impl<T: Config> InvestmentInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(foreign_currency)?,
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

		// NOTE: This line will be removed with market ratios
		let pool_amount = T::CurrencyConverter::stable_to_stable(
			pool_currency,
			self.base.foreign_currency,
			foreign_amount,
		)?;

		Ok(Swap {
			currency_in: pool_currency,
			currency_out: self.base.foreign_currency,
			amount_in: pool_amount,
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

		// NOTE: This line will be removed with market ratios
		let pool_amount = T::CurrencyConverter::stable_to_stable(
			pool_currency,
			self.base.foreign_currency,
			foreign_amount,
		)?;

		let pending_pool_amount_increment =
			Swaps::<T>::pending_amount_for(who, investment_id, Action::Investment, pool_currency);

		let investment_decrement = pool_amount.saturating_sub(pending_pool_amount_increment);
		if !investment_decrement.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?
					.ensure_sub(investment_decrement)
					.map_err(|_| Error::<T>::TooMuchDecrease)?,
			)?;
		}

		Ok(Swap {
			currency_in: self.base.foreign_currency,
			currency_out: pool_currency,
			amount_in: foreign_amount,
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
				&who,
				investment_id,
				T::Investment::investment(&who, investment_id)?.ensure_add(pool_amount)?,
			)?;
		}

		Ok(())
	}

	/// This method is performed after resolve the swap.
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

	pub fn post_collect(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::Balance>,
	) -> Result<ExecutedForeignCollect<T::Balance, T::CurrencyId>, DispatchError> {
		self.base.collected.increase(&collected)?;

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
			amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
		};

		Ok(msg)
	}

	pub fn remaining_foreign_amount(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		let pending_swap = Swaps::<T>::any_pending_amount_demominated_in(
			who,
			investment_id,
			Action::Investment,
			self.base.foreign_currency,
		)?;

		// NOTE: How make this works with market ratios?
		let pending_invested = T::CurrencyConverter::stable_to_stable(
			self.base.foreign_currency,
			pool_currency_of::<T>(investment_id)?,
			T::Investment::investment(who, investment_id)?,
		)?;

		Ok(pending_swap
			.ensure_add(self.decrease_swapped_amount)?
			.ensure_add(pending_invested)?)
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
			let msg = ExecutedForeignCollect {
				currency: self.base.foreign_currency,
				amount_currency_payout: self.swapped_amount,
				amount_tranche_tokens_payout: self.collected_tranche_tokens(),
				amount_remaining: T::Investment::redemption(&who, investment_id)?,
			};

			self.base.collected = CollectedAmount::default();
			self.swapped_amount = T::Balance::default();

			return Ok(Some(msg));
		}

		Ok(None)
	}

	pub fn collected_tranche_tokens(&self) -> T::Balance {
		self.base.collected.amount_payment
	}

	pub fn is_completed(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<bool, DispatchError> {
		Ok(T::Investment::redemption(&who, investment_id)?.is_zero()
			&& self.collected_tranche_tokens().is_zero())
	}
}
