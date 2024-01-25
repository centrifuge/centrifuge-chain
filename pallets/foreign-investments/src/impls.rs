//! Trait implementations. Higher level file.

use cfg_traits::{
	investments::{ForeignInvestment, Investment, InvestmentCollector, TrancheCurrency},
	PoolInspect, StatusNotificationHook, TokenSwaps,
};
use cfg_types::investments::CollectedAmount;
use frame_support::pallet_prelude::*;
use sp_runtime::traits::{EnsureAdd, EnsureSub, Zero};
use sp_std::marker::PhantomData;

use crate::{
	entities::{InvestmentInfo, RedemptionInfo},
	pallet::{
		Config, Error, ForeignIdToSwapId, ForeignInvestmentInfo, ForeignRedemptionInfo, Pallet,
		SwapIdToForeignId,
	},
	swaps::Swaps,
	Action, SwapOf,
};

/// Internal methods used by trait implementations
struct Util<T>(PhantomData<T>);

impl<T: Config> Util<T> {
	/// A wrap over `apply_swap()` that takes care of updating the swap id
	/// and notify
	fn apply_swap_and_notify(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		action: Action,
		new_swap: SwapOf<T>,
	) -> DispatchResult {
		let swap_id = ForeignIdToSwapId::<T>::get((who, investment_id, action));

		let status = Swaps::<T>::apply_swap(who, new_swap.clone(), swap_id)?;

		Swaps::<T>::update_swap_id(who, investment_id, action, status.swap_id)?;

		if !status.swapped.is_zero() {
			Self::notify_swap_done(
				who,
				investment_id,
				action,
				new_swap.currency_out,
				status.swapped,
				status.pending,
			)?;
		}

		Ok(())
	}

	/// Notifies that a partial swap has been done and applies the result to
	/// an `InvestmentInfo` or `RedemptionInfo`
	fn notify_swap_done(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		action: Action,
		currency_out: T::CurrencyId,
		swapped_amount: T::Balance,
		pending_amount: T::Balance,
	) -> DispatchResult {
		match action {
			Action::Investment => Util::<T>::notify_investment_swap_done(
				&who,
				investment_id,
				currency_out,
				swapped_amount,
				pending_amount,
			),
			Action::Redemption => Util::<T>::notify_redemption_swap_done(
				&who,
				investment_id,
				swapped_amount,
				pending_amount,
			),
		}
	}

	/// Notifies that a partial swap has been done and applies the result to
	/// an `InvestmentInfo`
	fn notify_investment_swap_done(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		currency_out: T::CurrencyId,
		swapped: T::Balance,
		pending: T::Balance,
	) -> DispatchResult {
		let msg = ForeignInvestmentInfo::<T>::mutate_exists(&who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;

			if currency_out == info.base.foreign_currency {
				info.post_increase_swap(who, investment_id, swapped)?;

				Ok(None)
			} else {
				let msg = info.post_decrease_swap(investment_id, swapped, pending)?;

				if info.is_completed()? {
					*entry = None;
				}

				Ok::<_, DispatchError>(msg)
			}
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
	fn notify_redemption_swap_done(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped_amount: T::Balance,
		pending_amount: T::Balance,
	) -> DispatchResult {
		let msg = ForeignRedemptionInfo::<T>::mutate_exists(&who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			let msg = info.post_swap(who, investment_id, swapped_amount, pending_amount)?;

			if info.is_completed() {
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

impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
	type Amount = T::Balance;
	type CurrencyId = T::CurrencyId;
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;

	fn increase_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::Balance,
		foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		let swap = ForeignInvestmentInfo::<T>::mutate(&who, investment_id, |info| {
			let info = info.get_or_insert(InvestmentInfo::new(foreign_currency)?);
			info.base.ensure_same_foreign(foreign_currency)?;
			info.pre_increase_swap(investment_id, foreign_amount)
		})?;

		Util::<T>::apply_swap_and_notify(who, investment_id, Action::Investment, swap)
	}

	fn decrease_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::Balance,
		foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		let swap = ForeignInvestmentInfo::<T>::mutate(&who, investment_id, |info| {
			let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.base.ensure_same_foreign(foreign_currency)?;
			info.pre_decrease_swap(who, investment_id, foreign_amount)
		})?;

		Util::<T>::apply_swap_and_notify(who, investment_id, Action::Investment, swap)
	}

	fn increase_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::Balance,
		payout_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate(who, investment_id, |info| -> DispatchResult {
			let info = info.get_or_insert(RedemptionInfo::new(payout_foreign_currency)?);
			info.base.ensure_same_foreign(payout_foreign_currency)?;
			info.increase(tranche_tokens_amount)
		})?;

		T::Investment::update_redemption(
			who,
			investment_id,
			T::Investment::redemption(who, investment_id)?.ensure_add(tranche_tokens_amount)?,
		)
	}

	fn decrease_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::Balance,
		payout_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
			let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.base.ensure_same_foreign(payout_foreign_currency)?;
			info.decrease(tranche_tokens_amount)
		})?;

		T::Investment::update_redemption(
			who,
			investment_id,
			T::Investment::redemption(who, investment_id)?.ensure_sub(tranche_tokens_amount)?,
		)
	}

	fn collect_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		payment_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignInvestmentInfo::<T>::mutate(&who, investment_id, |info| {
			let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.base.ensure_same_foreign(payment_foreign_currency)
		})?;

		T::Investment::collect_investment(who.clone(), investment_id)
	}

	fn collect_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		payout_foreign_currency: T::CurrencyId,
		_pool_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
			let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.base.ensure_same_foreign(payout_foreign_currency)
		})?;

		T::Investment::collect_redemption(who.clone(), investment_id)
	}

	fn investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		T::Investment::investment(who, investment_id)
	}

	fn redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		T::Investment::redemption(who, investment_id)
	}

	fn accepted_payment_currency(investment_id: T::InvestmentId, currency: T::CurrencyId) -> bool {
		if T::Investment::accepted_payment_currency(investment_id, currency) {
			true
		} else {
			T::PoolInspect::currency_for(investment_id.of_pool())
				.map(|pool_currency| T::TokenSwaps::valid_pair(pool_currency, currency))
				.unwrap_or(false)
		}
	}

	fn accepted_payout_currency(investment_id: T::InvestmentId, currency: T::CurrencyId) -> bool {
		if T::Investment::accepted_payout_currency(investment_id, currency) {
			true
		} else {
			T::PoolInspect::currency_for(investment_id.of_pool())
				.map(|pool_currency| T::TokenSwaps::valid_pair(currency, pool_currency))
				.unwrap_or(false)
		}
	}
}

pub struct FulfilledSwapOrderHook<T>(PhantomData<T>);
impl<T: Config> StatusNotificationHook for FulfilledSwapOrderHook<T> {
	type Error = DispatchError;
	type Id = T::SwapId;
	type Status = SwapOf<T>;

	fn notify_status_change(swap_id: T::SwapId, last_swap: SwapOf<T>) -> Result<(), DispatchError> {
		let (who, investment_id, action) =
			SwapIdToForeignId::<T>::get(swap_id).ok_or(Error::<T>::SwapOrderNotFound)?;

		let pending_amount = match T::TokenSwaps::get_order_details(swap_id) {
			Some(swap) => swap.amount_in,
			None => {
				Swaps::<T>::update_swap_id(&who, investment_id, action, None)?;
				T::Balance::default()
			}
		};

		Util::<T>::notify_swap_done(
			&who,
			investment_id,
			action,
			last_swap.currency_out,
			last_swap.amount_in,
			pending_amount,
		)
	}
}

pub struct CollectedInvestmentHook<T>(PhantomData<T>);
impl<T: Config> StatusNotificationHook for CollectedInvestmentHook<T> {
	type Error = DispatchError;
	type Id = (T::AccountId, T::InvestmentId);
	type Status = CollectedAmount<T::Balance>;

	fn notify_status_change(
		(who, investment_id): (T::AccountId, T::InvestmentId),
		collected: CollectedAmount<T::Balance>,
	) -> DispatchResult {
		let msg = ForeignInvestmentInfo::<T>::mutate_exists(&who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			let msg = info.post_collect(investment_id, collected);

			if info.is_completed()? {
				*entry = None;
			}

			msg
		})?;

		// We send the event out of the Info mutation closure
		T::CollectedForeignInvestmentHook::notify_status_change((who.clone(), investment_id), msg)
	}
}

pub struct CollectedRedemptionHook<T>(PhantomData<T>);
impl<T: Config> StatusNotificationHook for CollectedRedemptionHook<T> {
	type Error = DispatchError;
	type Id = (T::AccountId, T::InvestmentId);
	type Status = CollectedAmount<T::Balance>;

	fn notify_status_change(
		(who, investment_id): (T::AccountId, T::InvestmentId),
		collected: CollectedAmount<T::Balance>,
	) -> DispatchResult {
		let swap = ForeignRedemptionInfo::<T>::mutate(&who, investment_id, |info| {
			let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.pre_swap(investment_id, collected)
		})?;

		Util::<T>::apply_swap_and_notify(&who, investment_id, Action::Redemption, swap)
	}
}
