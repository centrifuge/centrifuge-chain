// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::{
	investments::TrancheCurrency, liquidity_pools::OutboundQueue, StatusNotificationHook,
};
use cfg_types::{
	domain_address::DomainAddress,
	investments::{ExecutedDecrease, ForeignInvestmentInfo},
};
use frame_support::{traits::fungibles::Mutate, transactional};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

use crate::{pallet::Config, Message, MessageOf, Pallet};

// TODO: Docs
pub struct DecreaseInvestOrderHook<T>(PhantomData<T>);

// TODO: Docs

pub struct CollectRedeemHook<T>(PhantomData<T>);

impl<T: Config> StatusNotificationHook for DecreaseInvestOrderHook<T>
where
	<T as frame_system::Config>::AccountId: Into<[u8; 32]>,
{
	type Error = DispatchError;
	type Id = ForeignInvestmentInfo<T::AccountId, T::TrancheCurrency>;
	type Status = ExecutedDecrease<T::Balance, T::CurrencyId>;

	#[transactional]
	fn notify_status_change(
		id: ForeignInvestmentInfo<T::AccountId, T::TrancheCurrency>,
		status: ExecutedDecrease<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let ForeignInvestmentInfo {
			id: investment_id,
			owner: investor,
		} = id;
		let currency = Pallet::<T>::try_get_general_index(status.return_currency)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&status.return_currency)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::burn_from(status.return_currency, &investor, status.amount_decreased)?;

		let message: MessageOf<T> = Message::ExecutedDecreaseInvestOrder {
			pool_id: investment_id.of_pool(),
			tranche_id: investment_id.of_tranche(),
			investor: investor.clone().into(),
			currency,
			currency_payout: status.amount_decreased,
			remaining_invest_order: status.amount_remaining,
		};

		T::OutboundQueue::submit(investor, domain_address.into(), message)?;

		Ok(())
	}
}

impl<T: Config> StatusNotificationHook for CollectRedeemHook<T>
where
	<T as frame_system::Config>::AccountId: Into<[u8; 32]>,
{
	type Error = DispatchError;
	type Id = ForeignInvestmentInfo<T::AccountId, T::TrancheCurrency>;
	type Status = cfg_types::investments::ExecutedCollectRedeem<T::Balance, T::CurrencyId>;

	#[transactional]
	fn notify_status_change(
		id: ForeignInvestmentInfo<T::AccountId, T::TrancheCurrency>,
		status: cfg_types::investments::ExecutedCollectRedeem<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let ForeignInvestmentInfo {
			id: investment_id,
			owner: investor,
		} = id;
		let currency = Pallet::<T>::try_get_general_index(status.currency)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&status.currency)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::burn_from(status.currency, &investor, status.amount_currency_payout)?;

		let message: MessageOf<T> = Message::ExecutedCollectInvest {
			pool_id: investment_id.of_pool(),
			tranche_id: investment_id.of_tranche(),
			investor: investor.clone().into(),
			currency,
			currency_payout: status.amount_currency_payout,
			tranche_tokens_payout: status.amount_tranche_tokens_payout,
			remaining_invest_order: status.amount_remaining,
		};

		T::OutboundQueue::submit(investor, domain_address.into(), message)?;

		Ok(())
	}
}
