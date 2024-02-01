//! Types with Config access. This module does not mutate FI storage

use cfg_traits::{investments::Investment, TokenSwaps};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest, Swap,
};
use frame_support::{dispatch::DispatchResult, ensure, RuntimeDebugNoBound};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		EnsureAdd, EnsureAddAssign, EnsureDiv, EnsureMul, EnsureSub, EnsureSubAssign, Saturating,
		Zero,
	},
	ArithmeticError, DispatchError,
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

/// Type used to be able to generate conversions from pool to foreign and
/// vice-verse without market ratios.
/// Both amounts are increased and decreased using the same values in each
/// currecies, maintaining always a correlation.
/// Any amount in pool or foreign currency can use this correlation to get its
/// representation in the opposite currency.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound)]
#[scale_info(skip_type_params(T))]
pub struct Correlation<T: Config> {
	pub pool_amount: T::Balance,
	pub foreign_amount: T::Balance,
}

impl<T: Config> Correlation<T> {
	pub fn new(pool_amount: T::Balance, foreign_amount: T::Balance) -> Self {
		Self {
			pool_amount,
			foreign_amount,
		}
	}

	/// Increase the correlate values.
	/// The difference between both values will affect the correlation
	pub fn increase(
		&mut self,
		pool_amount: T::Balance,
		foreign_amount: T::Balance,
	) -> DispatchResult {
		self.pool_amount.ensure_add_assign(pool_amount)?;
		self.foreign_amount.ensure_add_assign(foreign_amount)?;

		Ok(())
	}

	/// Decrease a correlation
	/// The foreign amount amount is proportionally decreased
	pub fn decrease(&mut self, pool_amount: T::Balance) -> DispatchResult {
		let foreign_amount = self.pool_to_foreign(pool_amount)?;

		self.pool_amount.ensure_sub_assign(pool_amount)?;
		self.foreign_amount.ensure_sub_assign(foreign_amount)?;

		Ok(())
	}

	/// Transform any pool amount into a foreign amount
	pub fn pool_to_foreign(&self, pool_amount: T::Balance) -> Result<T::Balance, DispatchError> {
		Ok(pool_amount
			.ensure_mul(self.foreign_amount)?
			.ensure_div(self.pool_amount)?)
	}

	/// Transform any foreign amount into a pool amount
	pub fn foreign_to_pool(&self, foreign_amount: T::Balance) -> Result<T::Balance, DispatchError> {
		Ok(foreign_amount
			.ensure_mul(self.pool_amount)?
			.ensure_div(self.foreign_amount)?)
	}
}

/// Hold the information of a foreign investment
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InvestmentInfo<T: Config> {
	/// General info
	pub base: BaseInfo<T>,

	/// Used to correlate the pool amount into foreign amount and vice-versa
	/// when the market conversion is not known upfront.
	///
	/// The correlation is increased & decreased to be have the following
	/// values:
	/// - The invested amount.
	/// - The pending decrease amount not swapped yet.
	pub correlation: Correlation<T>,

	/// Total decrease swapped amount pending to execute.
	/// It accumulates different partial swaps.
	pub decrease_swapped_foreign_amount: T::Balance,
}

impl<T: Config> InvestmentInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Result<Self, DispatchError> {
		Ok(Self {
			base: BaseInfo::new(foreign_currency)?,
			correlation: Correlation::new(T::Balance::default(), T::Balance::default()),
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

		let mut send_decrease_msg = false;
		if foreign_amount >= decreasing_foreign_amount {
			let swap_foreign_amount = foreign_amount.ensure_sub(decreasing_foreign_amount)?;

			self.decrease_swapped_foreign_amount = self
				.decrease_swapped_foreign_amount
				.saturating_sub(swap_foreign_amount);

			if !self.decrease_swapped_foreign_amount.is_zero() {
				// The message must be sent in the "post" stage to compute the correct
				// amount_remaining
				send_decrease_msg = true;
			}
		}

		Ok((
			Swap {
				currency_in: pool_currency,
				currency_out: self.base.foreign_currency,
				amount_out: foreign_amount,
			},
			send_decrease_msg,
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

			pool_investment_decrement = self
				.correlation
				.foreign_to_pool(foreign_investment_decrement)
				.map_err(|e| match e {
					DispatchError::Arithmetic(ArithmeticError::DivisionByZero) => {
						Error::<T>::TooMuchDecrease.into()
					}
					e => e,
				})?;

			T::Investment::update_investment(
				who,
				investment_id,
				invested_pool_amount
					.ensure_sub(pool_investment_decrement)
					.map_err(|_| Error::<T>::TooMuchDecrease)?,
			)?;
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
		from_cancelation: bool,
	) -> Result<Option<ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>>, DispatchError> {
		if !swapped_pool_amount.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_add(swapped_pool_amount)?,
			)?;

			if !from_cancelation {
				self.correlation
					.increase(swapped_pool_amount, swapped_foreign_amount)?;
			}

			if send_decrease_msg {
				return Ok(Some(ExecutedForeignDecreaseInvest {
					amount_decreased: sp_std::mem::take(&mut self.decrease_swapped_foreign_amount),
					foreign_currency: self.base.foreign_currency,
					amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
				}));
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
		from_cancelation: bool,
	) -> Result<Option<ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>>, DispatchError> {
		self.decrease_swapped_foreign_amount
			.ensure_add_assign(swapped_foreign_amount)?;

		if !from_cancelation {
			self.correlation.decrease(swapped_pool_amount)?;
		}

		if pending_pool_amount.is_zero() {
			return Ok(Some(ExecutedForeignDecreaseInvest {
				amount_decreased: sp_std::mem::take(&mut self.decrease_swapped_foreign_amount),
				foreign_currency: self.base.foreign_currency,
				amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
			}));
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

		let collected_foreign_amount =
			self.correlation.pool_to_foreign(collected.amount_payment)?;

		self.correlation.decrease(collected.amount_payment)?;

		Ok(ExecutedForeignCollect {
			currency: self.base.foreign_currency,
			amount_currency_payout: collected_foreign_amount,
			amount_tranche_tokens_payout: collected.amount_collected,
			amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
		})
	}

	/// Remaining amount to finalize the investment, denominated in foreign
	/// currency. It takes care of:
	/// - Any investment amount
	/// - Any increase pending amount to be swapped
	/// - Any decrease pending amount to be swapped.
	/// - Any decrease swapped amount.
	fn remaining_foreign_amount(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		let investment_and_pending_decrease = self.correlation.foreign_amount;
		Ok(investment_and_pending_decrease
			.ensure_add(self.pending_increase_swap(who, investment_id)?)?
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
