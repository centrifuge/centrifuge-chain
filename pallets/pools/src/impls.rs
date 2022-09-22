// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::{CurrencyPair, InvestmentAccountant, PriceValue, TrancheCurrency};
use cfg_types::InvestmentInfo;

use super::*;

impl<T: Config> PoolInspect<T::AccountId, T::CurrencyId> for Pallet<T> {
	type Moment = Moment;
	type PoolId = T::PoolId;
	type Rate = T::BalanceRatio;
	type TrancheId = T::TrancheId;

	fn pool_exists(pool_id: Self::PoolId) -> bool {
		Pool::<T>::contains_key(pool_id)
	}

	fn tranche_exists(pool_id: Self::PoolId, tranche_id: Self::TrancheId) -> bool {
		Pool::<T>::get(pool_id)
			.and_then(|pool| pool.tranches.tranche_index(&TrancheLoc::Id(tranche_id)))
			.is_some()
	}

	fn get_tranche_token_price(
		pool_id: Self::PoolId,
		tranche_id: Self::TrancheId,
	) -> Option<PriceValue<T::CurrencyId, T::BalanceRatio, Moment>> {
		let now = Self::now();
		let mut pool = Pool::<T>::get(pool_id)?;

		// Get cached nav as calculating current nav would be too computationally expensive
		let (nav, nav_last_updated) = T::NAV::nav(pool_id)?;
		let total_assets = pool.reserve.total.saturating_add(nav);

		let tranche_index: usize = pool
			.tranches
			.tranche_index(&TrancheLoc::Id(tranche_id))?
			.try_into()
			.ok()?;
		let prices = pool
			.tranches
			.calculate_prices::<T::BalanceRatio, T::Tokens, _>(total_assets, now)
			.ok()?;

		let base = pool.tranches.tranche_currency(TrancheLoc::Id(tranche_id))?;

		let price = prices.get(tranche_index).cloned()?;

		Some(PriceValue {
			pair: CurrencyPair {
				base,
				quote: pool.currency,
			},
			price,
			last_updated: nav_last_updated,
		})
	}
}

impl<T: Config> PoolReserve<T::AccountId, T::CurrencyId> for Pallet<T> {
	type Balance = T::Balance;

	fn withdraw(pool_id: Self::PoolId, to: T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::do_withdraw(to, pool_id, amount)
	}

	fn deposit(pool_id: Self::PoolId, from: T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::do_deposit(from, pool_id, amount)
	}
}

impl<T: Config> InvestmentAccountant<T::AccountId> for Pallet<T> {
	type Amount = T::Balance;
	type Error = DispatchError;
	type InvestmentId = T::TrancheCurrency;
	type InvestmentInfo = InvestmentInfo<T::AccountId, T::CurrencyId, Self::InvestmentId>;

	fn info(id: Self::InvestmentId) -> Result<Self::InvestmentInfo, Self::Error> {
		let details = Pool::<T>::get(id.of_pool()).ok_or(Error::<T>::NoSuchPool)?;
		// Need to check here, if this is a valid tranche
		let _currency = details
			.tranches
			.tranche_currency(TrancheLoc::Id(id.of_tranche()))
			.ok_or(Error::<T>::InvalidTrancheId)?;

		Ok(InvestmentInfo {
			owner: PoolLocator {
				pool_id: id.of_pool(),
			}
			.into_account_truncating(),
			id,
			payment_currency: details.currency,
		})
	}

	fn balance(id: Self::InvestmentId, who: &T::AccountId) -> Self::Amount {
		T::Tokens::balance(id.into(), who)
	}

	fn transfer(
		id: Self::InvestmentId,
		source: &T::AccountId,
		dest: &T::AccountId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		T::Tokens::transfer(id.into(), source, dest, amount, false).map(|_| ())
	}

	fn deposit(
		buyer: &T::AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		T::Tokens::mint_into(id.into(), buyer, amount)
	}

	fn withdraw(
		seller: &T::AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		T::Tokens::burn_from(id.into(), seller, amount).map(|_| ())
	}
}
