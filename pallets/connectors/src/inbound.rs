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

use cfg_traits::{ForeignInvestment, Permissions, PoolInspect};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
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

use crate::{pallet::Error, Config, GeneralCurrencyIndexOf, Pallet};

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
			T::PoolInspect::currency_for(pool_id).ok_or_else(|| Error::<T>::PoolNotFound)?;

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

	/// Decreases an existing investment order of the investor.
	///
	/// Initiates a return `ExecutedDecreaseInvestOrder` message to refund the
	/// decreased amount on the source domain. The dispatch of this message is
	/// delayed until the execution of the investment, e.g. at least until the
	/// next epoch transition.
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
			T::PoolInspect::currency_for(pool_id).ok_or_else(|| Error::<T>::PoolNotFound)?;

		T::ForeignInvestment::decrease_foreign_investment(
			&investor,
			invest_id,
			amount,
			payment_currency,
			pool_currency,
		)?;

		// TODO: Handle response `ExecutedDecreaseInvestOrder` message to
		// source destination which should refund the decreased amount. This includes
		// burning it from the investor account.
		//
		// NOTES:
		// 	* Blocked by https://github.com/centrifuge/centrifuge-chain/pull/1363
		// 	* Should be handled by `pallet-foreign-investment`
		//  * Requires notification of `currency_payout` and `remaining_invest_order`
		//    balances

		Ok(())
	}

	/// Increases an existing redemption order of the investor.
	// FIXME: This does not make sense? Once the redemption is processed and collected, the foreign
	// investment swaps it into return currency into the investor, not the domainlocator.
	/// Transfers the decreased redemption amount from the holdings of the
	/// `DomainLocator` account of origination domain of this message into the
	/// investor account.
	///
	/// Assumes that the amount of tranche tokens has been locked in the
	/// `DomainLocator` account of the origination domain beforehand.
	pub fn handle_increase_redemption(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		amount: <T as Config>::Balance,
		sending_domain: DomainAddress,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		T::ForeignInvestment::increase_foreign_redemption(&investor, invest_id, amount)?;

		Ok(())

		// TODO: Handle transfer in hook or drop entirely
		// // Transfer tranche tokens from `DomainLocator` account of
		// origination domain T::Tokens::transfer(
		// 	invest_id.clone().into(),
		// 	&Domain::convert(sending_domain.domain()),
		// 	&investor,
		// 	amount,
		// 	false,
		// )?;
	}

	/// Decreases an existing redemption order of the investor.
	///
	/// Initiates a return `ExecutedDecreaseRedemption` message to refund the
	/// decreased amount on the source domain. The dispatch of this message is
	/// delayed until the execution of the redemption, e.g. at least until the
	/// next epoch transition.
	pub fn handle_decrease_redemption(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		amount: <T as Config>::Balance,
		_sending_domain: DomainAddress,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		T::ForeignInvestment::decrease_foreign_redemption(&investor, invest_id, amount)?;

		Ok(())

		// TODO: Handle response `ExecutedDecreaseRedemption` message to
		// source destination which should refund the decreased amount. This
		// includes transferring the amount from the investor to the domain
		// locator account of the origination domain.
		//
		// NOTES:
		// 	* Blocked by https://github.com/centrifuge/centrifuge-chain/pull/1363
		// 	* Should be handled by `pallet-foreign-investment`
		//  * Requires notification of `tranche_tokens_payout` and
		//    `remaining_redeem_order` balances
	}

	/// Collect the results of a user's invest orders for the given investment
	/// id. If any amounts are not fulfilled, they are directly appended to the
	/// next active order for this investment.
	pub fn handle_collect_investment(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;

		T::ForeignInvestment::collect_foreign_investment(&investor, invest_id)?;

		// T::ForeignInvestment::collect_foreign_investment(investor, invest_id, )

		// TODO: Handle response `ExecutedCollectInvest` message to
		// source destination.
		//
		// Requires notification of `currency_payout`, `tranche_tokens_payout` and
		// `remaining_invest_order` balances as well as the payout currency id, which
		// needs to be mapped to its general index.

		Ok(())
	}

	/// Collect the results of a user's redeem orders for the given investment
	/// id. If any amounts are not fulfilled, they are directly appended to the
	/// next active order for this investment.
	pub fn handle_collect_redemption(
		pool_id: T::PoolId,
		tranche_id: T::TrancheId,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
	) -> DispatchResult {
		let invest_id: T::TrancheCurrency = Self::derive_invest_id(pool_id, tranche_id)?;
		let payment_currency = Self::try_get_payment_currency(invest_id.clone(), currency_index)?;
		let pool_currency =
			T::PoolInspect::currency_for(pool_id).ok_or_else(|| Error::<T>::PoolNotFound)?;

		T::ForeignInvestment::collect_foreign_redemption(
			&investor,
			invest_id,
			payment_currency,
			pool_currency,
		)?;

		// TODO: Handle response `ExecutedCollectRedeem` message to
		// source destination.
		//
		// Requires notification of `currency_payout`, `tranche_tokens_payout` and
		// `remaining_redeem_order` balances as well as the payout currency id, which
		// needs to be mapped to its general index.

		Ok(())
	}

	// TODO: At some point, some token transfer needs to happen for redemptions and
	// decrease investment
}
