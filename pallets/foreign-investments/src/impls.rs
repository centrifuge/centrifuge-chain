//! Trait implementations. Higher level file.

use cfg_traits::{
	investments::{ForeignInvestment, Investment, InvestmentCollector, TrancheCurrency},
	PoolInspect, StatusNotificationHook, TokenSwaps,
};
use cfg_types::investments::CollectedAmount;
use frame_support::pallet_prelude::*;
use sp_runtime::traits::{EnsureAdd, Zero};
use sp_std::marker::PhantomData;

use crate::{
	entities::{InvestmentInfo, RedemptionInfo},
	pallet::{Config, Error, ForeignInvestmentInfo, ForeignRedemptionInfo, Pallet},
	pool_currency_of,
	swaps::Swaps,
	Action, SwapOf,
};

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
		let swap = ForeignInvestmentInfo::<T>::mutate(who, investment_id, |info| {
			let info = info.get_or_insert(InvestmentInfo::new(foreign_currency)?);
			info.base.ensure_same_foreign(foreign_currency)?;
			info.pre_increase_swap(investment_id, foreign_amount)
		})?;

		let status = Swaps::<T>::apply(who, investment_id, Action::Investment, swap)?;

		if !status.swapped.is_zero() {
			SwapDone::<T>::for_increase_investment(who, investment_id, status.swapped)?;
		}

		Ok(())
	}

	fn decrease_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::Balance,
		foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		let swap = ForeignInvestmentInfo::<T>::mutate(who, investment_id, |info| {
			let info = info.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.base.ensure_same_foreign(foreign_currency)?;
			info.pre_decrease_swap(who, investment_id, foreign_amount)
		})?;

		let status = Swaps::<T>::apply(who, investment_id, Action::Investment, swap)?;

		if !status.swapped.is_zero() {
			SwapDone::<T>::for_decrease_investment(
				who,
				investment_id,
				status.swapped,
				status.pending,
			)?;
		}

		Ok(())
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
			info.increase(who, investment_id, tranche_tokens_amount)
		})
	}

	fn decrease_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::Balance,
		payout_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.base.ensure_same_foreign(payout_foreign_currency)?;
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
			info.base.ensure_same_foreign(payment_foreign_currency)
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
			info.base.ensure_same_foreign(payout_foreign_currency)
		})?;

		T::Investment::collect_redemption(who.clone(), investment_id)
	}

	fn investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
	) -> Result<T::Balance, DispatchError> {
		Ok(T::Investment::investment(who, investment_id)?.ensure_add(
			Swaps::<T>::pending_amount_for(
				who,
				investment_id,
				Action::Investment,
				pool_currency_of::<T>(investment_id)?,
			),
		)?)
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

	fn notify_status_change(swap_id: T::SwapId, last_swap: SwapOf<T>) -> DispatchResult {
		match Swaps::<T>::foreign_id_from(swap_id) {
			Ok((who, investment_id, action)) => {
				let pool_currency = pool_currency_of::<T>(investment_id)?;
				let swapped_amount = last_swap.amount_in;
				let pending_amount = match T::TokenSwaps::get_order_details(swap_id) {
					Some(swap) => swap.amount_in,
					None => {
						Swaps::<T>::update_id(&who, investment_id, action, None)?;
						T::Balance::default()
					}
				};

				match action {
					Action::Investment => match pool_currency == last_swap.currency_in {
						true => SwapDone::<T>::for_increase_investment(
							&who,
							investment_id,
							swapped_amount,
						),
						false => SwapDone::<T>::for_decrease_investment(
							&who,
							investment_id,
							swapped_amount,
							pending_amount,
						),
					},
					Action::Redemption => SwapDone::<T>::for_redemption(
						&who,
						investment_id,
						swapped_amount,
						pending_amount,
					),
				}
			}
			Err(_) => Ok(()), // The event is not for foreign investments
		}
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
	type Status = CollectedAmount<T::Balance>;

	fn notify_status_change(
		(who, investment_id): (T::AccountId, T::InvestmentId),
		collected: CollectedAmount<T::Balance>,
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
			let status = Swaps::<T>::apply(&who, investment_id, Action::Redemption, swap)?;

			if !status.swapped.is_zero() {
				SwapDone::<T>::for_redemption(&who, investment_id, status.swapped, status.pending)?;
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
		swapped: T::Balance,
	) -> DispatchResult {
		ForeignInvestmentInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.post_increase_swap(who, investment_id, swapped)
		})
	}

	/// Notifies that a partial decrease swap has been done and applies the
	/// result to an `InvestmentInfo`
	fn for_decrease_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swapped: T::Balance,
		pending: T::Balance,
	) -> DispatchResult {
		let msg = ForeignInvestmentInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			let msg = info.post_decrease_swap(who, investment_id, swapped, pending)?;

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
		swapped_amount: T::Balance,
		pending_amount: T::Balance,
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
