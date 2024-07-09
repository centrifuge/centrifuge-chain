//! Trait implementations. Higher level file.

use cfg_traits::{investments::ForeignInvestment, swaps::SwapInfo, StatusNotificationHook};
use cfg_types::investments::CollectedAmount;
use frame_support::pallet_prelude::*;
use sp_std::marker::PhantomData;

use crate::{
	entities::{InvestmentInfo, RedemptionInfo},
	pallet::{Config, Error, ForeignInvestmentInfo, ForeignRedemptionInfo, Pallet},
	pool_currency_of,
	swaps::fulfilled_order,
	Action,
};

impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
	type Amount = T::ForeignBalance;
	type CurrencyId = T::CurrencyId;
	type InvestmentId = T::InvestmentId;
	type TrancheAmount = T::TrancheBalance;

	fn increase_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_amount: T::ForeignBalance,
		foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignInvestmentInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.get_or_insert(InvestmentInfo::new(foreign_currency));
			info.ensure_same_foreign(foreign_currency)?;
			info.ensure_no_pending_cancel(investment_id)?;

			let (increased, pending) = info.increase(who, investment_id, foreign_amount)?;
			info.post_increase_swap(who, investment_id, increased, increased.into(), pending)?;

			remove_entry(info.is_completed(who, investment_id)?, entry)
		})
	}

	fn cancel_foreign_investment(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignInvestmentInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.ensure_same_foreign(foreign_currency)?;
			info.ensure_no_pending_cancel(investment_id)?;

			let (cancelled, pending) = info.cancel(who, investment_id)?;
			info.post_cancel_swap(who, investment_id, cancelled, pending)?;

			remove_entry(info.is_completed(who, investment_id)?, entry)
		})
	}

	fn increase_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		tranche_tokens_amount: T::TrancheBalance,
		payout_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate_exists(who, investment_id, |entry| -> DispatchResult {
			let info = entry.get_or_insert(RedemptionInfo::new(payout_foreign_currency));
			info.ensure_same_foreign(payout_foreign_currency)?;
			info.increase_redemption(who, investment_id, tranche_tokens_amount)?;
			remove_entry(info.is_completed(who, investment_id)?, entry)
		})
	}

	fn cancel_foreign_redemption(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		payout_foreign_currency: T::CurrencyId,
	) -> DispatchResult {
		ForeignRedemptionInfo::<T>::mutate_exists(who, investment_id, |entry| {
			let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
			info.ensure_same_foreign(payout_foreign_currency)?;
			info.cancel_redeemption(who, investment_id)?;
			remove_entry(info.is_completed(who, investment_id)?, entry)
		})
	}
}

impl<T: Config> StatusNotificationHook for Pallet<T> {
	type Error = DispatchError;
	type Id = T::OrderId;
	type Status = SwapInfo<T::SwapBalance, T::SwapBalance, T::CurrencyId, T::SwapRatio>;

	fn notify_status_change(order_id: T::OrderId, swap_info: Self::Status) -> DispatchResult {
		let (who, (investment_id, action)) = match fulfilled_order::<T>(&order_id, &swap_info) {
			Some(location) => location,
			None => return Ok(()), // notification not for FI
		};

		let pool_currency = pool_currency_of::<T>(investment_id)?;
		let swapped_amount_in = swap_info.swapped_in;
		let swapped_amount_out = swap_info.swapped_out;
		let pending_amount = swap_info.remaining.amount_out;

		match action {
			Action::Investment => {
				ForeignInvestmentInfo::<T>::mutate_exists(&who, investment_id, |entry| {
					let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
					if pool_currency == swap_info.remaining.currency_in {
						info.post_increase_swap(
							&who,
							investment_id,
							swapped_amount_in.into(),
							swapped_amount_out.into(),
							pending_amount.into(),
						)
					} else {
						info.post_cancel_swap(
							&who,
							investment_id,
							swapped_amount_in.into(),
							pending_amount.into(),
						)?;

						remove_entry(info.is_completed(&who, investment_id)?, entry)
					}
				})
			}
			Action::Redemption => {
				ForeignRedemptionInfo::<T>::mutate_exists(&who, investment_id, |entry| {
					let info = entry.as_mut().ok_or(Error::<T>::InfoNotFound)?;
					info.post_swap(
						&who,
						investment_id,
						swapped_amount_in.into(),
						pending_amount.into(),
					)?;

					remove_entry(info.is_completed(&who, investment_id)?, entry)
				})
			}
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
		ForeignInvestmentInfo::<T>::mutate_exists(&who, investment_id, |entry| {
			if let Some(info) = entry.as_mut() {
				info.ensure_no_pending_cancel(investment_id)?;
				info.post_collect(&who, investment_id, collected)?;

				remove_entry(info.is_completed(&who, investment_id)?, entry)?;
			}

			Ok(())
		})
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
		ForeignRedemptionInfo::<T>::mutate_exists(&who, investment_id, |entry| {
			if let Some(info) = entry.as_mut() {
				let (amount, pending) =
					info.post_collect_and_swap(&who, investment_id, collected)?;

				info.post_swap(&who, investment_id, amount, pending)?;

				remove_entry(info.is_completed(&who, investment_id)?, entry)?;
			}

			Ok(())
		})
	}
}

/// Avoiding boilerplate each time the entry needs to be removed
fn remove_entry<Entry>(condition: bool, entry: &mut Option<Entry>) -> DispatchResult {
	if condition {
		*entry = None;
	}

	Ok(())
}
