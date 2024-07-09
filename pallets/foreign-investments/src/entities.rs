//! Perform actions over ForeignInvestInfo and ForeignRedemptionInfo types
//! - This module does not handle FI storages
//! - This module does not call hooks
//! - This module does not directly OrderBooks
//! - This module does not directly OrderIdToSwapId storage

use cfg_traits::{investments::Investment, swaps::Swap, StatusNotificationHook};
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
	pool_currency_of,
	swaps::{cancel_swap, create_or_increase_swap, create_swap, get_swap},
	Action,
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
				.and_then(|id| get_swap::<T>(&id))
				.filter(|order_info| order_info.swap.currency_out == pool_currency)
				.is_none(),
			Error::<T>::CancellationInProgress
		);

		Ok(())
	}

	pub fn increase(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::ForeignBalance,
	) -> Result<(T::PoolBalance, T::ForeignBalance), DispatchError> {
		let pool_currency = pool_currency_of::<T>(investment_id)?;

		if self.foreign_currency != pool_currency {
			self.order_id = create_or_increase_swap::<T>(
				who,
				(investment_id, Action::Investment),
				&self.order_id,
				Swap {
					currency_in: pool_currency,
					currency_out: self.foreign_currency,
					amount_out: foreign_amount.into(),
				},
			)?;

			Ok((Zero::zero(), foreign_amount))
		} else {
			Ok((foreign_amount.into(), Zero::zero()))
		}
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
	pub fn cancel(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<(T::ForeignBalance, T::PoolBalance), DispatchError> {
		let swap_id = (investment_id, Action::Investment);
		let pool_currency = pool_currency_of::<T>(investment_id)?;

		let cancel_pool_amount = T::Investment::investment(who, investment_id)?;
		T::Investment::update_investment(who, investment_id, Zero::zero())?;

		if self.foreign_currency != pool_currency {
			let increase_foreign = match self.order_id {
				Some(order_id) => {
					let increase_foreign = cancel_swap::<T>(who, swap_id, &order_id)?.into();

					// When cancelling, we no longer need to correlate.
					// The entire amount returned in the cancel msg will be the entire foreign
					// amount in the system, so we add here the not yet tracked pending amount.
					self.foreign_amount.ensure_add_assign(increase_foreign)?;
					increase_foreign
				}
				None => T::ForeignBalance::zero(),
			};

			self.order_id = create_swap::<T>(
				who,
				swap_id,
				Swap {
					currency_in: self.foreign_currency,
					currency_out: pool_currency,
					amount_out: cancel_pool_amount.into(),
				},
			)?;

			Ok((increase_foreign, cancel_pool_amount))
		} else {
			Ok((cancel_pool_amount.into(), Zero::zero()))
		}
	}

	/// This method is performed after resolve the swap
	#[allow(clippy::type_complexity)]
	pub fn post_cancel_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_foreign_amount: T::ForeignBalance,
		pending_pool_amount: T::PoolBalance,
	) -> DispatchResult {
		self.decrease_swapped_foreign_amount
			.ensure_add_assign(swapped_foreign_amount)?;

		if pending_pool_amount.is_zero() {
			T::DecreasedForeignInvestOrderHook::notify_status_change(
				(who.clone(), investment_id),
				ExecutedForeignCancelInvest {
					foreign_currency: self.foreign_currency,
					amount_cancelled: self.decrease_swapped_foreign_amount,
					fulfilled: self.foreign_amount,
				},
			)?;

			self.decrease_swapped_foreign_amount = T::ForeignBalance::zero();
			self.foreign_amount = T::ForeignBalance::zero();
			self.order_id = None;
		}
		Ok(())
	}

	/// This method is performed after a collect
	#[allow(clippy::type_complexity)]
	pub fn post_collect(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::TrancheBalance, T::PoolBalance>,
	) -> DispatchResult {
		let invested = T::Investment::investment(who, investment_id)?;

		let collected_foreign_amount = if invested.is_zero() {
			// Last partial collect, we just return the tracked foreign amount
			// to ensure the sum of all partial collects matches the amount that was
			// incremented
			self.foreign_amount
		} else {
			let pool_amount_before_collecting = invested.ensure_add(collected.amount_payment)?;

			// Transform the collected pool amount into foreing amount.
			// This transformation is done by correlation, thanks that `foreing_amount`
			// contains the "same" amount as the investment pool amount but with different
			// denomination.
			collected
				.amount_payment
				.ensure_mul(self.foreign_amount.into())?
				.ensure_div(pool_amount_before_collecting)
				.unwrap_or(self.foreign_amount.into())
				.into()
		};

		self.foreign_amount
			.ensure_sub_assign(collected_foreign_amount)?;

		T::CollectedForeignInvestmentHook::notify_status_change(
			(who.clone(), investment_id),
			ExecutedForeignCollectInvest {
				currency: self.foreign_currency,
				amount_currency_invested: collected_foreign_amount,
				amount_tranche_tokens_payout: collected.amount_collected,
			},
		)
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

	/// Total collected tranche tokens pending to be sent.
	pub collected_tranche_tokens: T::TrancheBalance,

	/// A possible order id associated to this investment
	/// Could be a pool to foreign
	pub order_id: Option<T::OrderId>,
}

impl<T: Config> RedemptionInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Self {
		Self {
			foreign_currency,
			swapped_amount: T::ForeignBalance::default(),
			collected_tranche_tokens: T::TrancheBalance::default(),
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
	pub fn post_collect_and_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		collected: CollectedAmount<T::PoolBalance, T::TrancheBalance>,
	) -> Result<(T::ForeignBalance, T::PoolBalance), DispatchError> {
		self.collected_tranche_tokens
			.ensure_add_assign(collected.amount_payment)?;

		let pool_currency = pool_currency_of::<T>(investment_id)?;
		let collected_pool_amount = collected.amount_collected;

		if self.foreign_currency != pool_currency {
			self.order_id = create_or_increase_swap::<T>(
				who,
				(investment_id, Action::Redemption),
				&self.order_id,
				Swap {
					currency_in: self.foreign_currency,
					currency_out: pool_currency,
					amount_out: collected_pool_amount.into(),
				},
			)?;

			Ok((Zero::zero(), collected_pool_amount))
		} else {
			Ok((collected_pool_amount.into(), Zero::zero()))
		}
	}

	/// This method is performed after resolve the swap.
	#[allow(clippy::type_complexity)]
	pub fn post_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_amount: T::ForeignBalance,
		pending_amount: T::PoolBalance,
	) -> DispatchResult {
		self.swapped_amount.ensure_add_assign(swapped_amount)?;

		if pending_amount.is_zero() {
			T::CollectedForeignRedemptionHook::notify_status_change(
				(who.clone(), investment_id),
				ExecutedForeignCollectRedeem {
					currency: self.foreign_currency,
					amount_tranche_tokens_redeemed: self.collected_tranche_tokens,
					amount_currency_payout: self.swapped_amount,
				},
			)?;

			self.swapped_amount = T::ForeignBalance::zero();
			self.collected_tranche_tokens = T::TrancheBalance::zero();
			self.order_id = None;
		}

		Ok(())
	}

	pub fn is_completed(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<bool, DispatchError> {
		Ok(T::Investment::redemption(who, investment_id)?.is_zero()
			&& self.collected_tranche_tokens.is_zero())
	}
}
