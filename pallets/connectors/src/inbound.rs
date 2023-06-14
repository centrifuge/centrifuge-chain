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
	ops::{EnsureAdd, EnsureSub},
	Investment, InvestmentAccountant, InvestmentCollector, InvestmentProperties, PoolInspect,
	TrancheCurrency,
};
use frame_support::{
	ensure,
	traits::fungibles::{Mutate, Transfer},
};
use sp_runtime::{traits::Zero, DispatchError, DispatchResult};

use crate::{
	pallet, pallet::Error, Config, CurrencyIdOf, GeneralCurrencyIndexOf, Pallet, PoolIdOf,
	TrancheIdOf,
};

impl<T: Config> Pallet<T> {
	/// Ensures that the given pool and tranche exists and returns the
	/// corresponding investment id.
	pub fn derive_invest_id(
		pool_id: PoolIdOf<T>,
		tranche_id: TrancheIdOf<T>,
	) -> Result<<T as pallet::Config>::TrancheCurrency, DispatchError> {
		ensure!(
			T::PoolInspect::pool_exists(pool_id),
			Error::<T>::PoolNotFound
		);
		ensure!(
			T::PoolInspect::tranche_exists(pool_id, tranche_id),
			Error::<T>::TrancheNotFound
		);

		Ok(TrancheCurrency::generate(pool_id, tranche_id))
	}

	/// Ensures that the payment currency of the given investment id matches the
	/// derived currency and returns the latter.
	pub fn try_get_payment_currency(
		invest_id: <T as pallet::Config>::TrancheCurrency,
		currency_index: GeneralCurrencyIndexOf<T>,
	) -> Result<CurrencyIdOf<T>, DispatchError> {
		// retrieve currency id from general index
		let currency = Self::try_get_currency_id(currency_index)?;

		// get investment info
		let payment_currency: CurrencyIdOf<T> =
			<T as pallet::Config>::ForeignInvestmentAccountant::info(invest_id)?
				.payment_currency()
				.into();
		ensure!(
			payment_currency == currency,
			Error::<T>::InvalidInvestCurrency
		);

		Ok(currency)
	}

	/// Executes a transfer exclusively for non-tranche-tokens.
	pub fn do_transfer(
		currency: GeneralCurrencyIndexOf<T>,
		sender: T::AccountId,
		receiver: T::AccountId,
		amount: <T as pallet::Config>::Balance,
	) -> DispatchResult {
		ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);

		let currency_id = Self::try_get_currency_id(currency)?;
		T::Tokens::transfer(currency_id, &sender, &receiver, amount, false)?;

		Ok(())
	}

	/// Executes a transfer exclusively for tranche tokens.
	pub fn do_transfer_tranche_tokens(
		pool_id: PoolIdOf<T>,
		tranche_id: TrancheIdOf<T>,
		sender: T::AccountId,
		receiver: T::AccountId,
		amount: <T as pallet::Config>::Balance,
	) -> DispatchResult {
		ensure!(!amount.is_zero(), Error::<T>::InvalidTransferAmount);
		// TODO(@review): Is my assumption correct that we don't need to do permission
		// checking here?

		let invest_id = Self::derive_invest_id(pool_id, tranche_id)?;
		T::Tokens::transfer(invest_id.into(), &sender, &receiver, amount, false)?;

		Ok(())
	}

	/// Increases an existing investment order of the investor. Directly mints
	/// the additional investment amount into the investor account.
	pub fn do_increase_invest_order(
		pool_id: PoolIdOf<T>,
		tranche_id: TrancheIdOf<T>,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		amount: <T as pallet::Config>::Balance,
	) -> DispatchResult {
		// Retrieve investment details
		let invest_id: <T as Config>::TrancheCurrency =
			Self::derive_invest_id(pool_id, tranche_id)?;
		let currency = Self::try_get_payment_currency(invest_id.clone(), currency_index)?;

		// Determine post adjustment amount
		let pre_amount =
			<T as pallet::Config>::ForeignInvestment::investment(&investor, invest_id.clone())?;
		let post_amount = pre_amount.ensure_add(amount)?;

		// Mint additional amount
		<T as pallet::Config>::Tokens::mint_into(currency, &investor, amount)?;

		<T as pallet::Config>::ForeignInvestment::update_investment(
			&investor,
			invest_id,
			post_amount,
		)?;

		Ok(())
	}

	/// Decreases an existing investment order of the investor. Directly burns
	/// the decreased investment amount from the investor account.
	///
	/// Initiates a return `ExecutedDecreaseInvestOrder`
	/// message to refund the decreased amount on the source domain. The
	/// dispatch of this message is delayed until the execution of the
	/// investment, e.g. at least until the next epoch transition.
	pub fn do_decrease_invest_order(
		pool_id: PoolIdOf<T>,
		tranche_id: TrancheIdOf<T>,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		amount: <T as pallet::Config>::Balance,
	) -> DispatchResult {
		// Retrieve investment details
		let invest_id: <T as Config>::TrancheCurrency =
			Self::derive_invest_id(pool_id, tranche_id)?;
		let currency = Self::try_get_payment_currency(invest_id.clone(), currency_index)?;

		// Determine post adjustment amount
		let pre_amount =
			<T as pallet::Config>::ForeignInvestment::investment(&investor, invest_id.clone())?;
		let post_amount = pre_amount.ensure_sub(amount)?;

		<T as pallet::Config>::ForeignInvestment::update_investment(
			&investor,
			invest_id,
			post_amount,
		)?;

		// TODO(@review): We want to burn instead of transferring to some sovereign
		// account, right?
		<T as pallet::Config>::Tokens::burn_from(currency, &investor, amount)?;

		// TODO(subsequent PR): Handle response `ExecutedDecreaseInvestOrder`message to
		// source destination which should refund the decreased amount.
		// Blocked by https://github.com/centrifuge/centrifuge-chain/pull/1363
		// Should be handled by pallet-foreign-investments

		Ok(())
	}

	/// Increases an existing redemption order of the investor. Directly mints
	/// the additional redemption amount into the investor account.
	pub fn do_increase_redemption(
		pool_id: PoolIdOf<T>,
		tranche_id: TrancheIdOf<T>,
		investor: T::AccountId,
		amount: <T as pallet::Config>::Balance,
	) -> DispatchResult {
		// Retrieve investment details
		let invest_id: <T as Config>::TrancheCurrency =
			Self::derive_invest_id(pool_id, tranche_id)?;

		// Determine post adjustment amount
		let pre_amount =
			<T as pallet::Config>::ForeignInvestment::redemption(&investor, invest_id.clone())?;
		let post_amount = pre_amount.ensure_add(amount)?;

		// Mint additional amount
		<T as pallet::Config>::Tokens::mint_into(invest_id.clone().into(), &investor, amount)?;

		<T as pallet::Config>::ForeignInvestment::update_redemption(
			&investor,
			invest_id,
			post_amount,
		)?;

		Ok(())
	}

	/// Decreases an existing redemption order of the investor. Directly burns
	/// the decreased redemption amount from the investor account.
	///
	/// Initiates a return `ExecutedDecreaseRedemption`
	/// message to refund the decreased amount on the source domain. The
	/// dispatch of this message is delayed until the execution of the
	/// redemption, e.g. at least until the next epoch transition.
	pub fn do_decrease_redemption(
		pool_id: PoolIdOf<T>,
		tranche_id: TrancheIdOf<T>,
		investor: T::AccountId,
		currency_index: GeneralCurrencyIndexOf<T>,
		amount: <T as pallet::Config>::Balance,
	) -> DispatchResult {
		// Retrieve investment details
		let invest_id: <T as Config>::TrancheCurrency =
			Self::derive_invest_id(pool_id, tranche_id)?;
		// NOTE: Required for relaying `ExecutedDecreaseRedemption` message
		let _currency = Self::try_get_payment_currency(invest_id.clone(), currency_index)?;

		// Determine post adjustment amount
		let pre_amount =
			<T as pallet::Config>::ForeignInvestment::redemption(&investor, invest_id.clone())?;
		let post_amount = pre_amount.ensure_sub(amount)?;

		<T as pallet::Config>::ForeignInvestment::update_redemption(
			&investor,
			invest_id.clone(),
			post_amount,
		)?;

		// TODO(@review): We want to burn instead of transferring to some sovereign
		// account, right?
		<T as pallet::Config>::Tokens::burn_from(invest_id.into(), &investor, amount)?;

		// TODO(subsequent PR): Handle response `ExecutedDecreaseRedemption` message to
		// source destination which should refund the decreased amount.
		// Blocked by https://github.com/centrifuge/centrifuge-chain/pull/1363
		// Should be handled by pallet-foreign-investments

		Ok(())
	}

	/// Collect the results of a user's invest orders for the given investment
	/// id. If any amounts are not fulfilled, they are directly appended to the
	/// next active order for this investment.
	pub fn do_collect_investment(
		pool_id: PoolIdOf<T>,
		tranche_id: TrancheIdOf<T>,
		investor: T::AccountId,
	) -> DispatchResult {
		let invest_id: <T as Config>::TrancheCurrency =
			Self::derive_invest_id(pool_id, tranche_id)?;

		<T as pallet::Config>::ForeignInvestment::collect_investment(investor, invest_id)
	}

	/// Collect the results of a user's redeem orders for the given investment
	/// id. If any amounts are not fulfilled, they are directly appended to the
	/// next active order for this investment.
	pub fn do_collect_redemption(
		pool_id: PoolIdOf<T>,
		tranche_id: TrancheIdOf<T>,
		investor: T::AccountId,
	) -> DispatchResult {
		let invest_id: <T as Config>::TrancheCurrency =
			Self::derive_invest_id(pool_id, tranche_id)?;

		<T as pallet::Config>::ForeignInvestment::collect_redemption(investor, invest_id)
	}
}
