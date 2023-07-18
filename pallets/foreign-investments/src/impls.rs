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

use cfg_traits::{ForeignInvestment, Investment, StatusNotificationHook, TokenSwaps};
use cfg_types::investments::{ExecutedDecrease, InvestmentInfo};
use frame_support::{traits::Get, transactional};
use sp_runtime::{
	traits::{EnsureAdd, Zero},
	DispatchError, DispatchResult,
};

use crate::{
	pallet,
	types::{InvestState, InvestTransition, Swap},
	Config, Error, ForeignInvestmentInfo, ForeignInvestmentInfoOf, InvestmentState, Pallet, SwapOf,
	TokenSwapOrderIds,
};

// Handles the second stage of updating investments. Whichever (potentially
// async) code path of the first stage concludes it (partially) should call
// `Swap::Config::SwapNotificationHandler::notify_status_update(swap_order_id,
// swapped_amount)`.
impl<T: Config> StatusNotificationHook for Pallet<T> {
	type Error = DispatchError;
	type Id = T::TokenSwapOrderId;
	type Status = SwapOf<T>;

	fn notify_status_change(
		id: T::TokenSwapOrderId,
		status: SwapOf<T>,
	) -> Result<(), DispatchError> {
		let info = ForeignInvestmentInfo::<T>::get(id).ok_or(Error::<T>::InvestmentInfoNotFound)?;
		let pre_state = InvestmentState::<T>::get(info.owner, info.id).unwrap_or_default();

		match status.currency_in {
			pool_currency if T::Investment::accepted_payment_currency(info.id, pool_currency) => {
				pre_state
					.transition(InvestTransition::SwapIntoPool(status))
					.map(|_| ())
			}
			return_currency
				if T::Investment::accepted_payout_currency(info.id, return_currency) =>
			{
				pre_state
					.transition(InvestTransition::SwapIntoReturn(status))
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
	// resolved by `StatusNotificationHook::notify_status_change`.
	fn update_foreign_invest_order(
		who: &T::AccountId,
		return_currency: T::CurrencyId,
		pool_currency: T::CurrencyId,
		investment_id: T::InvestmentId,
		amount: T::Balance,
	) -> Result<(), DispatchError> {
		let pre_amount = T::Investment::investment(who, investment_id.clone())?;
		let pre_state = InvestmentState::<T>::get(who, investment_id.clone()).unwrap_or_default();

		if amount > pre_amount {
			let post_state = pre_state.transition(InvestTransition::IncreaseInvestOrder(Swap {
				currency_in: pool_currency,
				currency_out: return_currency,
				amount,
			}))?;
			Ok(())
		} else if amount < pre_amount {
			let post_state = pre_state.transition(InvestTransition::DecreaseInvestOrder(Swap {
				currency_in: return_currency,
				currency_out: pool_currency,
				amount,
			}))?;
			Ok(())
		} else {
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Updates chain storage after applying state transition of `InvestState`
	/// and execute `ExecutedDecreaseHook`.
	#[transactional]
	fn apply_state_transition(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		state: InvestState<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		match state.clone() {
			InvestState::NoState => {
				Self::kill_swap_order(who, investment_id)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;

				// Exit early to prevent setting InvestmentState to `NoState`
				InvestmentState::<T>::remove(who, investment_id);
				return Ok(());
			},
			InvestState::InvestmentOngoing { invest_amount } => {
				Self::kill_swap_order(who, investment_id)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;
			},
			InvestState::ActiveSwapIntoPoolCurrency(swap) |
			InvestState::ActiveSwapIntoReturnCurrency(swap) |
			// We don't care about `done_amount` until swap into return is fulfilled
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, .. } => {
				Self::place_swap_order(who, investment_id, swap)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing { swap, invest_amount } |
			InvestState::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing { swap, invest_amount } | 
			// We don't care about `done_amount` until swap into return is fulfilled
			InvestState::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap, invest_amount, .. } => {
				Self::place_swap_order(who, investment_id, swap)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap, done_amount } => {
				Self::place_swap_order(who, investment_id, swap)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;

				//
				Self::send_executed_decrease_hook(who, investment_id, done_amount)?;
			},
			InvestState::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing { swap, done_amount, invest_amount } => {
				Self::place_swap_order(who, investment_id, swap)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;

				Self::send_executed_decrease_hook(who, investment_id, done_amount)?;

			},
			InvestState::SwapIntoReturnDone(_swap) => {
				Self::kill_swap_order(who, investment_id)?;
				T::Investment::update_investment(who, investment_id, Zero::zero())?;
			},
			InvestState::SwapIntoReturnDoneAndInvestmentOngoing { swap, invest_amount } => {
				Self::kill_swap_order(who, investment_id)?;
				T::Investment::update_investment(who, investment_id, invest_amount)?;
			},
		};

		InvestmentState::<T>::insert(who, investment_id, state);

		// TODO: Emit event?

		Ok(())
	}

	/// Kills all storage associated with token swaps and cancels the
	/// potentially active swap order.
	fn kill_swap_order(who: &T::AccountId, investment_id: T::InvestmentId) -> DispatchResult {
		if let Some(swap_order_id) = TokenSwapOrderIds::<T>::take(who, investment_id) {
			T::TokenSwaps::cancel_order(swap_order_id);
			ForeignInvestmentInfo::<T>::remove(swap_order_id);
		}
		Ok(())
	}

	/// Initializes or updates an existing swap order.
	///
	/// Sets up `TokenSwapOrderIds` and `ForeignInvestmentInfo` storages, if the
	/// order does not exist yet.
	fn place_swap_order(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		swap: SwapOf<T>,
	) -> DispatchResult {
		if let Some(swap_order_id) = TokenSwapOrderIds::<T>::get(who, investment_id) {
			T::TokenSwaps::update_order(
				who.clone(),
				swap_order_id,
				swap.amount,
				T::DefaultTokenSwapSellPriceLimit::get(),
				T::DefaultTokenMinFulfillmentAmount::get(),
			)
		} else {
			// TODO: How to handle potential failure?
			let order_id = T::TokenSwaps::place_order(
				who.clone(),
				swap.currency_out,
				swap.currency_in,
				swap.amount,
				T::DefaultTokenSwapSellPriceLimit::get(),
				T::DefaultTokenMinFulfillmentAmount::get(),
			)?;
			TokenSwapOrderIds::<T>::insert(who, investment_id, order_id);
			ForeignInvestmentInfo::<T>::insert(
				order_id,
				ForeignInvestmentInfoOf::<T> {
					owner: who.clone(),
					id: investment_id,
				},
			);
			Ok(())
		}
	}

	/// Sends `ExecutedDecreaseHook` notification such that any potential
	/// consumer could act upon that, e.g. Connectors for
	/// `ExecutedDecrease{Invest, Redeem}Order`.
	fn send_executed_decrease_hook(
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		amount_payout: T::Balance,
	) -> DispatchResult {
		// TODO(@mustermeiszer): Does this return the entire desired amount or do we
		// need to tap into collecting?
		let amount_remaining = T::Investment::investment(who, investment_id)?;

		// TODO(@mustermeiszer): Do we add the active swap amount?
		T::ExecutedDecreaseHook::notify_status_change(
			ForeignInvestmentInfoOf::<T> {
				owner: who.clone(),
				id: investment_id,
			},
			ExecutedDecrease {
				amount_payout,
				amount_remaining,
			},
		)
	}
}

impl<Balance, Currency> InvestState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd,
	Currency: Clone + PartialEq,
{
	/// Solely apply state machine to transition one `InvestState` into another
	/// based on the transition, see https://centrifuge.hackmd.io/IPtRlOrOSrOF9MHjEY48BA?view#State-diagram.
	///
	/// NOTE: Does not mutate storage which is done by `apply_state_transition`.
	pub fn transition(
		&self,
		transition: InvestTransition<Balance, Currency>,
	) -> Result<Self, DispatchError> {
		match transition {
			InvestTransition::IncreaseInvestOrder(swap) => Self::handle_increase(&self, swap),
			InvestTransition::DecreaseInvestOrder(swap) => Self::handle_decrease(&self, swap),
			InvestTransition::SwapIntoPool(swap) => {
				Self::handle_fulfilled_swap_into_pool(&self, swap)
			}
			InvestTransition::SwapIntoReturn(swap) => {
				Self::handle_fulfilled_swap_into_return(&self, swap)
			}
		}
	}
}

// Actual impl of transition
impl<Balance, Currency> InvestState<Balance, Currency>
where
	Balance: Clone + Copy + EnsureAdd,
	Currency: Clone + PartialEq,
{
	// fn swap_required(&self,
	// 	return_currency: Currency,
	// 	pool_currency: Currency,
	// 	amount: Balance
	// ) -> Result<Self, DispatchError> {
	// 	if return_currency == pool_currency {

	// 	}
	// }

	/// Handle `increase` transitions depicted by `msg::increase` edges in the
	/// state diagram.
	fn handle_increase(&self, swap: Swap<Balance, Currency>) -> Result<Self, DispatchError> {
		match &self {
			Self::NoState => {
				if swap.currency_in == swap.currency_out {
					Ok(Self::InvestmentOngoing {
						invest_amount: swap.amount,
					})
				} else {
					Ok(Self::ActiveSwapIntoPoolCurrency(swap))
				}
			}
			Self::InvestmentOngoing { invest_amount } => {
				if swap.currency_in == swap.currency_out {
					Ok(Self::InvestmentOngoing {
						invest_amount: invest_amount.ensure_add(swap.amount)?,
					})
				} else {
					Ok(Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
						swap,
						invest_amount: *invest_amount,
					})
				}
			}
			Self::ActiveSwapIntoPoolCurrency(_) => todo!(),
			Self::ActiveSwapIntoReturnCurrency(_) => todo!(),
			Self::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
				swap,
				invest_amount,
			} => todo!(),
			Self::ActiveSwapIntoReturnCurrencyAndInvestmentOngoing {
				swap,
				invest_amount,
			} => todo!(),
			Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDone { swap, done_amount } => todo!(),
			Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDone { swap, done_amount } => {
				todo!()
			}
			Self::ActiveSwapIntoPoolCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				swap,
				done_amount,
				invest_amount,
			} => todo!(),
			Self::ActiveSwapIntoReturnCurrencyAndSwapIntoReturnDoneAndInvestmentOngoing {
				swap,
				done_amount,
				invest_amount,
			} => todo!(),
			Self::SwapIntoReturnDone(_) => todo!(),
			Self::SwapIntoReturnDoneAndInvestmentOngoing {
				swap,
				invest_amount,
			} => todo!(),
		}
	}

	/// Handle `decrease` transitions depicted by `msg::decrease` edges in the
	/// state diagram.
	fn handle_decrease(&self, swap: Swap<Balance, Currency>) -> Result<Self, DispatchError> {
		todo!("Do state transition here")
	}

	/// Handle partial/full token swap order transitions into pool currency
	/// depicted by `order_partial` edges in the state diagram where the swap
	/// currency matches the pool one.
	///
	/// NOTE: These should always increase the active ongoing investment.
	fn handle_fulfilled_swap_into_pool(
		&self,
		swap: Swap<Balance, Currency>,
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
		swap: Swap<Balance, Currency>,
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
