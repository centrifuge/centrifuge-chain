//! Types with Config access. This module does not mutate FI storage

use cfg_traits::{investments::Investment, TokenSwaps};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest, Swap,
};
use frame_support::{dispatch::DispatchResult, ensure};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		EnsureAdd, EnsureAddAssign, EnsureDiv, EnsureMul, EnsureSub, EnsureSubAssign, Saturating,
		Zero,
	},
	DispatchError,
};
use sp_std::cmp::min;

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

	/// Pool invested amount, denominated in foreign currency.
	///
	/// Used to correlate the pool amount into foreign amount when the market
	/// conversion is not known upfront.
	pub invested_foreign_amount: T::Balance,

	/// Decrease pool amount pending to be swapped, denominated in foreign
	/// currency.
	///
	/// Used to correlate the pool amount into foreign amount when the market
	/// conversion is not known upfront.
	pub decrease_pending_foreign_amount: T::Balance,

	/// Total decrease swapped amount pending to execute.
	/// It accumulates different partial swaps.
	pub decrease_swapped_foreign_amount: T::Balance,
}

impl<T: Config> InvestmentInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(foreign_currency)?,
			invested_foreign_amount: T::Balance::default(),
			decrease_pending_foreign_amount: T::Balance::default(),
			decrease_swapped_foreign_amount: T::Balance::default(),
		})
	}

	/// This method is performed before applying the swap.
	pub fn pre_increase_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::Balance,
	) -> Result<(SwapOf<T>, bool), DispatchError> {
		let pool_currency = pool_currency_of::<T>(investment_id)?;

		// It's ok to use the market ratio because this amount will be
		// cancelled.
		let decreasing_foreign_amount = T::TokenSwaps::convert_by_market(
			self.base.foreign_currency,
			pool_currency,
			self.pending_decrease_swap(who, investment_id)?,
		)?;

		let mut send_msg = false;
		if foreign_amount >= decreasing_foreign_amount {
			let swap_foreign_amount = foreign_amount.ensure_sub(decreasing_foreign_amount)?;

			self.decrease_swapped_foreign_amount = self
				.decrease_swapped_foreign_amount
				.saturating_sub(swap_foreign_amount);

			if !self.decrease_swapped_foreign_amount.is_zero() {
				send_msg = true;
			}
		}

		Ok((
			Swap {
				currency_in: pool_currency,
				currency_out: self.base.foreign_currency,
				amount_out: foreign_amount,
			},
			send_msg,
		))
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
		// We do not want to decrease the whole `foreign_amount` from the investment
		// amount if there is a pending investment swap.
		let increasing_foreign_amount = self.pending_increase_swap(who, investment_id)?;
		let foreign_investment_decrement = foreign_amount.saturating_sub(increasing_foreign_amount);

		let mut pool_investment_decrement = T::Balance::default();
		if !foreign_investment_decrement.is_zero() {
			let invested_pool_amount = T::Investment::investment(who, investment_id)?;

			pool_investment_decrement = foreign_investment_decrement
				.ensure_mul(invested_pool_amount)?
				.ensure_div(self.invested_foreign_amount)
				.map_err(|_| Error::<T>::TooMuchDecrease)?;

			T::Investment::update_investment(
				who,
				investment_id,
				invested_pool_amount
					.ensure_sub(pool_investment_decrement)
					.map_err(|_| Error::<T>::TooMuchDecrease)?,
			)?;

			self.invested_foreign_amount
				.ensure_sub_assign(foreign_investment_decrement)?;

			self.decrease_pending_foreign_amount
				.ensure_add_assign(foreign_investment_decrement)?;
		}

		let pool_currency = pool_currency_of::<T>(investment_id)?;

		// It's ok to use the market ratio because this amount will be
		// cancelled.
		let increasing_pool_amount = T::TokenSwaps::convert_by_market(
			pool_currency,
			self.base.foreign_currency,
			min(foreign_amount, increasing_foreign_amount),
		)?;

		Ok(Swap {
			currency_in: self.base.foreign_currency,
			currency_out: pool_currency,
			amount_out: increasing_pool_amount.ensure_add(pool_investment_decrement)?,
		})
	}

	/// Increase an investment taking into account that a previous decrement
	/// could be pending.
	/// This method is performed after resolve the swap.
	#[allow(clippy::type_complexity)]
	pub fn post_increase_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_pool_amount: T::Balance,
		swapped_foreign_amount: T::Balance,
		send_decrease_msg: bool,
	) -> Result<Option<ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>>, DispatchError> {
		if !swapped_pool_amount.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_add(swapped_pool_amount)?,
			)?;

			self.invested_foreign_amount
				.ensure_add_assign(swapped_foreign_amount)?;

			self.decrease_pending_foreign_amount = self
				.decrease_pending_foreign_amount
				.saturating_sub(swapped_foreign_amount);

			if send_decrease_msg {
				return self.generate_decrease_msg(who, investment_id).map(Some);
			}
		}

		Ok(None)
	}

	/// This method is performed after resolve the swap.
	#[allow(clippy::type_complexity)]
	pub fn post_decrease_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_foreign_amount: T::Balance,
		swapped_pool_amount: T::Balance,
		pending_pool_amount: T::Balance,
	) -> Result<Option<ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>>, DispatchError> {
		self.decrease_swapped_foreign_amount
			.ensure_add_assign(swapped_foreign_amount)?;

		self.decrease_pending_foreign_amount = pending_pool_amount
			.ensure_mul(swapped_foreign_amount)?
			.ensure_div(swapped_pool_amount)?;

		if pending_pool_amount.is_zero() {
			return self.generate_decrease_msg(who, investment_id).map(Some);
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

		let collected_pool_amount = collected.amount_payment;

		let previous_pool_amount_in_system =
			T::Investment::investment(who, investment_id)?.ensure_add(collected_pool_amount)?;

		let collected_foreign_amount = collected_pool_amount
			.ensure_mul(self.invested_foreign_amount)?
			.ensure_div(previous_pool_amount_in_system)?;

		self.invested_foreign_amount
			.ensure_sub_assign(collected_foreign_amount)?;

		Ok(ExecutedForeignCollect {
			currency: self.base.foreign_currency,
			amount_currency_payout: collected_foreign_amount,
			amount_tranche_tokens_payout: collected.amount_collected,
			amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
		})
	}

	/// Remaining amount to finalize the investment, denominated in foreign
	/// currency. It takes care of:
	/// - Any invested amount.
	/// - Any increase pending amount to be swapped.
	/// - Any decrease pending amount to be swapped.
	/// - Any decrease swapped amount.
	fn remaining_foreign_amount(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		Ok(self
			.invested_foreign_amount
			.ensure_add(self.pending_increase_swap(who, investment_id)?)?
			.ensure_add(self.decrease_pending_foreign_amount)?
			.ensure_add(self.decrease_swapped_foreign_amount)?)
	}

	/// In foreign currency denomination
	pub fn pending_increase_swap(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		Ok(Swaps::<T>::pending_amount_for(
			who,
			investment_id,
			Action::Investment,
			self.base.foreign_currency,
		))
	}

	/// In foreign currency denomination
	pub fn pending_decrease_swap(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		Ok(Swaps::<T>::pending_amount_for(
			who,
			investment_id,
			Action::Investment,
			pool_currency_of::<T>(investment_id)?,
		))
	}

	pub fn generate_decrease_msg(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>, DispatchError> {
		Ok(ExecutedForeignDecreaseInvest {
			amount_decreased: sp_std::mem::take(&mut self.decrease_swapped_foreign_amount),
			foreign_currency: self.base.foreign_currency,
			amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
		})
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
