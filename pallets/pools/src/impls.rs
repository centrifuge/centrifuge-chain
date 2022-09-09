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

use cfg_traits::{CurrencyPair, PriceValue};

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
