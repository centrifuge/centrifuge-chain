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

use cfg_traits::StatusNotificationHook;
use cfg_types::investments::{
	CollectedAmount, ForeignInvestmentInfo,
};
use frame_support::transactional;
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

use crate::{
	errors::{InvestError, RedeemError},
	types::{InvestState, InvestTransition, RedeemState, RedeemTransition, TokenSwapReason},
	Config, Error, ForeignInvestmentInfo as ForeignInvestmentInfoStorage, InvestmentState, Pallet,
	RedemptionState, SwapOf,
};

/// The hook struct which acts upon a fulfilled swap order. Depending on the
/// last swap reason, advances either the [`InvestmentState`] or
/// [`RedemptionState`].
///
/// Assumes `TokenSwaps` as caller of of the the `notify_status_change` message.
pub struct FulfilledSwapOrderHook<T>(PhantomData<T>);

// Hook execution for (partially) fulfilled token swaps which should be consumed
// by `TokenSwaps`.
impl<T: Config> StatusNotificationHook for FulfilledSwapOrderHook<T> {
	type Error = DispatchError;
	type Id = T::TokenSwapOrderId;
	type Status = SwapOf<T>;

	#[transactional]
	fn notify_status_change(
		id: T::TokenSwapOrderId,
		status: SwapOf<T>,
	) -> Result<(), DispatchError> {
		let info =
			ForeignInvestmentInfoStorage::<T>::get(id).ok_or(Error::<T>::InvestmentInfoNotFound)?;
		let reason = info
			.last_swap_reason
			.ok_or(Error::<T>::TokenSwapReasonNotFound)?;

		match reason {
			TokenSwapReason::Investment => {
				let pre_state = InvestmentState::<T>::get(&info.owner, info.id);
				let post_state = pre_state
					.transition(InvestTransition::FulfillSwapOrder(status))
					.map_err(|e| {
						// Inner error holds finer granularity but should never occur
						log::debug!("ForeignInvestment state transition error: {:?}", e);
						Error::<T>::from(InvestError::FulfillSwapOrder)
					})?;
				Pallet::<T>::apply_invest_state_transition(&info.owner, info.id, post_state)
			}
			TokenSwapReason::Redemption => {
				let pre_state = RedemptionState::<T>::get(&info.owner, info.id);
				let post_state = pre_state
					.transition(RedeemTransition::FulfillSwapOrder(status))
					.map_err(|e| {
						// Inner error holds finer granularity but should never occur
						log::debug!("ForeignInvestment state transition error: {:?}", e);
						Error::<T>::from(RedeemError::FulfillSwapOrder)
					})?;
				Pallet::<T>::apply_redeem_state_transition(&info.owner, info.id, post_state)
			}
		}
	}
}

/// The hook struct which acts upon the collection of a foreign investment.
///
/// NOTE: Only increments the collected amount and transitions the `InvestState`
/// to update the unprocessed invest amount but does not transfer back the
/// collected amounts. We expect the user do that via
/// `collect_foreign_investment`.
pub struct CollectedInvestmentHook<T>(PhantomData<T>);
impl<T: Config> StatusNotificationHook for CollectedInvestmentHook<T> {
	type Error = DispatchError;
	type Id = ForeignInvestmentInfo<T::AccountId, T::InvestmentId, ()>;
	type Status = CollectedAmount<T::Balance>;

	#[transactional]
	fn notify_status_change(
		id: ForeignInvestmentInfo<T::AccountId, T::InvestmentId, ()>,
		status: CollectedAmount<T::Balance>,
	) -> DispatchResult {
		let ForeignInvestmentInfo {
			id: investment_id,
			owner: investor,
			..
		} = id;
		let pre_state = InvestmentState::<T>::get(&investor, investment_id);

		// Exit early if there is no foreign investment
		if pre_state == InvestState::NoState {
			return Ok(());
		}

		Pallet::<T>::denote_collected_investment(&investor, investment_id, status)?;

		Ok(())
	}
}

/// The hook struct which acts upon a finalized redemption collection.
///
/// NOTE: Only increments the collected amount and transitions the `RedeemState`
/// to update the unprocessed redeem amount but does not transfer back the
/// collected amounts. We expect the user do via
/// `collect_foreign_redemption`.

pub struct CollectedRedemptionHook<T>(PhantomData<T>);
impl<T: Config> StatusNotificationHook for CollectedRedemptionHook<T> {
	type Error = DispatchError;
	type Id = ForeignInvestmentInfo<T::AccountId, T::InvestmentId, ()>;
	type Status = CollectedAmount<T::Balance>;

	#[transactional]
	fn notify_status_change(
		id: ForeignInvestmentInfo<T::AccountId, T::InvestmentId, ()>,
		status: CollectedAmount<T::Balance>,
	) -> DispatchResult {
		let ForeignInvestmentInfo {
			id: investment_id,
			owner: investor,
			..
		} = id;
		let pre_state = RedemptionState::<T>::get(&investor, investment_id);

		// Exit early if there is no foreign redemption
		if pre_state == RedeemState::NoState {
			return Ok(());
		}

		Pallet::<T>::denote_collected_redemption(&investor, investment_id, status)?;

		Ok(())
	}
}
