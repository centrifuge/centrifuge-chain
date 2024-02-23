//! Types with Config access. This module does not mutate FI storage

use cfg_traits::{
	investments::Investment,
	swaps::{Swap, Swaps},
};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest,
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
	pool_currency_of, Action, SwapOf,
};

/// Type used to be able to generate conversions from pool to foreign and
/// vice-verse without market ratios.
/// Both amounts are increased and decreased using the same values in each
/// currecies, maintaining always a correlation.
/// Any amount in pool or foreign currency can use this correlation to get its
/// representation in the opposite currency.
#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound)]
#[scale_info(skip_type_params(T))]
pub struct Correlation<T: Config> {
	pub pool_amount: T::PoolBalance,
	pub foreign_amount: T::ForeignBalance,
}

impl<T: Config> Correlation<T> {
	pub fn new(pool_amount: T::PoolBalance, foreign_amount: T::ForeignBalance) -> Self {
		Self {
			pool_amount,
			foreign_amount,
		}
	}

	/// Increase the correlate values.
	/// The difference between both values will affect the correlation
	pub fn increase(
		&mut self,
		pool_amount: T::PoolBalance,
		foreign_amount: T::ForeignBalance,
	) -> DispatchResult {
		self.pool_amount.ensure_add_assign(pool_amount)?;
		self.foreign_amount.ensure_add_assign(foreign_amount)?;

		Ok(())
	}

	/// Decrease a correlation
	/// The foreign amount amount is proportionally decreased
	pub fn decrease(&mut self, pool_amount: T::PoolBalance) -> DispatchResult {
		let foreign_amount = self.pool_to_foreign(pool_amount)?;

		self.pool_amount.ensure_sub_assign(pool_amount)?;
		self.foreign_amount.ensure_sub_assign(foreign_amount)?;

		Ok(())
	}

	/// Transform any pool amount into a foreign amount
	pub fn pool_to_foreign(
		&self,
		pool_amount: T::PoolBalance,
	) -> Result<T::ForeignBalance, DispatchError> {
		if pool_amount.is_zero() {
			return Ok(T::ForeignBalance::zero());
		}

		Ok(pool_amount
			.ensure_mul(self.foreign_amount.into())?
			.ensure_div(self.pool_amount)?
			.into())
	}

	/// Transform any foreign amount into a pool amount
	pub fn foreign_to_pool(
		&self,
		foreign_amount: T::ForeignBalance,
	) -> Result<T::PoolBalance, DispatchError> {
		if foreign_amount.is_zero() {
			return Ok(T::PoolBalance::zero());
		}

		Ok(foreign_amount
			.ensure_mul(self.pool_amount.into())?
			.ensure_div(self.foreign_amount)?
			.into())
	}
}

/// Hold the information of a foreign investment
#[derive(Clone, PartialEq, Eq, RuntimeDebugNoBound, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InvestmentInfo<T: Config> {
	/// Foreign currency of this investment
	pub foreign_currency: T::CurrencyId,

	/// Used to correlate the pool amount into foreign amount and vice-versa
	/// when the market conversion is not known upfront.
	///
	/// The correlation
	/// - is increased when an increase swap is paritally swapped
	/// - is decreased when a decrease swap is partially swapped.
	///
	/// Which can also be seen an addition of the following values:
	/// - The invested amount.
	/// - The pending decrease amount not swapped yet.
	pub correlation: Correlation<T>,

	/// Total decrease swapped amount pending to execute.
	/// It accumulates different partial swaps.
	pub decrease_swapped_foreign_amount: T::ForeignBalance,
}

impl<T: Config> InvestmentInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Self {
		Self {
			foreign_currency,
			correlation: Correlation::new(T::PoolBalance::zero(), T::ForeignBalance::zero()),
			decrease_swapped_foreign_amount: T::ForeignBalance::zero(),
		}
	}

	pub fn ensure_same_foreign(&self, foreign_currency: T::CurrencyId) -> DispatchResult {
		ensure!(
			self.foreign_currency == foreign_currency,
			Error::<T>::MismatchedForeignCurrency
		);

		Ok(())
	}

	/// This method is performed before applying the swap.
	pub fn pre_increase_swap(
		&mut self,
		_who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::ForeignBalance,
	) -> Result<SwapOf<T>, DispatchError> {
		Ok(Swap {
			currency_in: pool_currency_of::<T>(investment_id)?,
			currency_out: self.foreign_currency,
			amount_out: foreign_amount.into(),
		})
	}

	/// Decrease an investment taking into account that a previous increment
	/// could be pending.
	/// This method is performed before applying the swap.
	pub fn pre_decrease_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::ForeignBalance,
	) -> Result<SwapOf<T>, DispatchError> {
		let pool_currency = pool_currency_of::<T>(investment_id)?;

		// We do not want to decrease the whole `foreign_amount` from the investment
		// amount if there is a pending investment swap.
		let increasing_foreign_amount = self.pending_increase_swap(who, investment_id)?;
		let foreign_investment_decrement = foreign_amount.saturating_sub(increasing_foreign_amount);

		let pool_investment_decrement = self
			.correlation
			.foreign_to_pool(foreign_investment_decrement)
			.map_err(|e| match e {
				DispatchError::Arithmetic(ArithmeticError::DivisionByZero) => {
					Error::<T>::TooMuchDecrease.into()
				}
				e => e,
			})?;

		self.decrease_investment(who, investment_id, pool_investment_decrement)?;

		// It's ok to use the market ratio because this amount will be
		// cancelled in this instant.
		let increasing_pool_amount = T::Swaps::convert_by_market(
			pool_currency,
			self.foreign_currency,
			min(foreign_amount, increasing_foreign_amount).into(),
		)?;

		Ok(Swap {
			currency_in: self.foreign_currency,
			currency_out: pool_currency,
			amount_out: increasing_pool_amount.ensure_add(pool_investment_decrement.into())?,
		})
	}

	/// This method is performed after resolve the swap
	pub fn post_increase_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_pool_amount: T::PoolBalance,
		swapped_foreign_amount: T::ForeignBalance,
	) -> DispatchResult {
		self.correlation
			.increase(swapped_pool_amount, swapped_foreign_amount)?;

		self.increase_investment(who, investment_id, swapped_pool_amount)
	}

	/// This method is performed after resolve the swap by cancelling it
	#[allow(clippy::type_complexity)]
	pub fn post_increase_swap_by_cancel(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_pool_amount: T::PoolBalance,
		swapped_foreign_amount: T::ForeignBalance,
	) -> Result<
		Option<ExecutedForeignDecreaseInvest<T::ForeignBalance, T::CurrencyId>>,
		DispatchError,
	> {
		self.increase_investment(who, investment_id, swapped_pool_amount)?;

		self.decrease_swapped_foreign_amount
			.ensure_add_assign(swapped_foreign_amount)?;

		let no_pending_decrease = self.pending_decrease_swap(who, investment_id)?.is_zero();
		if no_pending_decrease && !self.decrease_swapped_foreign_amount.is_zero() {
			return Ok(Some(ExecutedForeignDecreaseInvest {
				foreign_currency: self.foreign_currency,
				amount_decreased: sp_std::mem::take(&mut self.decrease_swapped_foreign_amount),
				amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
			}));
		}

		Ok(None)
	}

	/// This method is performed after resolve the swap
	#[allow(clippy::type_complexity)]
	pub fn post_decrease_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_foreign_amount: T::ForeignBalance,
		swapped_pool_amount: T::PoolBalance,
		pending_pool_amount: T::PoolBalance,
	) -> Result<
		Option<ExecutedForeignDecreaseInvest<T::ForeignBalance, T::CurrencyId>>,
		DispatchError,
	> {
		self.correlation.decrease(swapped_pool_amount)?;

		self.post_decrease_swap_by_cancel(
			who,
			investment_id,
			swapped_foreign_amount,
			pending_pool_amount,
		)
	}

	/// This method is performed after resolve the swap by cancelling it
	#[allow(clippy::type_complexity)]
	pub fn post_decrease_swap_by_cancel(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_foreign_amount: T::ForeignBalance,
		pending_pool_amount: T::PoolBalance,
	) -> Result<
		Option<ExecutedForeignDecreaseInvest<T::ForeignBalance, T::CurrencyId>>,
		DispatchError,
	> {
		self.decrease_swapped_foreign_amount
			.ensure_add_assign(swapped_foreign_amount)?;

		if pending_pool_amount.is_zero() {
			return Ok(Some(ExecutedForeignDecreaseInvest {
				foreign_currency: self.foreign_currency,
				amount_decreased: sp_std::mem::take(&mut self.decrease_swapped_foreign_amount),
				amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
			}));
		}

		Ok(None)
	}

	/// This method is performed after a collect
	#[allow(clippy::type_complexity)]
	pub fn post_collect(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::TrancheBalance, T::PoolBalance>,
	) -> Result<
		ExecutedForeignCollect<
			T::ForeignBalance,
			T::TrancheBalance,
			T::ForeignBalance,
			T::CurrencyId,
		>,
		DispatchError,
	> {
		let collected_foreign_amount =
			self.correlation.pool_to_foreign(collected.amount_payment)?;

		self.correlation.decrease(collected.amount_payment)?;

		Ok(ExecutedForeignCollect {
			currency: self.foreign_currency,
			amount_currency_payout: collected_foreign_amount,
			amount_tranche_tokens_payout: collected.amount_collected,
			amount_remaining: self.remaining_foreign_amount(who, investment_id)?,
		})
	}

	fn increase_investment(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		pool_amount: T::PoolBalance,
	) -> DispatchResult {
		if !pool_amount.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_add(pool_amount)?,
			)?;
		}

		Ok(())
	}

	fn decrease_investment(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		pool_amount: T::PoolBalance,
	) -> DispatchResult {
		if !pool_amount.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?
					.ensure_sub(pool_amount)
					.map_err(|_| Error::<T>::TooMuchDecrease)?,
			)?;
		}

		Ok(())
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
	) -> Result<T::ForeignBalance, DispatchError> {
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
	) -> Result<T::ForeignBalance, DispatchError> {
		let swap_id = (investment_id, Action::Investment);
		Ok(T::Swaps::pending_amount(who, swap_id, self.foreign_currency)?.into())
	}

	/// In foreign currency denomination
	pub fn pending_decrease_swap(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::PoolBalance, DispatchError> {
		let swap_id = (investment_id, Action::Investment);
		let pool_currency = pool_currency_of::<T>(investment_id)?;
		Ok(T::Swaps::pending_amount(who, swap_id, pool_currency)?.into())
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
#[derive(Clone, PartialEq, Eq, RuntimeDebugNoBound, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct RedemptionInfo<T: Config> {
	/// Foreign currency of this redemption
	pub foreign_currency: T::CurrencyId,

	/// Total swapped amount pending to execute.
	pub swapped_amount: T::ForeignBalance,

	/// Total collected amount pending to be sent.
	pub collected: CollectedAmount<T::PoolBalance, T::TrancheBalance>,
}

impl<T: Config> RedemptionInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Self {
		Self {
			foreign_currency,
			swapped_amount: T::ForeignBalance::default(),
			collected: CollectedAmount::default(),
		}
	}

	pub fn ensure_same_foreign(&self, foreign_currency: T::CurrencyId) -> DispatchResult {
		ensure!(
			self.foreign_currency == foreign_currency,
			Error::<T>::MismatchedForeignCurrency
		);

		Ok(())
	}

	pub fn increase(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::TrancheBalance,
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
		tranche_tokens_amount: T::TrancheBalance,
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
		collected: CollectedAmount<T::PoolBalance, T::TrancheBalance>,
	) -> Result<SwapOf<T>, DispatchError> {
		self.collected.increase(&collected)?;

		Ok(Swap {
			currency_in: self.foreign_currency,
			currency_out: pool_currency_of::<T>(investment_id)?,
			amount_out: collected.amount_collected.into(),
		})
	}

	/// This method is performed after resolve the swap.
	#[allow(clippy::type_complexity)]
	pub fn post_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_amount: T::ForeignBalance,
		pending_amount: T::PoolBalance,
	) -> Result<
		Option<
			ExecutedForeignCollect<
				T::ForeignBalance,
				T::TrancheBalance,
				T::TrancheBalance,
				T::CurrencyId,
			>,
		>,
		DispatchError,
	> {
		self.swapped_amount.ensure_add_assign(swapped_amount)?;

		if pending_amount.is_zero() {
			let msg = ExecutedForeignCollect {
				currency: self.foreign_currency,
				amount_currency_payout: self.swapped_amount,
				amount_tranche_tokens_payout: self.collected_tranche_tokens(),
				amount_remaining: T::Investment::redemption(who, investment_id)?,
			};

			self.collected = CollectedAmount::default();
			self.swapped_amount = T::ForeignBalance::zero();

			return Ok(Some(msg));
		}

		Ok(None)
	}

	fn collected_tranche_tokens(&self) -> T::TrancheBalance {
		self.collected.amount_payment
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
