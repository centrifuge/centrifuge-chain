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

use cfg_traits::{investments::ForeignInvestment, liquidity_pools::OutboundMessageHandler};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{
	ensure,
	traits::{fungibles::Mutate, tokens::Preservation, OriginTrait},
};
use sp_core::Get;
use sp_runtime::{traits::Zero, DispatchResult};

use crate::{pallet::Error, Config, GeneralCurrencyIndexOf, Message, Pallet};

impl<T: Config> Pallet<T> {
	/// Executes a transfer from another domain exclusively for
	/// non-tranche-tokens.
	///
	/// Directly mints the currency into the receiver address.
	pub fn handle_transfer(
		currency: GeneralCurrencyIndexOf<T>,
		receiver: T::AccountId,
		amount: <T as Config>::Balance,
	) -> DispatchResult {
		ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);

		let currency_id = Self::try_get_currency_id(currency)?;
		T::Tokens::mint_into(currency_id, &receiver, amount)?;

		Ok(())
	}

	/// Executes a transfer from the `DomainLocator` account of the origination
	/// domain to the receiver exclusively for tranche tokens.
	///
	/// Assumes that the amount of tranche tokens has been locked in the
	/// `DomainLocator` account of the origination domain beforehand.
	pub fn handle_tranche_tokens_transfer(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		sending_domain: Domain,
		receiver: DomainAddress,
		amount: <T as Config>::Balance,
	) -> DispatchResult {
		ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);

		let local_representation_of_receiver = receiver.account();

		Self::validate_investor_can_transfer(
			local_representation_of_receiver.clone(),
			pool_id,
			tranche_id,
		)?;

		let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;

		T::Tokens::transfer(
			invest_id.into(),
			&sending_domain.into_account(),
			&local_representation_of_receiver,
			amount,
			Preservation::Expendable,
		)?;

		// If the receiver is not on the Centrifuge domain we need to forward it now
		// to the right domain from the holdings of the receiver we just transferred
		// them to.
		if receiver.domain() != Domain::Centrifuge {
			Pallet::<T>::transfer_tranche_tokens(
				T::RuntimeOrigin::signed(local_representation_of_receiver),
				pool_id,
				tranche_id,
				receiver,
				amount,
			)?;
		}

		Ok(())
	}

	/// Increases an existing investment order of the investor.
	///
	/// Directly mints the additional investment amount into the investor
	/// account.
	///
	/// If the provided currency does not match the pool currency, a token swap
	/// is initiated.
	pub fn handle_deposit_request(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		amount: <T as Config>::Balance,
	) -> DispatchResult {
		let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;
		let payment_currency = Self::try_get_currency_id(currency_index)?;

		// Mint additional amount of payment currency
		T::Tokens::mint_into(payment_currency, &investor, amount)?;

		T::ForeignInvestment::increase_foreign_investment(
			&investor,
			invest_id,
			amount,
			payment_currency,
		)?;

		Ok(())
	}

	/// Cancels an investment order.
	/// No more invested amount is in the system after calling this method.
	///
	/// Finalizing this action is asynchronous.
	/// The cancellation can be considered fully finished
	/// when `fulfilled_cancel_investment()` hook is called,
	/// which will respond with the `FulfilledCancelDepositRequest`.
	pub fn handle_cancel_deposit_request(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
	) -> DispatchResult {
		let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;
		let payout_currency = Self::try_get_currency_id(currency_index)?;

		T::ForeignInvestment::cancel_foreign_investment(&investor, invest_id, payout_currency)
	}

	/// Increases an existing redemption order of the investor.
	///
	/// Transfers the increase redemption amount from the holdings of the
	/// `DomainLocator` account of origination domain of this message into the
	/// investor account.
	///
	/// Assumes that the amount of tranche tokens has been locked in the
	/// `DomainLocator` account of the origination domain beforehand.
	pub fn handle_redeem_request(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		amount: <T as Config>::Balance,
		currency_index: GeneralCurrencyIndexOf<T>,
		sending_domain: DomainAddress,
	) -> DispatchResult {
		let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;
		let payout_currency = Self::try_get_currency_id(currency_index)?;

		// Transfer tranche tokens from `DomainLocator` account of
		// origination domain
		T::Tokens::transfer(
			invest_id.into(),
			&sending_domain.domain().into_account(),
			&investor,
			amount,
			Preservation::Expendable,
		)?;

		T::ForeignInvestment::increase_foreign_redemption(
			&investor,
			invest_id,
			amount,
			payout_currency,
		)?;

		Ok(())
	}

	/// Cancels an redemption order.
	/// No more redeemed amount is in the system after calling this method.
	///
	/// Initiates a return `FulfilledCancelRedeemRequest` message to refund the
	/// decreased amount on the source domain.
	pub fn handle_cancel_redeem_request(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		destination: DomainAddress,
	) -> DispatchResult {
		let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;
		let currency_u128 = currency_index.index;
		let payout_currency = Self::try_get_currency_id(currency_index)?;

		let amount =
			T::ForeignInvestment::cancel_foreign_redemption(&investor, invest_id, payout_currency)?;

		T::Tokens::transfer(
			invest_id.into(),
			&investor,
			&destination.domain().into_account(),
			amount,
			Preservation::Expendable,
		)?;

		let message = Message::FulfilledCancelRedeemRequest {
			pool_id: pool_id.into(),
			tranche_id: tranche_id.into(),
			investor: investor.clone().into(),
			currency: currency_u128,
			tranche_tokens_payout: amount.into(),
		};

		T::OutboundMessageHandler::handle(
			T::TreasuryAccount::get(),
			destination.domain(),
			message,
		)?;

		Ok(())
	}
}
