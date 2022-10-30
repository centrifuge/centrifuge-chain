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

//! Migrations of storage concerned with the pallet PoolsSystem

pub mod altair {
	use cfg_primitives::{Moment, PoolId};
	use cfg_traits::TrancheCurrency as _;
	use cfg_types::{CurrencyId as TCurrencyId, TrancheCurrency};
	use codec::{Decode, Encode};
	#[cfg(feature = "try-runtime")]
	use frame_support::storage::PrefixIterator;
	use frame_support::{
		dispatch::Weight,
		pallet_prelude::ValueQuery,
		storage::types::{StorageDoubleMap, StorageMap},
		traits::StorageInstance,
		Blake2_128Concat,
	};
	use scale_info::TypeInfo;
	use sp_arithmetic::{FixedPointNumber, FixedPointOperand, Perquintill};
	use sp_runtime::{
		traits::{Get, Zero},
		BoundedVec, RuntimeDebug,
	};
	#[cfg(feature = "try-runtime")]
	use sp_std::sync::Arc;
	use sp_std::{marker::PhantomData, vec::Vec};

	use crate::{
		Config, EpochExecutionInfo, EpochExecutionTranche, EpochExecutionTranches, EpochSolution,
		EpochState, One, PoolDepositOf, PoolDetails, PoolParameters, PoolStatus, ReserveDetails,
		ScheduledUpdateDetailsOf, Seniority, Tranche, TrancheSalt, TrancheType, Tranches,
	};

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct OldTranche<Balance, Rate, Weight, CurrencyId> {
		pub(super) tranche_type: TrancheType<Rate>,
		pub(super) seniority: Seniority,
		pub currency: CurrencyId,

		pub(super) outstanding_invest_orders: Balance,
		pub(super) outstanding_redeem_orders: Balance,

		pub(super) debt: Balance,
		pub(super) reserve: Balance,
		pub(super) loss: Balance,
		pub(super) ratio: Perquintill,
		pub(super) last_updated_interest: Moment,
		pub(super) _phantom: PhantomData<Weight>,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct OldTranches<Balance, Rate, Weight, Currency, TrancheId, PoolId> {
		pub tranches: Vec<OldTranche<Balance, Rate, Weight, Currency>>,
		pub ids: Vec<TrancheId>,
		pub salt: TrancheSalt<PoolId>,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct OldPoolDetails<
		CurrencyId,
		EpochId,
		Balance,
		Rate,
		MetaSize,
		Weight,
		TrancheId,
		PoolId,
	> where
		MetaSize: Get<u32> + Copy,
		Rate: FixedPointNumber<Inner = Balance>,
		Balance: FixedPointOperand,
	{
		/// Currency that the pool is denominated in (immutable).
		pub currency: CurrencyId,
		/// List of tranches, ordered junior to senior.
		pub tranches: OldTranches<Balance, Rate, Weight, CurrencyId, TrancheId, PoolId>,
		/// Details about the parameters of the pool.
		pub parameters: PoolParameters,
		/// Metadata that specifies the pool.
		pub metadata: Option<BoundedVec<u8, MetaSize>>,
		/// The status the pool is currently in.
		pub status: PoolStatus,
		/// Details about the epochs of the pool.
		pub epoch: EpochState<EpochId>,
		/// Details about the reserve (unused capital) in the pool.
		pub reserve: ReserveDetails<Balance>,
	}

	/// The old prefix we used when using pallet-pools as
	/// `Pools` in the construct_runtime!-macro
	pub struct PoolPrefix;
	impl StorageInstance for PoolPrefix {
		const STORAGE_PREFIX: &'static str = "Pool";

		fn pallet_prefix() -> &'static str {
			"Pools"
		}
	}
	type Pool<T> = StorageMap<
		PoolPrefix,
		Blake2_128Concat,
		<T as Config>::PoolId,
		OldPoolDetails<
			<T as Config>::CurrencyId,
			<T as Config>::EpochId,
			<T as Config>::Balance,
			<T as Config>::Rate,
			<T as Config>::MaxSizeMetadata,
			<T as Config>::TrancheWeight,
			<T as Config>::TrancheId,
			<T as Config>::PoolId,
		>,
	>;

	#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
	pub struct OldEpochExecutionTranche<Balance, BalanceRatio, Weight> {
		pub(super) supply: Balance,
		pub(super) price: BalanceRatio,
		pub(super) invest: Balance,
		pub(super) redeem: Balance,
		pub(super) min_risk_buffer: Perquintill,
		pub(super) seniority: Seniority,
		pub(super) _phantom: PhantomData<Weight>,
	}

	#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
	pub struct OldEpochExecutionTranches<Balance, BalanceRatio, Weight> {
		pub(super) tranches: Vec<OldEpochExecutionTranche<Balance, BalanceRatio, Weight>>,
	}

	#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct OldEpochExecutionInfo<Balance, BalanceRatio, EpochId, Weight, BlockNumber> {
		epoch: EpochId,
		nav: Balance,
		reserve: Balance,
		max_reserve: Balance,
		tranches: OldEpochExecutionTranches<Balance, BalanceRatio, Weight>,
		best_submission: Option<EpochSolution<Balance>>,
		challenge_period_end: Option<BlockNumber>,
	}

	/// The old prefix we used when using pallet-pools as
	/// `Pools` in the construct_runtime!-macro
	pub struct EpochExecutionPrefix;
	impl StorageInstance for EpochExecutionPrefix {
		const STORAGE_PREFIX: &'static str = "EpochExecution";

		fn pallet_prefix() -> &'static str {
			"Pools"
		}
	}
	type EpochExecution<T> = StorageMap<
		EpochExecutionPrefix,
		Blake2_128Concat,
		<T as Config>::PoolId,
		OldEpochExecutionInfo<
			<T as Config>::Balance,
			<T as Config>::Rate,
			<T as Config>::EpochId,
			<T as Config>::TrancheWeight,
			<T as frame_system::Config>::BlockNumber,
		>,
	>;

	#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
	pub struct OldEpochDetails<BalanceRatio> {
		pub invest_fulfillment: Perquintill,
		pub redeem_fulfillment: Perquintill,
		pub token_price: BalanceRatio,
	}

	pub struct EpochPrefix;
	impl StorageInstance for EpochPrefix {
		const STORAGE_PREFIX: &'static str = "Epoch";

		fn pallet_prefix() -> &'static str {
			"Pools"
		}
	}
	type Epoch<T> = StorageDoubleMap<
		EpochPrefix,
		Blake2_128Concat,
		<T as Config>::TrancheId,
		Blake2_128Concat,
		<T as Config>::EpochId,
		OldEpochDetails<<T as Config>::Rate>,
	>;

	pub struct OrderPrefix;
	impl StorageInstance for OrderPrefix {
		const STORAGE_PREFIX: &'static str = "Order";

		fn pallet_prefix() -> &'static str {
			"Pools"
		}
	}
	pub type Order<T> = StorageDoubleMap<
		OrderPrefix,
		Blake2_128Concat,
		<T as Config>::TrancheId,
		Blake2_128Concat,
		<T as frame_system::Config>::AccountId,
		UserOrder<<T as Config>::Balance, <T as Config>::EpochId>,
	>;

	/// Per-tranche and per-user order details.
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
	pub struct UserOrder<Balance, EpochId> {
		pub invest: Balance,
		pub redeem: Balance,
		pub epoch: EpochId,
	}

	impl<Balance, EpochId> Default for UserOrder<Balance, EpochId>
	where
		Balance: Zero,
		EpochId: One,
	{
		fn default() -> Self {
			UserOrder {
				invest: Zero::zero(),
				redeem: Zero::zero(),
				epoch: One::one(),
			}
		}
	}

	pub struct ScheduledUpdatePrefix;
	impl StorageInstance for ScheduledUpdatePrefix {
		const STORAGE_PREFIX: &'static str = "ScheduledUpdate";

		fn pallet_prefix() -> &'static str {
			"Pools"
		}
	}
	pub type ScheduledUpdate<T> = StorageMap<
		ScheduledUpdatePrefix,
		Blake2_128Concat,
		<T as Config>::PoolId,
		ScheduledUpdateDetailsOf<T>,
	>;

	pub struct AccountDepositPrefix;
	impl StorageInstance for AccountDepositPrefix {
		const STORAGE_PREFIX: &'static str = "AccountDeposit";

		fn pallet_prefix() -> &'static str {
			"Pools"
		}
	}
	pub type AccountDeposit<T> = StorageMap<
		AccountDepositPrefix,
		Blake2_128Concat,
		<T as frame_system::Config>::AccountId,
		<T as Config>::Balance,
		ValueQuery,
	>;

	pub struct PoolDepositPrefix;
	impl StorageInstance for PoolDepositPrefix {
		const STORAGE_PREFIX: &'static str = "PoolDeposit";

		fn pallet_prefix() -> &'static str {
			"Pools"
		}
	}
	pub type PoolDeposit<T> =
		StorageMap<PoolDepositPrefix, Blake2_128Concat, <T as Config>::PoolId, PoolDepositOf<T>>;

	fn migrate_pool_deposit<T: Config>() -> Weight {
		let mut weight = 0u64;

		// Migrate PoolDeposit
		let mut loops = 0u64;
		PoolDeposit::<T>::iter().for_each(|(pool_id, deposit)| {
			loops += 1;
			crate::PoolDeposit::<T>::insert(pool_id, deposit);
		});

		weight += loops * (T::DbWeight::get().write + T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	fn migrate_account_deposit<T: Config>() -> Weight {
		let mut weight = 0u64;

		// Migrate AccountDeposit
		let mut loops = 0u64;
		AccountDeposit::<T>::iter().for_each(|(pool_id, deposit)| {
			loops += 1;
			crate::AccountDeposit::<T>::insert(pool_id, deposit);
		});

		weight += loops * (T::DbWeight::get().write + T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	fn migrate_scheduled_update<T: Config>() -> Weight {
		let mut weight = 0u64;

		// Migrate ScheduledUpdate
		let mut loops = 0u64;
		ScheduledUpdate::<T>::iter().for_each(|(pool_id, scheduled_update)| {
			loops += 1;
			crate::ScheduledUpdate::<T>::insert(pool_id, scheduled_update);
		});

		weight += loops * (T::DbWeight::get().write + T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	fn migrate_tranches<T: Config>() -> Weight
	where
		T::TrancheId: From<[u8; 16]> + Into<[u8; 16]>,
		T::PoolId: From<PoolId> + Into<PoolId>,
		T::TrancheCurrency: From<TrancheCurrency>,
		T::CurrencyId: Into<TCurrencyId>,
	{
		let mut weight = 0u64;

		// Migrate PoolDetails
		let mut loops = 0u64;
		Pool::<T>::iter().for_each(|(pool_id, old_details)| {
			loops += 1;

			let OldPoolDetails {
				currency,
				tranches,
				parameters,
				metadata,
				status,
				epoch,
				reserve,
			} = old_details;

			let ids = tranches.ids.clone();
			let salt = tranches.salt.clone();
			let new_tranches = tranches
				.tranches
				.into_iter()
				.map(|old_tranche| {
					let tranche_id = match old_tranche.currency.into() {
						TCurrencyId::Tranche(_pool_id, tranche_id) => tranche_id,
						_ => unreachable!("All tranches have tranche as currency. Qed."),
					};

					Tranche {
						tranche_type: old_tranche.tranche_type,
						seniority: old_tranche.seniority,
						currency: TrancheCurrency::generate(pool_id.into(), tranche_id.into())
							.into(),
						debt: old_tranche.debt,
						reserve: old_tranche.reserve,
						loss: old_tranche.loss,
						ratio: old_tranche.ratio,
						last_updated_interest: old_tranche.last_updated_interest,
						_phantom: Default::default(),
					}
				})
				.collect::<Vec<_>>();

			crate::Pool::<T>::insert(
				pool_id,
				PoolDetails {
					currency,
					tranches: Tranches {
						tranches: new_tranches,
						ids,
						salt,
					},
					parameters,
					metadata,
					status,
					epoch,
					reserve,
				},
			);
		});
		weight += loops * (T::DbWeight::get().write + T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	/// MUST RUN BEFORE `migrate_tranches`
	fn migrate_epoch_tranches<T: Config>() -> Weight
	where
		T::TrancheId: From<[u8; 16]> + Into<[u8; 16]>,
		T::PoolId: From<PoolId> + Into<PoolId>,
		T::TrancheCurrency: From<TrancheCurrency>,
	{
		let mut weight = 0;

		// Migrate EpochExecutionInfo
		let mut loops = 0u64;
		EpochExecution::<T>::iter().for_each(|(pool_id, info)| {
			loops += 1;

			let OldEpochExecutionInfo {
				epoch,
				nav,
				reserve,
				max_reserve,
				tranches: OldEpochExecutionTranches {
					tranches: old_tranches,
				},
				best_submission,
				challenge_period_end,
			} = info;

			let details = Pool::<T>::get(pool_id)
				.expect("If EpochTranches exists then also pool exists. Qed.");

			let new_tranches = old_tranches
				.into_iter()
				.zip(details.tranches.ids)
				.map(|(old_tranche, tranche_id)| EpochExecutionTranche::<
					T::Balance,
					T::Rate,
					T::TrancheWeight,
					T::TrancheCurrency,
				> {
					currency: TrancheCurrency::generate(pool_id.into(), tranche_id.into()).into(),
					supply: old_tranche.supply,
					price: old_tranche.price,
					invest: old_tranche.invest,
					redeem: old_tranche.redeem,
					min_risk_buffer: old_tranche.min_risk_buffer,
					seniority: old_tranche.seniority,
					_phantom: Default::default(),
				})
				.collect::<Vec<_>>();

			crate::EpochExecution::<T>::insert(
				pool_id,
				EpochExecutionInfo {
					epoch,
					nav,
					reserve,
					max_reserve,
					tranches: EpochExecutionTranches::new(new_tranches),
					best_submission,
					challenge_period_end,
				},
			);
		});
		weight += loops * (T::DbWeight::get().write + 2 * T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	/// This function MUST be called AFTER `migrate_all_storage_under_old_prefix`
	fn remove_not_needed_storage<T: Config>() -> Weight {
		let mut weight = 0u64;

		// Remove EpochDetails
		let loops = Epoch::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		// Remove EpochExecutionInfo
		let loops = EpochExecution::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		// Remove Pool
		let loops = Pool::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		// Remove Order
		let loops = Order::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		// Remove ScheduledUpdate
		let loops = ScheduledUpdate::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		// Remove AccountDeposit
		let loops = AccountDeposit::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		// RemoveP PoolDeposit
		let loops = PoolDeposit::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	pub fn migrate_all_storage_under_old_prefix_and_remove_old_one<T: Config>() -> Weight
	where
		T::TrancheId: From<[u8; 16]> + Into<[u8; 16]>,
		T::PoolId: From<PoolId> + Into<PoolId>,
		T::TrancheCurrency: From<TrancheCurrency>,
		T::CurrencyId: Into<TCurrencyId>,
	{
		let mut weight = Weight::from_ref_time(0u64);

		weight += migrate_epoch_tranches::<T>();
		weight += migrate_tranches::<T>();
		weight += migrate_scheduled_update::<T>();
		weight += migrate_account_deposit::<T>();
		weight += migrate_pool_deposit::<T>();
		weight += remove_not_needed_storage::<T>();

		weight
	}

	#[cfg(feature = "try-runtime")]
	lazy_static::lazy_static! {
		pub static ref NUM_POOL_DETAILS: Arc<u32> = Arc::new(0);
		pub static ref NUM_EPOCH_EXECUTION_INFOS:  Arc<u32> = Arc::new(0);
		pub static ref NUM_SCHEDULED_UPDATES:  Arc<u32> = Arc::new(0);
		pub static ref NUM_POOL_DEPOSITS:  Arc<u32> = Arc::new(0);
		pub static ref NUM_ACCOUNT_DEPOSITS:  Arc<u32> = Arc::new(0);
	}

	#[cfg(feature = "try-runtime")]
	pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
		reset_counter(NUM_POOL_DETAILS.as_ref());
		reset_counter(NUM_EPOCH_EXECUTION_INFOS.as_ref());
		reset_counter(NUM_SCHEDULED_UPDATES.as_ref());
		reset_counter(NUM_POOL_DEPOSITS.as_ref());
		reset_counter(NUM_ACCOUNT_DEPOSITS.as_ref());

		count_items(Pool::<T>::iter_values(), NUM_POOL_DETAILS.as_ref());
		count_items(
			EpochExecution::<T>::iter_values(),
			NUM_EPOCH_EXECUTION_INFOS.as_ref(),
		);
		count_items(
			ScheduledUpdate::<T>::iter_values(),
			NUM_SCHEDULED_UPDATES.as_ref(),
		);
		count_items(PoolDeposit::<T>::iter_values(), NUM_POOL_DEPOSITS.as_ref());
		count_items(
			AccountDeposit::<T>::iter_values(),
			NUM_ACCOUNT_DEPOSITS.as_ref(),
		);

		Ok(())
	}

	#[cfg(feature = "try-runtime")]
	fn reset_counter(counter_ref: &u32) {
		let counter = unsafe { &mut *(counter_ref as *const u32 as *mut u32) };
		*counter = 0;
	}

	#[cfg(feature = "try-runtime")]
	fn count_items<T>(iter: PrefixIterator<T>, counter_ref: &u32) {
		let counter = unsafe { &mut *(counter_ref as *const u32 as *mut u32) };
		iter.for_each(|_| {
			*counter += 1;
		});
	}

	#[cfg(feature = "try-runtime")]
	fn assert_no_items<T>(iter: PrefixIterator<T>) {
		let mut counter = 0u32;
		iter.for_each(|_| {
			counter += 1;
		});

		assert_eq!(counter, 0);
	}

	#[cfg(feature = "try-runtime")]
	fn assert_correct_amount<T>(iter: PrefixIterator<T>, amount: &u32) {
		let mut counter = 0u32;
		iter.for_each(|_| {
			counter += 1;
		});

		assert_eq!(counter, *amount);
	}

	#[cfg(feature = "try-runtime")]
	pub fn post_migrate<T: Config>() -> Result<(), &'static str> {
		assert_correct_amount(crate::Pool::<T>::iter_values(), NUM_POOL_DETAILS.as_ref());
		assert_correct_amount(
			crate::EpochExecution::<T>::iter_values(),
			NUM_EPOCH_EXECUTION_INFOS.as_ref(),
		);
		assert_correct_amount(
			crate::ScheduledUpdate::<T>::iter_values(),
			NUM_SCHEDULED_UPDATES.as_ref(),
		);
		assert_correct_amount(
			crate::PoolDeposit::<T>::iter_values(),
			NUM_POOL_DEPOSITS.as_ref(),
		);
		assert_correct_amount(
			crate::AccountDeposit::<T>::iter_values(),
			NUM_ACCOUNT_DEPOSITS.as_ref(),
		);

		assert_no_items(EpochExecution::<T>::iter_values());
		assert_no_items(Epoch::<T>::iter_values());
		assert_no_items(Pool::<T>::iter_values());
		assert_no_items(Order::<T>::iter_values());
		assert_no_items(ScheduledUpdate::<T>::iter_values());
		assert_no_items(AccountDeposit::<T>::iter_values());
		assert_no_items(PoolDeposit::<T>::iter_values());

		Ok(())
	}

	#[cfg(test)]
	#[cfg(feature = "try-runtime")]
	mod test {
		use cfg_primitives::TrancheId;
		use cfg_types::Rate;
		use frame_support::assert_ok;
		use orml_traits::Change;

		use super::*;
		use crate::{
			mock::{new_test_ext, MockAccountId, Test},
			PoolChanges, PoolDepositInfo, ScheduledUpdateDetails,
		};

		#[test]
		fn all_migrations_are_correct() {
			new_test_ext().execute_with(|| {
				const POOL_ID: PoolId = 0;
				const POOL_ID_2: PoolId = 0;
				const TRANCHE_ID_JUNIOR: TrancheId = [0u8; 16];
				const TRANCHE_ID_SENIOR: TrancheId = [1u8; 16];

				const ACCOUNT_JUNIOR_INVESTOR: MockAccountId = 0;
				const ACCOUNT_SENIOR_INVESTOR: MockAccountId = 1;

				// Setup storage correctly first from old version
				// We need one pool-details
				Pool::<Test>::insert(
					POOL_ID,
					OldPoolDetails {
						currency: TCurrencyId::AUSD,
						tranches: OldTranches {
							tranches: vec![
								OldTranche {
									tranche_type: TrancheType::Residual,
									seniority: 0,
									currency: TCurrencyId::Tranche(POOL_ID, TRANCHE_ID_JUNIOR),
									outstanding_invest_orders: 0,
									outstanding_redeem_orders: 0,
									debt: 0,
									reserve: 0,
									loss: 0,
									ratio: Default::default(),
									last_updated_interest: 0,
									_phantom: Default::default(),
								},
								OldTranche {
									tranche_type: TrancheType::NonResidual {
										interest_rate_per_sec: Rate::one(),
										min_risk_buffer: Perquintill::zero(),
									},
									seniority: 0,
									currency: TCurrencyId::Tranche(POOL_ID, TRANCHE_ID_SENIOR),
									outstanding_invest_orders: 0,
									outstanding_redeem_orders: 0,
									debt: 0,
									reserve: 0,
									loss: 0,
									ratio: Default::default(),
									last_updated_interest: 0,
									_phantom: Default::default(),
								},
							],
							ids: vec![TRANCHE_ID_JUNIOR, TRANCHE_ID_SENIOR],
							salt: (POOL_ID, 2),
						},

						parameters: PoolParameters {
							min_epoch_time: 0,
							max_nav_age: 0,
						},
						metadata: None,
						status: PoolStatus::Open,
						epoch: EpochState {
							current: 0,
							last_closed: 0,
							last_executed: 0,
						},
						reserve: ReserveDetails {
							max: 0,
							total: 0,
							available: 0,
						},
					},
				);
				// We need one epochExecution Info
				EpochExecution::<Test>::insert(
					POOL_ID,
					OldEpochExecutionInfo {
						epoch: 0,
						nav: 0,
						reserve: 0,
						max_reserve: 0,
						tranches: OldEpochExecutionTranches {
							tranches: vec![
								OldEpochExecutionTranche {
									supply: 0,
									price: Rate::one(),
									invest: 0,
									redeem: 0,
									min_risk_buffer: Default::default(),
									seniority: 0,
									_phantom: Default::default(),
								},
								OldEpochExecutionTranche {
									supply: 0,
									price: Rate::one(),
									invest: 0,
									redeem: 0,
									min_risk_buffer: Default::default(),
									seniority: 0,
									_phantom: Default::default(),
								},
							],
						},
						best_submission: None,
						challenge_period_end: None,
					},
				);
				// We need two Orders with two different keys
				Order::<Test>::insert(
					TRANCHE_ID_JUNIOR,
					ACCOUNT_JUNIOR_INVESTOR,
					UserOrder::default(),
				);
				Order::<Test>::insert(
					TRANCHE_ID_SENIOR,
					ACCOUNT_SENIOR_INVESTOR,
					UserOrder::default(),
				);

				// We need to Epoch with different keys
				Epoch::<Test>::insert(
					TRANCHE_ID_JUNIOR,
					0,
					OldEpochDetails {
						invest_fulfillment: Default::default(),
						redeem_fulfillment: Default::default(),
						token_price: Rate::one(),
					},
				);
				Epoch::<Test>::insert(
					TRANCHE_ID_SENIOR,
					1,
					OldEpochDetails {
						invest_fulfillment: Default::default(),
						redeem_fulfillment: Default::default(),
						token_price: Rate::one(),
					},
				);
				PoolDeposit::<Test>::insert(
					POOL_ID,
					PoolDepositInfo {
						depositor: Default::default(),
						deposit: Default::default(),
					},
				);
				PoolDeposit::<Test>::insert(
					POOL_ID_2,
					PoolDepositInfo {
						depositor: Default::default(),
						deposit: Default::default(),
					},
				);
				AccountDeposit::<Test>::insert(POOL_ID, 0);
				AccountDeposit::<Test>::insert(POOL_ID_2, 0);
				ScheduledUpdate::<Test>::insert(
					POOL_ID,
					ScheduledUpdateDetails {
						changes: PoolChanges {
							tranches: Change::NoChange,
							tranche_metadata: Change::NoChange,
							min_epoch_time: Change::NoChange,
							max_nav_age: Change::NoChange,
						},
						scheduled_time: 0,
					},
				);
				ScheduledUpdate::<Test>::insert(
					POOL_ID_2,
					ScheduledUpdateDetails {
						changes: PoolChanges {
							tranches: Change::NoChange,
							tranche_metadata: Change::NoChange,
							min_epoch_time: Change::NoChange,
							max_nav_age: Change::NoChange,
						},
						scheduled_time: 0,
					},
				);
				ScheduledUpdate::<Test>::insert(
					POOL_ID_2,
					ScheduledUpdateDetails {
						changes: PoolChanges {
							tranches: Change::NoChange,
							tranche_metadata: Change::NoChange,
							min_epoch_time: Change::NoChange,
							max_nav_age: Change::NoChange,
						},
						scheduled_time: 0,
					},
				);

				crate::AccountDeposit::<Test>::insert(POOL_ID_2, 0);
				crate::AccountDeposit::<Test>::insert(POOL_ID, 0);

				assert_ok!(pre_migrate::<Test>());

				// Run migrations
				let _ = migrate_all_storage_under_old_prefix_and_remove_old_one::<Test>();

				// Assert post migration
				assert_ok!(post_migrate::<Test>());
			})
		}
	}
}
