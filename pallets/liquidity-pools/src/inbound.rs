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
	investments::ForeignInvestment, liquidity_pools::OutboundQueue, Permissions, PoolInspect,
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	investments::ExecutedForeignCollectInvest,
	permissions::{PermissionScope, PoolRole, Role},
};
use frame_support::{
	ensure,
	traits::fungibles::{Mutate, Transfer},
};
use sp_runtime::{
	traits::{Convert, Zero},
	DispatchResult,
};

use crate::{pallet::Error, Config, GeneralCurrencyIndexOf, Message, MessageOf, Pallet};

impl<T: Config> Pallet<T>
where
	T::AccountId: Into<[u8; 32]>,
{
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
		sending_domain: DomainAddress,
		receiver: T::AccountId,
		amount: <T as Config>::Balance,
	) -> DispatchResult {
		ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);

		ensure!(
			T::Permission::has(
				PermissionScope::Pool(pool_id),
				receiver.clone(),
				Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Self::now())),
			),
			Error::<T>::UnauthorizedTransfer
		);

		let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;

		T::Tokens::transfer(
			invest_id.into(),
			&Domain::convert(sending_domain.domain()),
			&receiver,
			amount,
			false,
		)?;

		Ok(())
	}

	/// Increases an existing investment order of the investor.
	///
	/// Directly mints the additional investment amount into the investor
	/// account.
	///
	/// If the provided currency does not match the pool currency, a token swap
	/// is initiated.
	pub fn handle_increase_invest_order(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		amount: <T as Config>::Balance,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		let payment_currency = Self::try_get_payment_currency(invest_id.clone(), currency_index)?;
		let pool_currency =
			T::PoolInspect::currency_for(pool_id).ok_or(Error::<T>::PoolNotFound)?;

		// Mint additional amount of payment currency
		T::Tokens::mint_into(payment_currency, &investor, amount)?;

		T::ForeignInvestment::increase_foreign_investment(
			&investor,
			invest_id,
			amount,
			payment_currency,
			pool_currency,
		)?;

		Ok(())
	}

	/// Initiates the decrement of an existing investment order of the investor.
	///
	/// On success, the unprocessed investment amount is decremented and a swap
	/// back into the provided foreign currency initiated.
	///
	/// The finalization of this call (fulfillment of the swap) is assumed to be
	/// asynchronous. In any case, it is handled by `DecreaseInvestOrderHook`
	/// which burns the corresponding amount in foreign currency and dispatches
	/// `ExecutedDecreaseInvestOrder`.
	pub fn handle_decrease_invest_order(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		amount: <T as Config>::Balance,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		let payment_currency = Self::try_get_payment_currency(invest_id.clone(), currency_index)?;
		let pool_currency =
			T::PoolInspect::currency_for(pool_id).ok_or(Error::<T>::PoolNotFound)?;

		T::ForeignInvestment::decrease_foreign_investment(
			&investor,
			invest_id,
			amount,
			payment_currency,
			pool_currency,
		)?;

		Ok(())
	}

	/// Cancels an invest order by decreasing by the entire unprocessed
	/// investment amount.
	///
	/// On success, initiates a swap back into the provided foreign currency.
	///
	/// The finalization of this call (fulfillment of the swap) is assumed to be
	/// asynchronous. In any case, it is handled by `DecreaseInvestOrderHook`
	/// which burns the corresponding amount in foreign currency and dispatches
	/// `ExecutedDecreaseInvestOrder`.
	pub fn handle_cancel_invest_order(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		let amount = T::ForeignInvestment::investment(&investor, invest_id)?;
		Self::handle_decrease_invest_order(pool_id, tranche_id, investor, currency_index, amount)
	}

	/// Increases an existing redemption order of the investor.
	///
	/// Transfers the increase redemption amount from the holdings of the
	/// `DomainLocator` account of origination domain of this message into the
	/// investor account.
	///
	/// Assumes that the amount of tranche tokens has been locked in the
	/// `DomainLocator` account of the origination domain beforehand.
	pub fn handle_increase_redeem_order(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		amount: <T as Config>::Balance,
		sending_domain: DomainAddress,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;

		// Transfer tranche tokens from `DomainLocator` account of
		// origination domain
		// TODO(@review): Should this rather be pat of `increase_foreign_redemption`?
		T::Tokens::transfer(
			invest_id.clone().into(),
			&Domain::convert(sending_domain.domain()),
			&investor,
			amount,
			false,
		)?;

		T::ForeignInvestment::increase_foreign_redemption(&investor, invest_id, amount)?;

		Ok(())
	}

	/// Decreases an existing redemption order of the investor.
	///
	/// Initiates a return `ExecutedDecreaseRedemption` message to refund the
	/// decreased amount on the source domain.
	///
	/// NOTE: In contrast to investments, redemption decrements happen
	/// fully synchronously as they can only be called in between increasing a
	/// redemption and its (full) processing.
	pub fn handle_decrease_redeem_order(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		amount: <T as Config>::Balance,
		destination: DomainAddress,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		// TODO(@review): This is exactly `amount` as we can only decrement up to the
		// unprocessed redemption
		let tranche_tokens_payout = T::ForeignInvestment::decrease_foreign_redemption(
			&investor,
			invest_id.clone(),
			amount,
		)?;

		T::Tokens::transfer(
			invest_id.into(),
			&investor,
			&Domain::convert(destination.domain()),
			tranche_tokens_payout,
			false,
		)?;

		let message: MessageOf<T> = Message::ExecutedDecreaseRedeemOrder {
			pool_id,
			tranche_id,
			investor: investor.clone().into(),
			currency: currency_index.index,
			tranche_tokens_payout,
		};

		// TODO: Collect fee from treasury instead
		T::OutboundQueue::submit(investor, destination.domain(), message)?;

		Ok(())
	}

	/// Cancels an existing redemption order of the investor by decreasing the
	/// redemption by the entire unprocessed amount.
	///
	/// Initiates a return `ExecutedDecreaseRedemption` message to refund the
	/// decreased amount on the source domain.
	pub fn handle_cancel_redeem_order(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		destination: DomainAddress,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		let amount = T::ForeignInvestment::redemption(&investor, invest_id)?;
		Self::handle_decrease_redeem_order(
			pool_id,
			tranche_id,
			investor,
			currency_index,
			amount,
			destination,
		)
	}

	/// Collect the results of a user's invest orders for the given investment
	/// id. If any amounts are not fulfilled, they are directly appended to the
	/// next active order for this investment.
	///
	/// Transfers collected amount from investor's sovereign account to the
	/// sending domain locator.
	///
	/// NOTE: In contrast to collecting a redemption, investments can be
	/// collected entirely synchronously as it does not involve swapping. It
	/// simply transfers the tranche tokens from the pool to the sovereign
	/// investor account on the local domain.
	pub fn handle_collect_investment(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		destination: DomainAddress,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		let currency_index_u128 = currency_index.index;
		let payment_currency = Self::try_get_payment_currency(invest_id.clone(), currency_index)?;
		let pool_currency =
			T::PoolInspect::currency_for(pool_id).ok_or(Error::<T>::PoolNotFound)?;

		let ExecutedForeignCollectInvest::<T::Balance> {
			amount_currency_payout,
			amount_tranche_tokens_payout,
		} = T::ForeignInvestment::collect_foreign_investment(
			&investor,
			invest_id.clone(),
			payment_currency,
			pool_currency,
		)?;

		T::Tokens::transfer(
			invest_id.into(),
			&investor,
			&Domain::convert(destination.domain()),
			amount_tranche_tokens_payout,
			false,
		)?;

		let message: MessageOf<T> = Message::ExecutedCollectInvest {
			pool_id,
			tranche_id,
			investor: investor.clone().into(),
			currency: currency_index_u128,
			currency_payout: amount_currency_payout,
			tranche_tokens_payout: amount_tranche_tokens_payout,
		};

		// TODO: Collect fee from treasury instead
		T::OutboundQueue::submit(investor, destination.domain(), message)?;

		Ok(())
	}

	/// Collect the results of a user's redeem orders for the given investment
	/// id in the pool currency. If any amounts are not fulfilled, they are
	/// directly appended to the next active order for this investment.
	///
	/// On success, a swap will be initiated to exchange the (partially)
	/// collected amount in pool currency into the desired foreign currency.
	///
	/// The termination of this call (fulfillment of the swap) is assumed to be
	/// asynchronous and handled by the `CollectRedeemHook`. It burns the return
	/// currency amount and dispatches `Message::ExecutedCollectRedeem` to the
	/// destination domain.
	pub fn handle_collect_redemption(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		let payment_currency = Self::try_get_payment_currency(invest_id.clone(), currency_index)?;
		let pool_currency =
			T::PoolInspect::currency_for(pool_id).ok_or(Error::<T>::PoolNotFound)?;

		T::ForeignInvestment::collect_foreign_redemption(
			&investor,
			invest_id,
			payment_currency,
			pool_currency,
		)?;

		Ok(())
	}
}
