//! Types with Config access. This module does not mutate FI storage

use cfg_traits::{
	investments::Investment,
	swaps::{OrderRatio, Swap, TokenSwaps},
};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCancelInvest, ExecutedForeignCollectInvest,
	ExecutedForeignCollectRedeem,
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
	pool_currency_of, SwapOf,
};

/// Hold the information of a foreign investment
#[derive(Clone, PartialEq, Eq, RuntimeDebugNoBound, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InvestmentInfo<T: Config> {
	/// Foreign currency of this investment
	pub foreign_currency: T::CurrencyId,

	/// Represents the foreign amount that has been converted into pool amount.
	/// This allow us to correlate both amounts to be able to make
	/// transformations in `post_collect()`.
	/// This value change when:
	/// - an increase swap is swapped (then, it increase).
	/// - some amount is collected (then, it decrease).
	/// - when cancel (then, it is reset).
	/// Note that during the cancelation, this variable "breaks" its meaning and
	/// also increases with any pending increasing swap until be fully reset.
	/// This does not break the `post_collect()` invariant because after
	/// cancelling, `post_collect()` can not be called.
	pub foreign_amount: T::ForeignBalance,

	/// Total decrease swapped amount pending to execute.
	/// It accumulates different partial swaps.
	pub decrease_swapped_foreign_amount: T::ForeignBalance,

	/// A possible order id associated to this investment
	/// Could be a pool to foreign order or foreign to pool order
	pub order_id: Option<T::OrderId>,
}

impl<T: Config> InvestmentInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Self {
		Self {
			foreign_currency,
			foreign_amount: T::ForeignBalance::zero(),
			decrease_swapped_foreign_amount: T::ForeignBalance::zero(),
			order_id: None,
		}
	}

	pub fn ensure_same_foreign(&self, foreign_currency: T::CurrencyId) -> DispatchResult {
		ensure!(
			self.foreign_currency == foreign_currency,
			Error::<T>::MismatchedForeignCurrency
		);

		Ok(())
	}

	pub fn ensure_no_pending_cancel(&self, investment_id: T::InvestmentId) -> DispatchResult {
		let pool_currency = pool_currency_of::<T>(investment_id)?;
		ensure!(
			self.order_id
				.and_then(|id| T::OrderBook::get_order_details(&id))
				.filter(|order_info| order_info.swap.currency_out == pool_currency)
				.is_none(),
			Error::<T>::CancellationInProgress
		);

		Ok(())
	}

	pub fn increase_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::ForeignBalance,
	) -> DispatchResult {
		let pool_currency = pool_currency_of::<T>(investment_id)?;
		if pool_currency != self.foreign_currency {
			match self.order_id {
				Some(order_id) => {
					T::OrderBook::increase_order(order_id, foreign_amount.into())?;
				}
				None => {
					self.order_id = Some(T::OrderBook::place_order(
						who.clone(),
						pool_currency,
						self.foreign_currency,
						foreign_amount.into(),
						OrderRatio::Market,
					)?);
				}
			}
			// TODO Pallet::<T>::deposit_apply_swap_events()
		}

		Ok(())
	}

	/// This method is performed after resolve the swap
	pub fn post_increase_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_pool_amount: T::PoolBalance,
		swapped_foreign_amount: T::ForeignBalance,
		pending_foreign_amount: T::ForeignBalance,
	) -> DispatchResult {
		self.foreign_amount
			.ensure_add_assign(swapped_foreign_amount)?;

		if !swapped_pool_amount.is_zero() {
			T::Investment::update_investment(
				who,
				investment_id,
				T::Investment::investment(who, investment_id)?.ensure_add(swapped_pool_amount)?,
			)?;
		}

		if pending_foreign_amount.is_zero() {
			self.order_id = None;
		}

		Ok(())
	}

	/// Decrease an investment taking into account that a previous increment
	/// could be pending.
	pub fn cancel_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::ForeignBalance, DispatchError> {
		let pool_currency = pool_currency_of::<T>(investment_id)?;

		let cancel_pool_amount = T::Investment::investment(who, investment_id)?;
		T::Investment::update_investment(who, investment_id, Zero::zero())?;

		if pool_currency != self.foreign_currency {
			let cancel_foreign_amount = match self.order_id {
				Some(order_id) => {
					let cancel_amount = T::OrderBook::cancel_order(order_id)?.into();

					// When cancelling, we no longer need to correlate.
					// The entire amount returned in the cancel msg will be the entire foreign
					// amount in the system, so we add here the not yet tracked pending amount.
					self.foreign_amount.ensure_add_assign(cancel_amount)?;
					cancel_amount
				}
				None => Zero::zero(),
			};

			self.order_id = Some(T::OrderBook::place_order(
				who.clone(),
				self.foreign_currency,
				pool_currency,
				cancel_pool_amount.into(),
				OrderRatio::Market,
			)?);

			//TODO: Pallet::<T>::deposit_apply_swap_events()?;
			Ok(cancel_foreign_amount)
		} else {
			Ok(cancel_pool_amount.into())
		}
	}

	/// This method is performed after resolve the swap
	#[allow(clippy::type_complexity)]
	pub fn post_cancel_swap(
		&mut self,
		swapped_foreign_amount: T::ForeignBalance,
		pending_pool_amount: T::PoolBalance,
	) -> Result<Option<ExecutedForeignCancelInvest<T::ForeignBalance, T::CurrencyId>>, DispatchError>
	{
		self.decrease_swapped_foreign_amount
			.ensure_add_assign(swapped_foreign_amount)?;

		if pending_pool_amount.is_zero() {
			let msg = ExecutedForeignCancelInvest {
				foreign_currency: self.foreign_currency,
				amount_cancelled: self.decrease_swapped_foreign_amount,
				fulfilled: self.foreign_amount,
			};

			self.decrease_swapped_foreign_amount = T::ForeignBalance::zero();
			self.foreign_amount = T::ForeignBalance::zero();

			return Ok(Some(msg));
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
		ExecutedForeignCollectInvest<T::ForeignBalance, T::TrancheBalance, T::CurrencyId>,
		DispatchError,
	> {
		// pool amount before collecting
		let pool_amount_before_collecting =
			T::Investment::investment(who, investment_id)? + collected.amount_payment;

		// Transform the collected pool amount into foreing amount.
		// This transformation is done by correlation, thanks that `foreing_amount`
		// contains the "same" amount as the investment pool amount but with different
		// denomination.
		let collected_foreign_amount = collected
			.amount_payment
			.ensure_mul(self.foreign_amount.into())?
			.ensure_div(pool_amount_before_collecting)
			.unwrap_or(self.foreign_amount.into())
			.into();

		self.foreign_amount
			.ensure_sub_assign(collected_foreign_amount)?;

		Ok(ExecutedForeignCollectInvest {
			currency: self.foreign_currency,
			amount_currency_invested: collected_foreign_amount,
			amount_tranche_tokens_payout: collected.amount_collected,
		})
	}

	pub fn is_completed(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<bool, DispatchError> {
		Ok(T::Investment::investment(who, investment_id)?.is_zero() && self.order_id.is_none())
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

	/// A possible order id associated to this investment
	/// Could be a pool to foreign
	pub order_id: Option<T::OrderId>,
}

impl<T: Config> RedemptionInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Self {
		Self {
			foreign_currency,
			swapped_amount: T::ForeignBalance::default(),
			collected: CollectedAmount::default(),
			order_id: None,
		}
	}

	pub fn ensure_same_foreign(&self, foreign_currency: T::CurrencyId) -> DispatchResult {
		ensure!(
			self.foreign_currency == foreign_currency,
			Error::<T>::MismatchedForeignCurrency
		);

		Ok(())
	}

	pub fn increase_redemption(
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

	pub fn cancel_redeemption(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> DispatchResult {
		T::Investment::update_redemption(who, investment_id, Zero::zero())
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
			self.order_id = None;

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
