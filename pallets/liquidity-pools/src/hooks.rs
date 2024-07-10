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
use frame_support::{
	traits::{
		fungibles::Mutate,
		tokens::{Fortitude, Precision, Preservation},
	},
	transactional,
};
use sp_core::Get;
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::marker::PhantomData;

use crate::{pallet::Config, Message, Pallet};

impl<T: Config> ForeignInvestmentHooks<T::AccountId> for Pallet<T>
where
	<T as frame_system::Config>::AccountId: Into<[u8; 32]>,
{
	type Amount = T::Balance;
	type CurrencyId = T::CurrencyId;
	type InvestmentId = T::TrancheCurrency;
	type TrancheAmount = T::Balance;

	/// An async cancellation has been done
	fn fulfill_cancel_investment(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
		currency_id: Self::CurrencyId,
		amount_cancelled: Self::Amount,
		fulfilled: Self::Amount,
	) -> DispatchResult {
		todo!()
	}

	/// An async investment collection has been done
	fn fulfill_collect_investment(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
		currency_id: Self::CurrencyId,
		amount_collected: Self::Amount,
		tranche_tokens_payout: Self::TrancheAmount,
	) -> DispatchResult {
		todo!()
	}

	/// An async redemption collection has been done
	fn fulfill_collect_redemption(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
		currency_id: Self::CurrencyId,
		tranche_tokens_collected: Self::TrancheAmount,
		amount_payout: Self::Amount,
	) -> DispatchResult {
		todo!()
	}
}

/*
/// The hook struct which acts upon a finalized investment decrement.
pub struct DecreasedForeignInvestOrderHook<T>(PhantomData<T>);

impl<T: Config> StatusNotificationHook for DecreasedForeignInvestOrderHook<T>
where
	<T as frame_system::Config>::AccountId: Into<[u8; 32]>,
{
	type Error = DispatchError;
	type Id = (T::AccountId, T::TrancheCurrency);
	type Status = ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>;

	#[transactional]
	fn notify_status_change(
		(investor, investment_id): (T::AccountId, T::TrancheCurrency),
		status: ExecutedForeignDecreaseInvest<T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(status.foreign_currency)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&status.foreign_currency)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::burn_from(
			status.foreign_currency,
			&investor,
			status.amount_decreased,
			Precision::Exact,
			Fortitude::Polite,
		)?;

		let message = Message::FulfilledCancelDepositRequest {
			pool_id: investment_id.of_pool().into(),
			tranche_id: investment_id.of_tranche().into(),
			investor: investor.into(),
			currency,
			currency_payout: status.amount_decreased.into(),
			fulfilled_invest_amount: status.amount_remaining.into(),
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
	type Id = (T::AccountId, T::TrancheCurrency);
	type Status = ExecutedForeignCollect<T::Balance, T::Balance, T::Balance, T::CurrencyId>;

	#[transactional]
	fn notify_status_change(
		(investor, investment_id): (T::AccountId, T::TrancheCurrency),
		status: ExecutedForeignCollect<T::Balance, T::Balance, T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(status.currency)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&status.currency)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::burn_from(
			status.currency,
			&investor,
			status.amount_currency_payout,
			Precision::Exact,
			Fortitude::Polite,
		)?;

		let message = Message::FulfilledRedeemRequest {
			pool_id: investment_id.of_pool().into(),
			tranche_id: investment_id.of_tranche().into(),
			investor: investor.into(),
			currency,
			currency_payout: status.amount_currency_payout.into(),
			tranche_tokens_payout: status.amount_tranche_tokens_payout.into(),
		};

		T::OutboundQueue::submit(T::TreasuryAccount::get(), domain_address.domain(), message)?;

		Ok(())
	}
}

/// The hook struct which acts upon a finalized investment collection.
pub struct CollectedForeignInvestmentHook<T>(PhantomData<T>);

impl<T: Config> StatusNotificationHook for CollectedForeignInvestmentHook<T>
where
	<T as frame_system::Config>::AccountId: Into<[u8; 32]>,
{
	type Error = DispatchError;
	type Id = (T::AccountId, T::TrancheCurrency);
	type Status = ExecutedForeignCollect<T::Balance, T::Balance, T::Balance, T::CurrencyId>;

	#[transactional]
	fn notify_status_change(
		(investor, investment_id): (T::AccountId, T::TrancheCurrency),
		status: ExecutedForeignCollect<T::Balance, T::Balance, T::Balance, T::CurrencyId>,
	) -> DispatchResult {
		let currency = Pallet::<T>::try_get_general_index(status.currency)?;
		let wrapped_token = Pallet::<T>::try_get_wrapped_token(&status.currency)?;
		let domain_address: DomainAddress = wrapped_token.into();

		T::Tokens::transfer(
			investment_id.clone().into(),
			&investor,
			&domain_address.domain().into_account(),
			status.amount_tranche_tokens_payout,
			Preservation::Expendable,
		)?;

		let message = Message::FulfilledDepositRequest {
			pool_id: investment_id.of_pool().into(),
			tranche_id: investment_id.of_tranche().into(),
			investor: investor.into(),
			currency,
			currency_payout: status.amount_currency_payout.into(),
			tranche_tokens_payout: status.amount_tranche_tokens_payout.into(),
		};

		T::OutboundQueue::submit(T::TreasuryAccount::get(), domain_address.domain(), message)?;

		Ok(())
	}
}
*/
