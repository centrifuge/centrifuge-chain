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
	changes::ChangeGuard,
	fee::{PoolFeeBucket, PoolFeesMutate},
	investments::{InvestmentAccountant, TrancheCurrency},
	PoolUpdateGuard, TrancheTokenPrice, UpdateState,
};
use cfg_types::{epoch::EpochState, investments::InvestmentInfo, pools::PoolFeeInfo};
use frame_support::traits::{
	tokens::{Fortitude, Precision, Preservation},
	Contains,
};
use sp_runtime::traits::Hash;

use super::*;
use crate::{
	pool_types::{
		changes::{NotedPoolChange, Requirement},
		PoolDetails, PoolParameters, PoolStatus, ReserveDetails, ScheduledUpdateDetails,
	},
	tranches::{TrancheInput, TrancheLoc, TrancheUpdate, Tranches},
};

impl<T: Config> PoolInspect<T::AccountId, T::CurrencyId> for Pallet<T> {
	type Moment = Seconds;
	type PoolId = T::PoolId;
	type TrancheId = T::TrancheId;

	fn pool_exists(pool_id: Self::PoolId) -> bool {
		Pool::<T>::contains_key(pool_id)
	}

	fn tranche_exists(pool_id: Self::PoolId, tranche_id: Self::TrancheId) -> bool {
		Pool::<T>::get(pool_id)
			.and_then(|pool| pool.tranches.tranche_index(&TrancheLoc::Id(tranche_id)))
			.is_some()
	}

	fn account_for(pool_id: Self::PoolId) -> T::AccountId {
		PoolLocator { pool_id }.into_account_truncating()
	}

	fn currency_for(pool_id: Self::PoolId) -> Option<T::CurrencyId> {
		Pool::<T>::get(pool_id).map(|pool| pool.currency)
	}
}

impl<T: Config> TrancheTokenPrice<T::AccountId, T::CurrencyId> for Pallet<T> {
	type BalanceRatio = T::BalanceRatio;
	type Moment = Seconds;
	type PoolId = T::PoolId;
	type TrancheId = T::TrancheId;

	fn get_price(
		pool_id: Self::PoolId,
		tranche_id: Self::TrancheId,
	) -> Option<(T::BalanceRatio, Seconds)> {
		let mut pool = Pool::<T>::get(pool_id)?;

		let (nav_loans, nav_loans_updates) = T::AssetsUnderManagementNAV::nav(pool_id)?;
		let (nav_fees, nav_fees_updated) = T::PoolFeesNAV::nav(pool_id)?;

		let nav = Nav::new(nav_loans, nav_fees);
		let total_assets = nav
			.total(pool.reserve.total)
			.unwrap_or(<T as Config>::Balance::zero());

		let tranche_index: usize = pool
			.tranches
			.tranche_index(&TrancheLoc::Id(tranche_id))?
			.try_into()
			.ok()?;
		let prices = pool
			.tranches
			.calculate_prices::<T::BalanceRatio, T::Tokens, _>(total_assets, T::Time::now())
			.ok()?;

		let price = prices.get(tranche_index).cloned()?;

		Some((price, sp_std::cmp::min(nav_fees_updated, nav_loans_updates)))
	}
}

impl<T: Config> PoolMutate<T::AccountId, T::PoolId> for Pallet<T> {
	type Balance = T::Balance;
	type CurrencyId = T::CurrencyId;
	type PoolChanges = PoolChangesOf<T>;
	type PoolFeeInput = (
		PoolFeeBucket,
		PoolFeeInfo<T::AccountId, T::Balance, T::Rate>,
	);
	type TrancheInput = TrancheInput<T::Rate, T::StringLimit>;

	fn create(
		admin: T::AccountId,
		depositor: T::AccountId,
		pool_id: T::PoolId,
		tranche_inputs: Vec<TrancheInput<T::Rate, T::StringLimit>>,
		currency: T::CurrencyId,
		max_reserve: T::Balance,
		pool_fees: Vec<Self::PoolFeeInput>,
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
				.collect::<Vec<_>>(),
		)?;

		Self::take_deposit(depositor.clone(), pool_id)?;

		let now = T::Time::now();

		let tranches = Tranches::<
			T::Balance,
			T::Rate,
			T::TrancheWeight,
			T::TrancheCurrency,
			T::TrancheId,
			T::PoolId,
			T::MaxTranches,
		>::from_input::<T::StringLimit>(pool_id, tranche_inputs.clone(), now)?;

		let pool_details = PoolDetails {
			currency,
			tranches: tranches.clone(),
			status: PoolStatus::Open,
			epoch: EpochState {
				current: One::one(),
				last_closed: now,
				last_executed: Zero::zero(),
			},
			parameters: PoolParameters {
				min_epoch_time: T::DefaultMinEpochTime::get(),
				max_nav_age: T::DefaultMaxNAVAge::get(),
			},
			reserve: ReserveDetails {
				max: max_reserve,
				available: Zero::zero(),
				total: Zero::zero(),
			},
		};

		Pool::<T>::insert(pool_id, pool_details.clone());

		// For SubQuery, pool creation event should be dispatched before related events
		let ids: Vec<T::TrancheCurrency> = tranches
			.tranches
			.clone()
			.into_iter()
			.map(|tranche| tranche.currency)
			.collect();
		Self::deposit_event(Event::Created {
			admin: admin.clone(),
			depositor,
			pool_id,
			essence: pool_details
				.essence_from_tranche_input::<T::StringLimit>(ids, tranche_inputs.clone())?,
		});

		for (tranche, tranche_input) in tranches.tranches.iter().zip(&tranche_inputs) {
			let token_name = tranche_input.metadata.token_name.clone();
			let token_symbol = tranche_input.metadata.token_symbol.clone();

			// The decimals of the tranche token need to match the decimals of the pool
			// currency. Otherwise, we'd always need to convert investments to the decimals
			// of tranche tokens and vice versa
			let decimals = match T::AssetRegistry::metadata(&currency) {
				Some(metadata) => metadata.decimals,
				None => return Err(Error::<T>::MetadataForCurrencyNotFound.into()),
			};

			let metadata = tranche.create_asset_metadata(decimals, token_name, token_symbol);

			T::AssetRegistry::register_asset(Some(tranche.currency.into()), metadata)
				.map_err(|_| Error::<T>::FailedToRegisterTrancheMetadata)?;
		}

		for (fee_bucket, pool_fee) in pool_fees.into_iter() {
			T::PoolFees::add_fee(pool_id, fee_bucket, pool_fee)?;
		}

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

		// TODO: Remove this implicit behaviour. See https://github.com/centrifuge/centrifuge-chain/issues/1171
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

		let now = T::Time::now();

		let update = ScheduledUpdateDetails {
			changes: changes.clone(),
			submitted_at: now,
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
		let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;

		ensure!(
			EpochExecution::<T>::try_get(pool_id).is_err(),
			Error::<T>::InSubmissionPeriod
		);

		let update =
			ScheduledUpdate::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoScheduledUpdate)?;

		let now = T::Time::now();
		ensure!(
			now >= update.submitted_at.ensure_add(T::MinUpdateDelay::get())?,
			Error::<T>::ScheduledTimeHasNotPassed
		);

		ensure!(
			T::UpdateGuard::released(&pool, &update, now),
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

		T::Tokens::transfer(id.into(), source, dest, amount, Preservation::Expendable).map(|_| ())
	}

	fn deposit(
		buyer: &T::AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		let _details = Pool::<T>::get(id.of_pool()).ok_or(Error::<T>::NoSuchPool)?;

		T::Tokens::mint_into(id.into(), buyer, amount).map(|_| ())
	}

	fn withdraw(
		seller: &T::AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		let _details = Pool::<T>::get(id.of_pool()).ok_or(Error::<T>::NoSuchPool)?;

		T::Tokens::burn_from(
			id.into(),
			seller,
			amount,
			Precision::Exact,
			Fortitude::Polite,
		)
		.map(|_| ())
	}
}

impl<T: Config> ChangeGuard for Pallet<T> {
	type Change = T::RuntimeChange;
	type ChangeId = T::Hash;
	type PoolId = T::PoolId;

	fn note(pool_id: Self::PoolId, change: Self::Change) -> Result<Self::ChangeId, DispatchError> {
		// NOTE: Essentially, this key-generation allows to override previously
		//       submitted changes, if they are identical.
		let change_id: Self::ChangeId = T::Hashing::hash(&change.encode());
		let noted_change = NotedPoolChange {
			submitted_time: T::Time::now(),
			change,
		};
		NotedChange::<T>::insert(pool_id, change_id, noted_change.clone());

		Self::deposit_event(Event::ProposedChange {
			pool_id,
			change_id,
			change: noted_change.change,
		});

		Ok(change_id)
	}

	fn released(
		pool_id: Self::PoolId,
		change_id: Self::ChangeId,
	) -> Result<Self::Change, DispatchError> {
		let NotedPoolChange {
			submitted_time,
			change,
		} = NotedChange::<T>::get(pool_id, change_id).ok_or(Error::<T>::ChangeNotFound)?;

		let pool_change: PoolChangeProposal = change.clone().into();
		let pool = Pool::<T>::get(pool_id).ok_or(Error::<T>::NoSuchPool)?;

		// Default requirement for all changes
		let mut allowed = !pool.epoch.is_submission_period();

		for requirement in pool_change.requirements() {
			allowed &= match requirement {
				Requirement::NextEpoch => submitted_time < pool.epoch.last_closed,
				Requirement::DelayTime(secs) => {
					T::Time::now().saturating_sub(submitted_time) >= secs as u64
				}
				Requirement::BlockedByLockedRedemptions => true, // TODO: #1407
			}
		}

		let change = allowed
			.then(|| {
				NotedChange::<T>::remove(pool_id, change_id);
				change
			})
			.ok_or(Error::<T>::ChangeNotReady)?;

		Self::deposit_event(Event::ReleasedChange {
			pool_id,
			change_id,
			change: change.clone(),
		});

		Ok(change)
	}
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks_utils {
	use cfg_traits::{
		benchmarking::{
			FundedPoolBenchmarkHelper, InvestmentIdBenchmarkHelper, PoolBenchmarkHelper,
		},
		investments::Investment,
	};
	use cfg_types::{
		pools::TrancheMetadata,
		tokens::{CurrencyId, CustomMetadata},
	};
	use frame_benchmarking::account;
	use frame_support::traits::Currency;
	use frame_system::RawOrigin;
	use sp_std::vec;

	use super::*;

	const POOL_CURRENCY: CurrencyId = CurrencyId::ForeignAsset(1);
	const FUNDS: u128 = u64::max_value() as u128;

	impl<T: Config<CurrencyId = CurrencyId>> PoolBenchmarkHelper for Pallet<T> {
		type AccountId = T::AccountId;
		type PoolId = T::PoolId;

		fn bench_create_pool(pool_id: T::PoolId, admin: &T::AccountId) {
			if T::AssetRegistry::metadata(&POOL_CURRENCY).is_none() {
				frame_support::assert_ok!(T::AssetRegistry::register_asset(
					Some(POOL_CURRENCY),
					orml_asset_registry::AssetMetadata {
						decimals: 12,
						name: Default::default(),
						symbol: Default::default(),
						existential_deposit: Zero::zero(),
						location: None,
						additional: CustomMetadata {
							pool_currency: true,
							..CustomMetadata::default()
						},
					},
				));
			}

			// Pool creation
			T::Currency::make_free_balance_be(
				admin,
				T::PoolDeposit::get() + T::Currency::minimum_balance(),
			);
			frame_support::assert_ok!(Pallet::<T>::create(
				admin.clone(),
				admin.clone(),
				pool_id,
				vec![
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						},
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: One::one(),
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						},
					},
				],
				POOL_CURRENCY,
				FUNDS.into(),
				// NOTE: Genesis pool fees missing per default, could be added via <T::PoolFees as
				// PoolFeesBenchmarkHelper>::add_pool_fees(..)
				vec![],
			));
		}
	}

	impl<T: Config<CurrencyId = CurrencyId>> FundedPoolBenchmarkHelper for Pallet<T>
	where
		T::Investments: Investment<T::AccountId, InvestmentId = T::TrancheCurrency>,
		<T::Investments as Investment<T::AccountId>>::Amount: From<u128>,
	{
		type AccountId = T::AccountId;
		type Balance = T::Balance;
		type PoolId = T::PoolId;

		fn bench_create_funded_pool(pool_id: T::PoolId, admin: &T::AccountId) {
			Self::bench_create_pool(pool_id, admin);

			// Fund pool account
			const POOL_ACCOUNT_BALANCE: u128 = u64::max_value() as u128;
			let pool_account = PoolLocator { pool_id }.into_account_truncating();
			frame_support::assert_ok!(T::Tokens::mint_into(
				POOL_CURRENCY,
				&pool_account,
				POOL_ACCOUNT_BALANCE.into()
			));

			// Investment in pool
			let investor = account::<T::AccountId>("investor_benchmark_pool", 0, 0);
			Self::bench_investor_setup(
				pool_id,
				investor.clone(),
				T::Currency::minimum_balance() + FUNDS.into(),
			);
			let tranche =
				<Self as InvestmentIdBenchmarkHelper>::bench_default_investment_id(pool_id)
					.of_tranche();
			frame_support::assert_ok!(T::Investments::update_investment(
				&investor,
				T::TrancheCurrency::generate(pool_id, tranche),
				FUNDS.into(),
			));

			// Close epoch
			Pool::<T>::mutate(pool_id, |pool| {
				let pool = pool.as_mut().unwrap();
				pool.parameters.min_epoch_time = 0;
				pool.parameters.max_nav_age = 999_999_999_999;
			});

			frame_support::assert_ok!(Pallet::<T>::close_epoch(
				RawOrigin::Signed(admin.clone()).into(),
				pool_id
			));
		}

		fn bench_investor_setup(pool_id: T::PoolId, account: T::AccountId, balance: T::Balance) {
			T::Tokens::mint_into(POOL_CURRENCY, &account, balance).unwrap();
			T::Currency::make_free_balance_be(&account, balance);

			let tranche =
				<Self as InvestmentIdBenchmarkHelper>::bench_default_investment_id(pool_id)
					.of_tranche();
			frame_support::assert_ok!(T::Permission::add(
				PermissionScope::Pool(pool_id),
				account,
				Role::PoolRole(PoolRole::TrancheInvestor(tranche, u64::MAX)),
			));
		}
	}

	impl<T: Config<CurrencyId = CurrencyId>> InvestmentIdBenchmarkHelper for Pallet<T> {
		type InvestmentId = T::TrancheCurrency;
		type PoolId = T::PoolId;

		fn bench_default_investment_id(pool_id: Self::PoolId) -> T::TrancheCurrency {
			let tranche_id = Pallet::<T>::pool(pool_id)
				.expect("Pool should exist")
				.tranches
				.tranche_id(TrancheLoc::Index(0))
				.expect("Tranche at index 0 should exist");
			T::TrancheCurrency::generate(pool_id, tranche_id)
		}
	}
}
