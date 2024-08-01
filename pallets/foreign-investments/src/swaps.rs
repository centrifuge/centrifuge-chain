//! This is the only module to handle
//! - OrderBook trait
//! - OrderIdToSwapId storage
//! - Swap events

use cfg_traits::swaps::{OrderInfo, OrderRatio, Swap, SwapInfo, TokenSwaps};
use sp_runtime::{
	traits::{EnsureAdd, Zero},
	DispatchError, DispatchResult,
};

use crate::{Config, Event, OrderIdToSwapId, Pallet, SwapId, SwapOf};

pub fn create_swap<T: Config>(
	who: &T::AccountId,
	swap_id: SwapId<T>,
	swap: SwapOf<T>,
) -> Result<Option<T::OrderId>, DispatchError> {
	if swap.amount_out == Zero::zero() {
		return Ok(None);
	}

	Pallet::<T>::deposit_event(Event::SwapCreatedOrUpdated {
		who: who.clone(),
		swap_id,
		swap: swap.clone(),
	});

	let order_id = T::OrderBook::place_order(
		who.clone(),
		swap.currency_in,
		swap.currency_out,
		swap.amount_out,
		OrderRatio::Market,
	)?;

	OrderIdToSwapId::<T>::insert(order_id, (who.clone(), swap_id));

	Ok(Some(order_id))
}

pub fn increase_swap<T: Config>(
	who: &T::AccountId,
	swap_id: SwapId<T>,
	order_id: &T::OrderId,
	amount: T::SwapBalance,
) -> DispatchResult {
	if amount == Zero::zero() {
		return Ok(());
	}

	match T::OrderBook::get_order_details(*order_id) {
		Some(info) => {
			let new_amount = info.swap.amount_out.ensure_add(amount)?;

			Pallet::<T>::deposit_event(Event::SwapCreatedOrUpdated {
				who: who.clone(),
				swap_id,
				swap: Swap {
					amount_out: new_amount,
					..info.swap
				},
			});

			T::OrderBook::update_order(*order_id, new_amount, info.ratio)
		}
		None => Err(DispatchError::Other(
			"increase_swap() is always called over an existent order, qed",
		)),
	}
}

pub fn create_or_increase_swap<T: Config>(
	who: &T::AccountId,
	swap_id: SwapId<T>,
	order_id: &Option<T::OrderId>,
	swap: SwapOf<T>,
) -> Result<Option<T::OrderId>, DispatchError> {
	match order_id {
		None => create_swap::<T>(who, swap_id, swap),
		Some(order_id) => {
			increase_swap::<T>(who, swap_id, order_id, swap.amount_out)?;
			Ok(Some(*order_id))
		}
	}
}

pub fn cancel_swap<T: Config>(
	who: &T::AccountId,
	swap_id: SwapId<T>,
	order_id: &T::OrderId,
) -> Result<T::SwapBalance, DispatchError> {
	match T::OrderBook::get_order_details(*order_id) {
		Some(info) => {
			Pallet::<T>::deposit_event(Event::SwapCancelled {
				who: who.clone(),
				swap_id,
				swap: info.swap.clone(),
			});

			T::OrderBook::cancel_order(*order_id)?;

			OrderIdToSwapId::<T>::remove(order_id);

			Ok(info.swap.amount_out)
		}
		None => Err(DispatchError::Other(
			"cancel_swap() is always called over an existent order, qed",
		)),
	}
}

pub fn get_swap<T: Config>(
	order_id: &T::OrderId,
) -> Option<OrderInfo<T::SwapBalance, T::CurrencyId, T::SwapRatio>> {
	T::OrderBook::get_order_details(*order_id)
}

pub fn fulfilled_order<T: Config>(
	order_id: &T::OrderId,
	swap_info: &SwapInfo<T::SwapBalance, T::SwapBalance, T::CurrencyId, T::SwapRatio>,
) -> Option<(T::AccountId, SwapId<T>)> {
	let swap_id = OrderIdToSwapId::<T>::get(order_id);

	if let Some((who, (investment_id, action))) = swap_id.clone() {
		if swap_info.remaining.amount_out.is_zero() {
			OrderIdToSwapId::<T>::remove(order_id);
		}

		Pallet::<T>::deposit_event(Event::SwapFullfilled {
			who: who.clone(),
			swap_id: (investment_id, action),
			remaining: swap_info.remaining.clone(),
			swapped_in: swap_info.swapped_in,
			swapped_out: swap_info.swapped_out,
		});
	}

	swap_id
}
