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
	investments::{ExecutedForeignDecrease, ForeignInvestmentInfo},
};
use frame_support::{traits::fungibles::Mutate, transactional};
use sp_core::Get;
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

use crate::{pallet::Config, Message, MessageOf, Pallet};

/// The hook struct which acts upon a finalized investment decrement.
pub struct DecreasedForeignInvestOrderHook<T>(PhantomData<T>);

impl<T: Config> StatusNotificationHook for DecreasedForeignInvestOrderHook<T>
where
	<T as frame_system::Config>::AccountId: Into<[u8; 32]>,
{
	type Error = DispatchError;
	type Id = ForeignInvestmentInfo<T::AccountId, T::TrancheCurrency, ()>;
	type Status = ExecutedForeignDecrease<T::Balance, T::CurrencyId>;

	#[transactional]
	fn notify_status_change(
		id: ForeignInvestmentInfo<T::AccountId, T::TrancheCurrency, ()>,
		status: ExecutedForeignDecrease<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let ForeignInvestmentInfo {
			id: investment_id,
			owner: investor,
			..
		} = id;
		let currency = Pallet::<T>::try_get_general_index(status.foreign_currency)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&status.foreign_currency)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::burn_from(status.foreign_currency, &investor, status.amount_decreased)?;

		let message: MessageOf<T> = Message::ExecutedDecreaseInvestOrder {
			pool_id: investment_id.of_pool(),
			tranche_id: investment_id.of_tranche(),
			investor: investor.into(),
			currency,
			currency_payout: status.amount_decreased,
		};
		T::OutboundQueue::submit(T::TreasuryAccount::get(), domain_address.domain(), message)?;

		Ok(())
	}
}

/// The hook struct which acts upon a finalized redemption collection.

pub struct CollectedForeignRedemptionHook<T>(PhantomData<T>);

impl<T: Config> StatusNotificationHook for CollectedForeignRedemptionHook<T>
where
	<T as frame_system::Config>::AccountId: Into<[u8; 32]>,
{
	type Error = DispatchError;
	type Id = ForeignInvestmentInfo<T::AccountId, T::TrancheCurrency, ()>;
	type Status = cfg_types::investments::ExecutedForeignCollectRedeem<T::Balance, T::CurrencyId>;

	#[transactional]
	fn notify_status_change(
		id: ForeignInvestmentInfo<T::AccountId, T::TrancheCurrency, ()>,
		status: cfg_types::investments::ExecutedForeignCollectRedeem<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let ForeignInvestmentInfo {
			id: investment_id,
			owner: investor,
			..
		} = id;
		let currency = Pallet::<T>::try_get_general_index(status.currency)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&status.currency)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::burn_from(status.currency, &investor, status.amount_currency_payout)?;

		let message: MessageOf<T> = Message::ExecutedCollectInvest {
			pool_id: investment_id.of_pool(),
			tranche_id: investment_id.of_tranche(),
			investor: investor.into(),
			currency,
			currency_payout: status.amount_currency_payout,
			tranche_tokens_payout: status.amount_tranche_tokens_payout,
		};

		T::OutboundQueue::submit(T::TreasuryAccount::get(), domain_address.domain(), message)?;

		Ok(())
	}
}
