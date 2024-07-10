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
	investments::{ForeignInvestmentHooks, TrancheCurrency},
	liquidity_pools::OutboundQueue,
};
use cfg_types::domain_address::DomainAddress;
use frame_support::traits::{
	fungibles::Mutate,
	tokens::{Fortitude, Precision, Preservation},
};
use sp_core::Get;
use sp_runtime::DispatchResult;

use crate::{pallet::Config, Message, Pallet};

impl<T: Config> ForeignInvestmentHooks<T::AccountId> for Pallet<T>
where
	<T as frame_system::Config>::AccountId: Into<[u8; 32]>,
{
	type Amount = T::Balance;
	type CurrencyId = T::CurrencyId;
	type InvestmentId = T::TrancheCurrency;
	type TrancheAmount = T::Balance;

	fn fulfill_cancel_investment(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
		currency_id: Self::CurrencyId,
		amount_cancelled: Self::Amount,
		fulfilled: Self::Amount,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(currency_id)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&currency_id)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::burn_from(
			currency_id,
			&who,
			amount_cancelled,
			Precision::Exact,
			Fortitude::Polite,
		)?;

		let message = Message::FulfilledCancelDepositRequest {
			pool_id: investment_id.of_pool().into(),
			tranche_id: investment_id.of_tranche().into(),
			investor: who.clone().into(),
			currency,
			currency_payout: amount_cancelled.into(),
			fulfilled_invest_amount: fulfilled.into(),
		};

		T::OutboundQueue::submit(T::TreasuryAccount::get(), domain_address.domain(), message)
	}

	fn fulfill_collect_investment(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
		currency_id: Self::CurrencyId,
		amount_collected: Self::Amount,
		tranche_tokens_payout: Self::TrancheAmount,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(currency_id)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&currency_id)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::transfer(
			investment_id.clone().into(),
			&who,
			&domain_address.domain().into_account(),
			tranche_tokens_payout,
			Preservation::Expendable,
		)?;

		let message = Message::FulfilledDepositRequest {
			pool_id: investment_id.of_pool().into(),
			tranche_id: investment_id.of_tranche().into(),
			investor: who.clone().into(),
			currency,
			currency_payout: amount_collected.into(),
			tranche_tokens_payout: tranche_tokens_payout.into(),
		};

		T::OutboundQueue::submit(T::TreasuryAccount::get(), domain_address.domain(), message)
	}

	fn fulfill_collect_redemption(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
		currency_id: Self::CurrencyId,
		tranche_tokens_collected: Self::TrancheAmount,
		amount_payout: Self::Amount,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(currency_id)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&currency_id)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::burn_from(
			currency_id,
			&who,
			amount_payout,
			Precision::Exact,
			Fortitude::Polite,
		)?;

		let message = Message::FulfilledRedeemRequest {
			pool_id: investment_id.of_pool().into(),
			tranche_id: investment_id.of_tranche().into(),
			investor: who.clone().into(),
			currency,
			currency_payout: amount_payout.into(),
			tranche_tokens_payout: tranche_tokens_collected.into(),
		};

		T::OutboundQueue::submit(T::TreasuryAccount::get(), domain_address.domain(), message)
	}
}
