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

use cfg_traits::{
	CurrencyPair, InvestmentAccountant, PoolUpdateGuard, PriceValue, TrancheCurrency, UpdateState,
};
use cfg_types::{epoch::EpochState, investments::InvestmentInfo};
use frame_support::traits::Contains;

use super::*;
use crate::{
	pool_types::{PoolDetails, PoolParameters, PoolStatus, ReserveDetails, ScheduledUpdateDetails},
	tranches::{TrancheInput, TrancheLoc, TrancheUpdate, Tranches},
};

impl<T: Config> PoolInspect<T::AccountId, T::CurrencyId> for Pallet<T> {
	type Moment = Moment;
	type PoolId = T::PoolId;
	type Rate = T::Rate;
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
	) -> Option<PriceValue<T::CurrencyId, T::Rate, Moment>> {
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
			.calculate_prices::<T::Rate, T::Tokens, _>(total_assets, now)
			.ok()?;

		let base = pool
			.tranches
			.tranche_currency(TrancheLoc::Id(tranche_id))?
			.into();

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

impl<T: Config> PoolMutate<T::AccountId, T::PoolId> for Pallet<T> {
	type Balance = T::Balance;
	type CurrencyId = T::CurrencyId;
	type MaxTokenNameLength = T::MaxTokenNameLength;
	type MaxTokenSymbolLength = T::MaxTokenSymbolLength;
	type MaxTranches = T::MaxTranches;
	type PoolChanges = PoolChangesOf<T>;
	type Rate = T::Rate;
	type TrancheInput = TrancheInput<T::Rate, T::MaxTokenNameLength, T::MaxTokenSymbolLength>;

	fn create(
		admin: T::AccountId,
		depositor: T::AccountId,
		pool_id: T::PoolId,
		tranche_inputs: Vec<TrancheInput<T::Rate, T::MaxTokenNameLength, T::MaxTokenSymbolLength>>,
		currency: T::CurrencyId,
		max_reserve: T::Balance,
		metadata: Option<Vec<u8>>,
	) -> DispatchResult {
		// A single pool ID can only be used by one owner.
		ensure!(!Pool::<T>::contains_key(pool_id), Error::<T>::PoolInUse);

		ensure!(
			T::PoolCurrency::contains(&currency),
			Error::<T>::InvalidCurrency
		);

		Self::is_valid_tranche_change(
			None,
			&tranche_inputs
				.iter()
				.map(|t| TrancheUpdate {
					tranche_type: t.tranche_type,
					seniority: t.seniority,
				})
				.collect(),
		)?;

		Self::take_deposit(depositor.clone(), pool_id)?;

		let now = Self::now();

		let tranches = Tranches::<
			T::Balance,
			T::Rate,
			T::TrancheWeight,
			T::TrancheCurrency,
			T::TrancheId,
			T::PoolId,
		>::from_input::<T::MaxTokenNameLength, T::MaxTokenSymbolLength>(
			pool_id,
			tranche_inputs.clone(),
			now,
		)?;

		let checked_metadata: Option<BoundedVec<u8, T::MaxSizeMetadata>> = match metadata {
			Some(metadata_value) => {
				let checked: BoundedVec<u8, T::MaxSizeMetadata> = metadata_value
					.try_into()
					.map_err(|_| Error::<T>::BadMetadata)?;

				Some(checked)
			}
			None => None,
		};

		for (tranche, tranche_input) in tranches.tranches.iter().zip(&tranche_inputs) {
			let token_name: BoundedVec<u8, T::MaxTokenNameLength> =
				tranche_input.metadata.token_name.clone();

			let token_symbol: BoundedVec<u8, T::MaxTokenSymbolLength> =
				tranche_input.metadata.token_symbol.clone();

			let decimals = match T::AssetRegistry::metadata(&currency) {
				Some(metadata) => metadata.decimals,
				None => return Err(Error::<T>::MetadataForCurrencyNotFound.into()),
			};

			let parachain_id = T::ParachainId::get();

			let metadata = tranche.create_asset_metadata(
				decimals,
				parachain_id,
				T::PalletIndex::get(),
				token_name.to_vec(),
				token_symbol.to_vec(),
			);

			T::AssetRegistry::register_asset(Some(tranche.currency.into()), metadata)
				.map_err(|_| Error::<T>::FailedToRegisterTrancheMetadata)?;
		}

		let min_epoch_time = sp_std::cmp::min(
			sp_std::cmp::max(
				T::DefaultMinEpochTime::get(),
				T::MinEpochTimeLowerBound::get(),
			),
			T::MinEpochTimeUpperBound::get(),
		);

		let max_nav_age =
			sp_std::cmp::min(T::DefaultMaxNAVAge::get(), T::MaxNAVAgeUpperBound::get());

		let pool_details = PoolDetails {
			currency,
			tranches,
			status: PoolStatus::Open,
			epoch: EpochState {
				current: One::one(),
				last_closed: now,
				last_executed: Zero::zero(),
			},
			parameters: PoolParameters {
				min_epoch_time,
				max_nav_age,
			},
			reserve: ReserveDetails {
				max: max_reserve,
				available: Zero::zero(),
				total: Zero::zero(),
			},
			metadata: checked_metadata,
		};

		Pool::<T>::insert(pool_id, pool_details.clone());

		Self::deposit_event(Event::Created {
			admin: admin.clone(),
			depositor,
			pool_id,
			essence: pool_details
				.essence::<T::AssetRegistry, T::Balance, T::MaxTokenNameLength, T::MaxTokenSymbolLength>(
				)?,
		});

		T::Permission::add(
			PermissionScope::Pool(pool_id),
			admin,
			Role::PoolRole(PoolRole::PoolAdmin),
		)?;

		Ok(())
	}

	fn update(pool_id: T::PoolId, changes: PoolChangesOf<T>) -> Result<UpdateState, DispatchError> {
		ensure!(
			EpochExecution::<T>::try_get(pool_id).is_err(),
			Error::<T>::InSubmissionPeriod
		);

		// Both changes.tranches and changes.tranche_metadata
		// have to be NoChange or Change, we don't allow to change either or
		// ^ = XOR, !^ = negated XOR
		ensure!(
			!((changes.tranches == Change::NoChange)
				^ (changes.tranche_metadata == Change::NoChange)),
			Error::<T>::InvalidTrancheUpdate
		);

		if changes.min_epoch_time == Change::NoChange
			&& changes.max_nav_age == Change::NoChange
			&& changes.tranches == Change::NoChange
		{
			// If there's an existing update, we remove it
			// If not, this transaction is a no-op
			if ScheduledUpdate::<T>::contains_key(pool_id) {
				ScheduledUpdate::<T>::remove(pool_id);
			}

			return Ok(UpdateState::NoExecution);
		}

		if let Change::NewValue(min_epoch_time) = changes.min_epoch_time {
			ensure!(
				min_epoch_time >= T::MinEpochTimeLowerBound::get()
					&& min_epoch_time <= T::MinEpochTimeUpperBound::get(),
				Error::<T>::PoolParameterBoundViolated
			);
		}

		if let Change::NewValue(max_nav_age) = changes.max_nav_age {
			ensure!(
				max_nav_age <= T::MaxNAVAgeUpperBound::get(),
				Error::<T>::PoolParameterBoundViolated
			);
		}

		let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;

		if let Change::NewValue(tranches) = &changes.tranches {
			Self::is_valid_tranche_change(Some(&pool.tranches), tranches)?;
		}

		let now = Self::now();

		let update = ScheduledUpdateDetails {
			changes: changes.clone(),
			scheduled_time: now.saturating_add(T::MinUpdateDelay::get()),
		};

		let num_tranches = pool.tranches.num_tranches().try_into().unwrap();
		if T::MinUpdateDelay::get() == 0 && T::UpdateGuard::released(&pool, &update, now) {
			Self::do_update_pool(&pool_id, &changes)?;

			Ok(UpdateState::Executed(num_tranches))
		} else {
			// If an update was already stored, this will override it
			ScheduledUpdate::<T>::insert(pool_id, update);

			Ok(UpdateState::Stored(num_tranches))
		}
	}

	fn execute_update(pool_id: T::PoolId) -> Result<u32, DispatchError> {
		let update =
			ScheduledUpdate::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoScheduledUpdate)?;

		ensure!(
			Self::now() >= update.scheduled_time,
			Error::<T>::ScheduledTimeHasNotPassed
		);

		let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;

		ensure!(
			T::UpdateGuard::released(&pool, &update, Self::now()),
			Error::<T>::UpdatePrerequesitesNotFulfilled
		);

		Self::do_update_pool(&pool_id, &update.changes)?;

		let num_tranches = pool.tranches.num_tranches().try_into().unwrap();
		Ok(num_tranches)
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
		let _details = Pool::<T>::get(id.of_pool()).ok_or(Error::<T>::NoSuchPool)?;

		T::Tokens::transfer(id.into(), source, dest, amount, false).map(|_| ())
	}

	fn deposit(
		buyer: &T::AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		let _details = Pool::<T>::get(id.of_pool()).ok_or(Error::<T>::NoSuchPool)?;

		T::Tokens::mint_into(id.into(), buyer, amount)
	}

	fn withdraw(
		seller: &T::AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		let _details = Pool::<T>::get(id.of_pool()).ok_or(Error::<T>::NoSuchPool)?;

		T::Tokens::burn_from(id.into(), seller, amount).map(|_| ())
	}
}
