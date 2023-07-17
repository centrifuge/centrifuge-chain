// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::{ForeignInvestment, Investment, SwapNotificationHook};
use cfg_types::investments::InvestmentInfo;
use sp_runtime::DispatchError;

use crate::{
	pallet,
	types::{InvestState, InvestTransition, Swap},
	Config, Error, ForeignInvestmentInfo, InvestmentState, Pallet,
};

// Handles the second stage of updating investments. Whichever (potentially
// async) code path of the first stage concludes it (partially) should call
// `Swap::Config::SwapNotificationHandler::notify_status_update(swap_order_id,
// swapped_amount)`.
impl<T: Config> SwapNotificationHook for Pallet<T> {
	type Error = DispatchError;
	type Id = T::TokenSwapOrderId;
	type Status = T::Balance;

	fn notify_status_change(
		id: T::TokenSwapOrderId,
		status: T::Balance,
	) -> Result<(), DispatchError> {
		// get InvestState
		let info = ForeignInvestmentInfo::<T>::get(id).ok_or(Error::<T>::InvestmentInfoNotFound)?;

		// update invest state
		let pre_state = InvestmentState::<T>::get(info.owner, info.id).unwrap_or_default();

		match info.payment_currency {
			pool_currency if T::Investment::accepted_payment_currency(info.id, pool_currency) => {
				pre_state
					.transition(InvestTransition::SwapIntoPool(Swap {
						currency: pool_currency,
						amount: status,
					}))
					.map(|_| ())
			}
			return_currency
				if T::Investment::accepted_payout_currency(info.id, return_currency) =>
			{
				pre_state
					.transition(InvestTransition::SwapIntoReturn(Swap {
						currency: return_currency,
						amount: status,
					}))
					.map(|_| ())
			}
			_ => Err(Error::<T>::InvalidInvestmentCurrency.into()),
		}
	}
}

impl<T: Config> ForeignInvestment<T::AccountId> for Pallet<T> {
	type Amount = T::Balance;
	type CurrencyId = T::CurrencyId;
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;
	type SwapNotification = Pallet<T>;

	// Consumers such as Connectors should call this function instead of
	// `Investment::update_invest_order` as this implementation accounts for
	// (potentially) splitting the update into two stages. The second stage is
	// resolved by `SwapNotificationHook::notify_status_change`.
	fn update_foreign_invest_order(
		who: &T::AccountId,
		payment_currency: T::CurrencyId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
	) -> Result<(), DispatchError> {
		let pre_amount = T::Investment::investment(who, investment_id.clone())?;
		let pre_state = InvestmentState::<T>::get(who, investment_id.clone()).unwrap_or_default();
		let swap = Swap {
			currency: payment_currency,
			amount,
		};

		if amount > pre_amount {
			let post_state = pre_state.transition(InvestTransition::IncreaseInvestOrder(swap))?;
			Ok(())
		} else if amount < pre_amount {
			let post_state = pre_state.transition(InvestTransition::DecreaseInvestOrder(swap))?;
			Ok(())
		} else {
			Ok(())
		}
	}
}

impl<Balance, Currency> InvestState<Balance, Currency>
where
	Balance: Clone,
	Currency: Clone,
{
	// TODO: Kill storage here if post_state = NoState? Might need a wrapper around
	// this to mutate the actual storage
	/// Apply state machine, see https://centrifuge.hackmd.io/IPtRlOrOSrOF9MHjEY48BA?view#State-diagram
	pub fn transition(
		&self,
		transition: InvestTransition<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match transition {
			InvestTransition::IncreaseInvestOrder(swap) => {
				Self::handle_increase(&self, swap.currency, swap.amount)
			}
			InvestTransition::DecreaseInvestOrder(swap) => {
				Self::handle_decrease(&self, swap.currency, swap.amount)
			}
			InvestTransition::SwapIntoPool(swap) => {
				Self::handle_fulfilled_swap_into_pool(&self, swap.currency, swap.amount)
			}
			// TODO: Should expose hook implemented by consumers such as Connectors
			// for handling `ExecutedDecreaseInvestOrder`.
			InvestTransition::SwapIntoReturn(swap) => {
				Self::handle_fulfilled_swap_into_return(&self, swap.currency, swap.amount)
			}
		}
	}
}

// Actual impl of transition
impl<Balance, Currency> InvestState<Balance, Currency>
where
	Balance: Clone,
	Currency: Clone,
{
	/// Handle `increase` transitions depicted by `msg::increase` edges in the
	/// state diagram.
	fn handle_increase(
		&self,
		swap_currency: Currency,
		amount: Balance,
	) -> Result<Self, DispatchError> {
		todo!("Do state transition here")
	}

	/// Handle `decrease` transitions depicted by `msg::decrease` edges in the
	/// state diagram.
	fn handle_decrease(
		&self,
		swap_currency: Currency,
		amount: Balance,
	) -> Result<Self, DispatchError> {
		todo!("Do state transition here")
	}

	/// Handle partial/full token swap order transitions into pool currency
	/// depicted by `order_partial` edges in the state diagram where the swap
	/// currency matches the pool one.
	///
	/// NOTE: These should always increase the active ongoing investment.
	fn handle_fulfilled_swap_into_pool(
		&self,
		swap_currency: Currency,
		amount: Balance,
	) -> Result<Self, DispatchError> {
		todo!("Do state transition here")
	}

	/// Handle partial/full token swap order transitions into return currency
	/// depicted by `order_partial` edges in the state diagram with the swap
	/// currency matches the return one.
	///
	/// NOTE: Assumes the corresponding investment has been decreased
	/// beforehand.
	fn handle_fulfilled_swap_into_return(
		&self,
		swap_currency: Currency,
		amount: Balance,
	) -> Result<Self, DispatchError> {
		todo!("Do state transition here")
	}
}

// TODO: How to merge token swaps and investment trait? Create new trait
// ForeignInvestment? > Check diagrams

// impl<T: Config> Investment<T::AccountId> for Pallet<T> {
// 	type Amount = T::Balance;
// 	type CurrencyId = T::CurrencyId;
// 	type Error = DispatchError;
// 	type InvestmentId = T::InvestmentId;

// 	fn update_investment(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 		amount: Self::Amount,
// 	) -> Result<(), Self::Error> {
// 		let pre_amount = Self::investment(who, investment_id.clone())?;
// 		let pre_state = InvestmentState::<T>::get(who,
// investment_id.clone()).unwrap_or_default();

// 		if amount > pre_amount {
// 			// TODO: Can payment currency be derived?
// 			let swap_currency =
// 				<Self as Accountant>::info(investment_id).map(|info|
// info.payment_currency()); 			let post_state: Option<InvestState<<T as
// Config>::Balance, <T as Config>::CurrencyId>> = 				pre_state.
// transition(InvestTransition::IncreaseInvestOrder(amount))?; 			Ok(())
// 		} else if amount < pre_amount {
// 			let post_state: Option<InvestState<<T as Config>::Balance, <T as
// Config>::CurrencyId>> = 				pre_state.
// transition(InvestTransition::DecreaseInvestOrder(amount))?; 			Ok(())
// 		} else {
// 			Ok(())
// 		}
// 	}

// 	fn accepted_payment_currency(
// 		investment_id: Self::InvestmentId,
// 		currency: Self::CurrencyId,
// 	) -> bool {
// 		T::Investment::accepted_payment_currency(investment_id, currency)
// 	}

// 	fn investment(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 	) -> Result<Self::Amount, Self::Error> {
// 		todo!()
// 	}

// 	fn update_redemption(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 		amount: Self::Amount,
// 	) -> Result<(), Self::Error> {
// 		todo!()
// 	}

// 	fn accepted_payout_currency(
// 		investment_id: Self::InvestmentId,
// 		currency: Self::CurrencyId,
// 	) -> bool {
// 		T::Investment::accepted_payout_currency(investment_id, currency)
// 	}

// 	fn redemption(
// 		who: &T::AccountId,
// 		investment_id: Self::InvestmentId,
// 	) -> Result<Self::Amount, Self::Error> {
// 		todo!()
// 	}
// }
