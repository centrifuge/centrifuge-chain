//! Types with Config access. This module does not mutate FI storage

use cfg_traits::{
	investments::Investment,
	swaps::{Swap, Swaps},
};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollectInvest, ExecutedForeignCollectRedeem,
	ExecutedForeignDecreaseInvest,
};
use frame_support::{dispatch::DispatchResult, ensure, RuntimeDebugNoBound};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureAdd, EnsureAddAssign, EnsureDiv, EnsureMul, EnsureSubAssign, Zero},
	DispatchError,
};

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
	pub fn decrease(
		&mut self,
		pool_amount: T::PoolBalance,
	) -> Result<T::ForeignBalance, DispatchError> {
		let mut foreign_amount = self.pool_to_foreign(pool_amount)?;

		self.pool_amount.ensure_sub_assign(pool_amount)?;

		// If the pool amount is zero we consider the foreign amount must also be zero
		// even if maths don't give us a zero (due some precision-lost)
		if self.pool_amount.is_zero() {
			foreign_amount = self.foreign_amount;
		}

		self.foreign_amount.ensure_sub_assign(foreign_amount)?;
		Ok(foreign_amount)
	}

	pub fn decrease_all(&mut self) -> T::ForeignBalance {
		let foreign_amount = self.foreign_amount;
		self.pool_amount = Zero::zero();
		self.foreign_amount = Zero::zero();
		foreign_amount
	}

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
	/// - is decreased when a decrease swap is fully swapped or an amount is
	///   collected.
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

	pub fn ensure_no_pending_cancel(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> DispatchResult {
		ensure!(
			self.pending_decrease_swap(who, investment_id)?.is_zero(),
			Error::<T>::CancellationInProgress
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

	/// Decrease an investment taking into account that a previous increment
	/// could be pending.
	/// This method is performed before applying the swap.
	pub fn pre_cancel_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<(T::ForeignBalance, SwapOf<T>), DispatchError> {
		Ok((
			self.pending_increase_swap(who, investment_id)?,
			Swap {
				currency_in: self.foreign_currency,
				currency_out: pool_currency_of::<T>(investment_id)?,
				amount_out: self.decrease_all_investment(who, investment_id)?.into(),
			},
		))
	}

	/// This method is performed after resolve the swap
	#[allow(clippy::type_complexity)]
	pub fn post_cancel_swap(
		&mut self,
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
				fulfilled: self.correlation.decrease_all(),
			}));
		}

		Ok(None)
	}

	/// This method is performed after a collect
	#[allow(clippy::type_complexity)]
	pub fn post_collect(
		&mut self,
		collected: CollectedAmount<T::TrancheBalance, T::PoolBalance>,
	) -> Result<
		ExecutedForeignCollectInvest<T::ForeignBalance, T::TrancheBalance, T::CurrencyId>,
		DispatchError,
	> {
		let collected_foreign_amount = self.correlation.decrease(collected.amount_payment)?;

		Ok(ExecutedForeignCollectInvest {
			currency: self.foreign_currency,
			amount_currency_invested: collected_foreign_amount,
			amount_tranche_tokens_payout: collected.amount_collected,
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

	fn decrease_all_investment(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::PoolBalance, DispatchError> {
		let pool_amount = T::Investment::investment(who, investment_id)?;
		T::Investment::update_investment(who, investment_id, pool_amount)?;

		Ok(pool_amount)
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
		let pool_amount = self
			.pending_decrease_swap(who, investment_id)?
			.ensure_add(T::Investment::investment(who, investment_id)?)?;

		let foreign_amount = self.pending_increase_swap(who, investment_id)?;

		Ok(pool_amount.is_zero() && foreign_amount.is_zero())
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
	) -> DispatchResult {
		T::Investment::update_redemption(
			who,
			investment_id,
			T::Investment::redemption(who, investment_id)?,
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
		swapped_amount: T::ForeignBalance,
		pending_amount: T::PoolBalance,
	) -> Result<
		Option<ExecutedForeignCollectRedeem<T::ForeignBalance, T::TrancheBalance, T::CurrencyId>>,
		DispatchError,
	> {
		self.swapped_amount.ensure_add_assign(swapped_amount)?;

		if pending_amount.is_zero() {
			let msg = ExecutedForeignCollectRedeem {
				currency: self.foreign_currency,
				amount_tranche_tokens_redeemed: self.collected_tranche_tokens(),
				amount_currency_payout: self.swapped_amount,
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
