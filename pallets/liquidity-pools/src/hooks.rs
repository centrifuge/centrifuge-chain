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

use cfg_traits::{investments::ForeignInvestmentHooks, liquidity_pools::OutboundMessageHandler};
use cfg_types::domain_address::{Domain, LocalAddress};
use frame_support::traits::{
	fungibles::Mutate,
	tokens::{Fortitude, Precision, Preservation},
};
use sp_core::Get;
use sp_runtime::DispatchResult;

use crate::{pallet::Config, Message, Pallet};

impl<T: Config> ForeignInvestmentHooks<T::AccountId> for Pallet<T>
where
	T::AccountId: From<LocalAddress> + Into<LocalAddress>,
{
	type Amount = T::Balance;
	type CurrencyId = T::CurrencyId;
	type InvestmentId = (T::PoolId, T::TrancheId);
	type TrancheAmount = T::Balance;

	fn fulfill_cancel_investment(
		who: &T::AccountId,
		(pool_id, tranche_id): (T::PoolId, T::TrancheId),
		currency_id: Self::CurrencyId,
		amount_cancelled: Self::Amount,
		fulfilled: Self::Amount,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(currency_id)?;
		let (chain_id, ..) = Pallet::<T>::try_get_wrapped_token(&currency_id)?;
		let domain = Domain::Evm(chain_id);

		T::Tokens::burn_from(
			currency_id,
			who,
			amount_cancelled,
			Precision::Exact,
			Fortitude::Polite,
		)?;

		let message = Message::FulfilledCancelDepositRequest {
			pool_id: pool_id.into(),
			tranche_id: tranche_id.into(),
			investor: who.clone().into(),
			currency,
			currency_payout: amount_cancelled.into(),
			fulfilled_invest_amount: fulfilled.into(),
		};

		T::OutboundMessageHandler::handle(T::TreasuryAccount::get(), domain, message)
	}

	fn fulfill_collect_investment(
		who: &T::AccountId,
		(pool_id, tranche_id): (T::PoolId, T::TrancheId),
		currency_id: Self::CurrencyId,
		amount_collected: Self::Amount,
		tranche_tokens_payout: Self::TrancheAmount,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(currency_id)?;
		let (chain_id, ..) = Pallet::<T>::try_get_wrapped_token(&currency_id)?;
		let domain = Domain::Evm(chain_id);

		T::Tokens::transfer(
			(pool_id, tranche_id).into(),
			who,
			&domain.into_account(),
			tranche_tokens_payout,
			Preservation::Expendable,
		)?;

		let message = Message::FulfilledDepositRequest {
			pool_id: pool_id.into(),
			tranche_id: tranche_id.into(),
			investor: who.clone().into(),
			currency,
			currency_payout: amount_collected.into(),
			tranche_tokens_payout: tranche_tokens_payout.into(),
		};

		T::OutboundMessageHandler::handle(T::TreasuryAccount::get(), domain, message)
	}

	fn fulfill_collect_redemption(
		who: &T::AccountId,
		(pool_id, tranche_id): (T::PoolId, T::TrancheId),
		currency_id: Self::CurrencyId,
		tranche_tokens_collected: Self::TrancheAmount,
		amount_payout: Self::Amount,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(currency_id)?;
		let (chain_id, ..) = Pallet::<T>::try_get_wrapped_token(&currency_id)?;
		let domain = Domain::Evm(chain_id);

		T::Tokens::burn_from(
			currency_id,
			who,
			amount_payout,
			Precision::Exact,
			Fortitude::Polite,
		)?;

		let message = Message::FulfilledRedeemRequest {
			pool_id: pool_id.into(),
			tranche_id: tranche_id.into(),
			investor: who.clone().into(),
			currency,
			currency_payout: amount_payout.into(),
			tranche_tokens_payout: tranche_tokens_collected.into(),
		};

		T::OutboundMessageHandler::handle(T::TreasuryAccount::get(), domain, message)
	}
}
