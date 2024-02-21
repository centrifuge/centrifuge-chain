//! Trait implementations. Higher level file.

use cfg_traits::{
	investments::{ForeignInvestment, Investment, InvestmentCollector},
	swaps::{SwapState, Swaps},
	StatusNotificationHook,
};
use cfg_types::investments::CollectedAmount;
use frame_support::pallet_prelude::*;
use sp_runtime::traits::{EnsureAdd, EnsureSub, Zero};
use sp_std::marker::PhantomData;

use crate::{
	entities::{InvestmentInfo, RedemptionInfo},
	pallet::{Config, Error, ForeignInvestmentInfo, ForeignRedemptionInfo, Pallet},
	pool_currency_of, Action, SwapId,
};

impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
	type Amount = T::ForeignBalance;
	type CurrencyId = T::CurrencyId;
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;
	type TrancheAmount = T::TrancheBalance;

	fn increase_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::ForeignBalance,
		foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		let msg = ForeignInvestmentInfo::<T>::mutate(who, investment_id, |entry| {
			let info = entry.get_or_insert(InvestmentInfo::new(foreign_currency));
			info.ensure_same_foreign(foreign_currency)?;

			let swap = info.pre_increase_swap(who, investment_id, foreign_amount)?;
			let swap_id = (investment_id, Action::Investment);
			let status = T::Swaps::apply_swap(who, swap_id, swap.clone())?;

			let mut msg = None;
			if !status.swapped.is_zero() {
				let swapped_foreign_amount = foreign_amount.ensure_sub(status.pending.into())?;
				if !swap.has_same_currencies() {
					msg = info.post_increase_swap_by_cancel(
						who,
						investment_id,
						status.swapped.into(),
						swapped_foreign_amount,
					)?;
				} else {
					info.post_increase_swap(
						who,
						investment_id,
						status.swapped.into(),
						swapped_foreign_amount,
					)?;
				}
			}

			Ok::<_, DispatchError>(msg)
		})?;

		if let Some(msg) = msg {
			T::DecreasedForeignInvestOrderHook::notify_status_change(
				(who.clone(), investment_id),
				msg,
			)?;
		}

		Ok(())
	}

	fn decrease_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::ForeignBalance,
		foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		let msg = ForeignInvestmentInfo::<T>::mutate(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.ensure_same_foreign(foreign_currency)?;

			let swap = info.pre_decrease_swap(who, investment_id, foreign_amount)?;
			let swap_id = (investment_id, Action::Investment);
			let status = T::Swaps::apply_swap(who, swap_id, swap.clone())?;

			let mut msg = None;
			if !status.swapped.is_zero() {
				if !swap.has_same_currencies() {
					msg = info.post_decrease_swap_by_cancel(
						who,
						investment_id,
						status.swapped.into(),
						status.pending.into(),
					)?;
				} else {
					msg = info.post_decrease_swap(
						who,
						investment_id,
						status.swapped.into(),
						status.swapped.into(),
						status.pending.into(),
					)?;
				}
			}

			if info.is_completed(who, investment_id)? {
				*entry = None;
			}

			Ok::<_, DispatchError>(msg)
		})?;

		if let Some(msg) = msg {
			T::DecreasedForeignInvestOrderHook::notify_status_change(
				(who.clone(), investment_id),
				msg,
			)?;
		}

		Ok(())
	}

	fn increase_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::TrancheBalance,
		payout_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate(who, investment_id, |info| -> DispatchResult {
			let info = info.get_or_insert(RedemptionInfo::new(payout_foreign_currency));
			info.ensure_same_foreign(payout_foreign_currency)?;
			info.increase(who, investment_id, tranche_tokens_amount)
		})
	}

	fn decrease_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::TrancheBalance,
		payout_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.ensure_same_foreign(payout_foreign_currency)?;
			info.decrease(who, investment_id, tranche_tokens_amount)?;

			if info.is_completed(who, investment_id)? {
				*entry = None;
			}

			Ok(())
		})
	}

	fn collect_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		payment_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignInvestmentInfo::<T>::mutate(who, investment_id, |info| {
			let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.ensure_same_foreign(payment_foreign_currency)
		})?;

		T::Investment::collect_investment(who.clone(), investment_id)
	}

	fn collect_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		payout_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate(who, investment_id, |info| {
			let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.ensure_same_foreign(payout_foreign_currency)
		})?;

		T::Investment::collect_redemption(who.clone(), investment_id)
	}

	fn investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::ForeignBalance, DispatchError> {
		Ok(match ForeignInvestmentInfo::<T>::get(who, investment_id) {
			Some(info) => {
				let pool_investment = T::Investment::investment(who, investment_id)?;
				let foreing_investment = info
					.correlation
					.pool_to_foreign(pool_investment)
					.unwrap_or_default();

				foreing_investment.ensure_add(info.pending_increase_swap(who, investment_id)?)?
			}
			None => T::ForeignBalance::default(),
		})
	}

	fn redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::TrancheBalance, DispatchError> {
		T::Investment::redemption(who, investment_id)
	}

	fn accepted_payment_currency(investment_id: T::InvestmentId, currency: T::CurrencyId) -> bool {
		T::Investment::accepted_payment_currency(investment_id, currency)
	}

	fn accepted_payout_currency(investment_id: T::InvestmentId, currency: T::CurrencyId) -> bool {
		T::Investment::accepted_payout_currency(investment_id, currency)
	}
}

pub struct FulfilledSwapHook<T>(PhantomData<T>);
impl<T: Config> StatusNotificationHook for FulfilledSwapHook<T> {
	type Error = DispatchError;
	type Id = (T::AccountId, SwapId<T>);
	type Status = SwapState<T::SwapBalance, T::SwapBalance, T::CurrencyId>;

	fn notify_status_change(
		(who, (investment_id, action)): Self::Id,
		swap_state: Self::Status,
	) -> DispatchResult {
		let pool_currency = pool_currency_of::<T>(investment_id)?;
		let swapped_amount_in = swap_state.swapped_in;
		let swapped_amount_out = swap_state.swapped_out;
		let pending_amount = swap_state.remaining.amount_out;

		match action {
			Action::Investment => match pool_currency == swap_state.remaining.currency_in {
				true => SwapDone::<T>::for_increase_investment(
					&who,
					investment_id,
					swapped_amount_in.into(),
					swapped_amount_out.into(),
				),
				false => SwapDone::<T>::for_decrease_investment(
					&who,
					investment_id,
					swapped_amount_in.into(),
					swapped_amount_out.into(),
					pending_amount.into(),
				),
			},
			Action::Redemption => SwapDone::<T>::for_redemption(
				&who,
				investment_id,
				swapped_amount_in.into(),
				pending_amount.into(),
			),
		}
	}
}

pub struct CollectedInvestmentHook<T>(PhantomData<T>);
impl<T: Config> StatusNotificationHook for CollectedInvestmentHook<T> {
	type Error = DispatchError;
	type Id = (T::AccountId, T::InvestmentId);
	type Status = CollectedAmount<T::TrancheBalance, T::PoolBalance>;

	fn notify_status_change(
		(who, investment_id): (T::AccountId, T::InvestmentId),
		collected: CollectedAmount<T::TrancheBalance, T::PoolBalance>,
	) -> DispatchResult {
		let msg = ForeignInvestmentInfo::<T>::mutate_exists(&who, investment_id, |entry| {
			match entry.as_mut() {
				Some(info) => {
					let msg = info.post_collect(&who, investment_id, collected)?;

					if info.is_completed(&who, investment_id)? {
						*entry = None;
					}

					Ok::<_, DispatchError>(Some(msg))
				}
				None => Ok(None), // Then notification is not for foreign investments
			}
		})?;

		// We send the event out of the Info mutation closure
		if let Some(msg) = msg {
			T::CollectedForeignInvestmentHook::notify_status_change(
				(who.clone(), investment_id),
				msg,
			)?;
		}

		Ok(())
	}
}

pub struct CollectedRedemptionHook<T>(PhantomData<T>);
impl<T: Config> StatusNotificationHook for CollectedRedemptionHook<T> {
	type Error = DispatchError;
	type Id = (T::AccountId, T::InvestmentId);
	type Status = CollectedAmount<T::PoolBalance, T::TrancheBalance>;

	fn notify_status_change(
		(who, investment_id): (T::AccountId, T::InvestmentId),
		collected: CollectedAmount<T::PoolBalance, T::TrancheBalance>,
	) -> DispatchResult {
		let swap = ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |entry| {
			match entry.as_mut() {
				Some(info) => info
					.post_collect_and_pre_swap(investment_id, collected)
					.map(Some),
				None => Ok(None), // Then notification is not for foreign investments
			}
		})?;

		if let Some(swap) = swap {
			let swap_id = (investment_id, Action::Redemption);
			let status = T::Swaps::apply_swap(&who, swap_id, swap)?;

			if !status.swapped.is_zero() {
				SwapDone::<T>::for_redemption(
					&who,
					investment_id,
					status.swapped.into(),
					status.pending.into(),
				)?;
			}
		}

		Ok(())
	}
}

/// Internal methods used to execute swaps already done
struct SwapDone<T>(PhantomData<T>);
impl<T: Config> SwapDone<T> {
	/// Notifies that a partial increse swap has been done and applies the
	/// result to an `InvestmentInfo`
	fn for_increase_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_pool_amount: T::PoolBalance,
		swapped_foreign_amount: T::ForeignBalance,
	) -> DispatchResult {
		ForeignInvestmentInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.post_increase_swap(
				who,
				investment_id,
				swapped_pool_amount,
				swapped_foreign_amount,
			)
		})
	}

	/// Notifies that a partial decrease swap has been done and applies the
	/// result to an `InvestmentInfo`
	fn for_decrease_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_foreign_amount: T::ForeignBalance,
		swapped_pool_amount: T::PoolBalance,
		pending_pool_amount: T::PoolBalance,
	) -> DispatchResult {
		let msg = ForeignInvestmentInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			let msg = info.post_decrease_swap(
				who,
				investment_id,
				swapped_foreign_amount,
				swapped_pool_amount,
				pending_pool_amount,
			)?;

			if info.is_completed(who, investment_id)? {
				*entry = None;
			}

			Ok::<_, DispatchError>(msg)
		})?;

		// We send the event out of the Info mutation closure
		if let Some(msg) = msg {
			T::DecreasedForeignInvestOrderHook::notify_status_change(
				(who.clone(), investment_id),
				msg,
			)?;
		}

		Ok(())
	}

	/// Notifies that a partial swap has been done and applies the result to
	/// an `RedemptionInfo`
	fn for_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_amount: T::ForeignBalance,
		pending_amount: T::PoolBalance,
	) -> DispatchResult {
		let msg = ForeignRedemptionInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			let msg = info.post_swap(who, investment_id, swapped_amount, pending_amount)?;

			if info.is_completed(who, investment_id)? {
				*entry = None;
			}

			Ok::<_, DispatchError>(msg)
		})?;

		// We send the event out of the Info mutation closure
		if let Some(msg) = msg {
			T::CollectedForeignRedemptionHook::notify_status_change(
				(who.clone(), investment_id),
				msg,
			)?;
		}

		Ok(())
	}
}
