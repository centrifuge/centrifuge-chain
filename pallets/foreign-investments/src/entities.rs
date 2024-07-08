//! Types with Config access.
//! - This module does not handle FI storages
//! - This module does not call hooks

use cfg_traits::{
	investments::Investment,
	swaps::{Swap, Swaps},
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
	deposit_apply_swap_event, deposit_cancel_swap_event,
	pallet::{Config, Error},
	pool_currency_of, Action,
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
}

impl<T: Config> InvestmentInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Self {
		Self {
			foreign_currency,
			foreign_amount: T::ForeignBalance::zero(),
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

	pub fn increase_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::ForeignBalance,
	) -> DispatchResult {
		let pool_currency = pool_currency_of::<T>(investment_id)?;

		if self.foreign_currency != pool_currency {
			let swap_id = (investment_id, Action::Investment);
			let swap = Swap {
				currency_in: pool_currency,
				currency_out: self.foreign_currency,
				amount_out: foreign_amount.into(),
			};

			let status = T::Swaps::apply_swap(who, swap_id, swap.clone())?;
			deposit_apply_swap_event::<T>(who, swap_id, &swap, &status);
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

		Ok(())
	}

	/// Decrease an investment taking into account that a previous increment
	/// could be pending.
	pub fn cancel_swap(
		&mut self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<(T::ForeignBalance, T::PoolBalance), DispatchError> {
		let cancel_pool_amount = T::Investment::investment(who, investment_id)?;
		T::Investment::update_investment(who, investment_id, Zero::zero())?;

		let pool_currency = pool_currency_of::<T>(investment_id)?;
		if self.foreign_currency != pool_currency {
			let swap_id = (investment_id, Action::Investment);
			let increase_foreign = self.pending_increase_swap(who, investment_id)?;

			if !increase_foreign.is_zero() {
				T::Swaps::cancel_swap(
					who,
					swap_id,
					increase_foreign.into(),
					self.foreign_currency,
				)?;

				deposit_cancel_swap_event::<T>(
					who,
					swap_id,
					self.foreign_currency,
					increase_foreign.into(),
				)?;

				// When cancelling, we no longer need to correlate.
				// The entire amount returned in the cancel msg will be the entire foreign
				// amount in the system, so we add here the not yet tracked pending amount.
				self.foreign_amount.ensure_add_assign(increase_foreign)?;
			}

			let swap = Swap {
				currency_in: self.foreign_currency,
				currency_out: pool_currency,
				amount_out: cancel_pool_amount.into(),
			};

			let status = T::Swaps::apply_swap(who, swap_id, swap.clone())?;
			deposit_apply_swap_event::<T>(who, swap_id, &swap, &status);

			Ok((increase_foreign, cancel_pool_amount))
		} else {
			Ok((cancel_pool_amount.into(), Zero::zero()))
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

			Ok(Some(msg))
		} else {
			Ok(None)
		}
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

	pub fn pending_increase_swap(
		&self,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::ForeignBalance, DispatchError> {
		let swap_id = (investment_id, Action::Investment);
		Ok(T::Swaps::pending_amount(who, swap_id, self.foreign_currency)?.into())
	}

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

	/// Total collected tranche tokens pending to be sent.
	pub collected_tranche_tokens: T::TrancheBalance,
}

impl<T: Config> RedemptionInfo<T> {
	pub fn new(foreign_currency: T::CurrencyId) -> Self {
		Self {
			foreign_currency,
			swapped_amount: T::ForeignBalance::default(),
			collected_tranche_tokens: T::TrancheBalance::default(),
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
			let swap_id = (investment_id, Action::Redemption);
			let swap = Swap {
				currency_in: self.foreign_currency,
				currency_out: pool_currency,
				amount_out: collected_pool_amount.into(),
			};
			let status = T::Swaps::apply_swap(who, swap_id, swap.clone())?;
			deposit_apply_swap_event::<T>(who, swap_id, &swap, &status);

			Ok((Zero::zero(), collected_pool_amount))
		} else {
			Ok((collected_pool_amount.into(), Zero::zero()))
		}
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
				amount_tranche_tokens_redeemed: self.collected_tranche_tokens,
				amount_currency_payout: self.swapped_amount,
			};

			self.swapped_amount = T::ForeignBalance::zero();
			self.collected_tranche_tokens = T::TrancheBalance::zero();

			return Ok(Some(msg));
		}

		Ok(None)
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
