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

//! Migrations of storage concerned with the pallet Pools

pub mod altair {
	use cfg_primitives::{Moment, PoolId};
	use cfg_traits::TrancheCurrency as _;
	use cfg_types::{CurrencyId as TCurrencyId, TrancheCurrency};
	use codec::{Decode, Encode};
	use frame_support::{dispatch::Weight, Blake2_128Concat};
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
		EpochState, One, Pallet, PoolDetails, PoolParameters, PoolStatus, ReserveDetails,
		Seniority, Tranche, TrancheSalt, TrancheType, Tranches,
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

	#[frame_support::storage_alias]
	type Pool<T: Config> = StorageMap<
		Pallet<T>,
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

	#[frame_support::storage_alias]
	type EpochExecution<T: Config> = StorageMap<
		Pallet<T>,
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

	#[frame_support::storage_alias]
	type Epoch<T: Config> = StorageDoubleMap<
		Pallet<T>,
		Blake2_128Concat,
		<T as Config>::TrancheId,
		Blake2_128Concat,
		<T as Config>::EpochId,
		OldEpochDetails<<T as Config>::Rate>,
	>;

	#[frame_support::storage_alias]
	pub type Order<T: Config> = StorageDoubleMap<
		Pallet<T>,
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

	pub fn migrate_tranches<T: Config>() -> Weight
	where
		T::TrancheId: From<[u8; 16]> + Into<[u8; 16]>,
		T::PoolId: From<PoolId> + Into<PoolId>,
		T::TrancheCurrency: From<TrancheCurrency>,
		T::CurrencyId: Into<TCurrencyId>,
	{
		let mut weight = 0u64;

		// Migrate PoolDetails
		let mut loops = 0u64;
		crate::Pool::<T>::translate::<
			OldPoolDetails<
				T::CurrencyId,
				T::EpochId,
				T::Balance,
				T::Rate,
				T::MaxSizeMetadata,
				T::TrancheWeight,
				T::TrancheId,
				T::PoolId,
			>,
			_,
		>(|pool_id, old_details| {
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

			Some(PoolDetails {
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
			})
		});
		weight += loops * (T::DbWeight::get().write + T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	/// MUST RUN BEFORE `migrate_tranches`
	pub fn migrate_epoch_tranches<T: Config>() -> Weight
	where
		T::TrancheId: From<[u8; 16]> + Into<[u8; 16]>,
		T::PoolId: From<PoolId> + Into<PoolId>,
		T::TrancheCurrency: From<TrancheCurrency>,
	{
		let mut weight = 0;

		// Migrate EpochExecutionInfo
		let mut loops = 0u64;
		crate::EpochExecution::<T>::translate::<
			OldEpochExecutionInfo<
				T::Balance,
				T::Rate,
				T::EpochId,
				T::TrancheWeight,
				T::BlockNumber,
			>,
			_,
		>(|pool_id, info| {
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
				.map(|(old_tranche, tranche_id)| EpochExecutionTranche {
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

			Some(EpochExecutionInfo {
				epoch,
				nav,
				reserve,
				max_reserve,
				tranches: EpochExecutionTranches::new(new_tranches),
				best_submission,
				challenge_period_end,
			})
		});
		weight += loops * (T::DbWeight::get().write + 2 * T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	/// This function MUST be called AFTER `migrate_orders`
	pub fn remove_not_needed_storage<T: Config>() -> Weight {
		let mut weight = 0u64;

		// Remove EpochDetails
		let loops = Epoch::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		// Remove Order
		let loops = Order::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		Weight::from_ref_time(weight)
	}

	#[cfg(feature = "try-runtime")]
	lazy_static::lazy_static! {
		pub static ref NUM_POOL_DETAILS: Arc<u32> = Arc::new(0);
		pub static ref NUM_EPOCH_EXECUTION_INFOS:  Arc<u32> = Arc::new(0);
	}

	#[cfg(feature = "try-runtime")]
	pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
		unsafe {
			let mut_ref = &mut *(NUM_POOL_DETAILS.as_ref() as *const u32 as *mut u32);
			*mut_ref = 0;
		}
		unsafe {
			let mut_ref = &mut *(NUM_EPOCH_EXECUTION_INFOS.as_ref() as *const u32 as *mut u32);
			*mut_ref = 0;
		}

		Pool::<T>::iter_values()
			.map(|_| unsafe {
				let mut_ref = &mut *(NUM_POOL_DETAILS.as_ref() as *const u32 as *mut u32);
				*mut_ref = *mut_ref + 1;
			})
			.for_each(|_| {});

		EpochExecution::<T>::iter_values()
			.map(|_| unsafe {
				let mut_ref = &mut *(NUM_EPOCH_EXECUTION_INFOS.as_ref() as *const u32 as *mut u32);
				*mut_ref = *mut_ref + 1;
			})
			.for_each(|_| {});

		Ok(())
	}

	#[cfg(feature = "try-runtime")]
	pub fn post_migrate<T: Config>() -> Result<(), &'static str> {
		let mut count_pool_details = 0u32;
		let mut count_epoch_execution_infos = 0u32;

		crate::Pool::<T>::iter_values()
			.map(|_| count_pool_details += 1)
			.for_each(|_| {});

		crate::EpochExecution::<T>::iter_values()
			.map(|_| count_epoch_execution_infos += 1)
			.for_each(|_| {});

		assert_eq!(count_pool_details, *NUM_POOL_DETAILS.as_ref());
		assert_eq!(
			count_epoch_execution_infos,
			*NUM_EPOCH_EXECUTION_INFOS.as_ref()
		);

		Ok(())
	}

	#[cfg(test)]
	#[cfg(feature = "try-runtime")]
	mod test {
		use cfg_primitives::TrancheId;
		use cfg_types::Rate;
		use frame_support::assert_ok;

		use super::*;
		use crate::mock::{new_test_ext, MockAccountId, Test};

		#[test]
		fn all_three_migrations_are_correct() {
			new_test_ext().execute_with(|| {
				const POOL_ID: PoolId = 0;
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

				assert_ok!(pre_migrate::<Test>());

				// Run migrations
				let _ = migrate_epoch_tranches::<Test>();
				let _ = migrate_tranches::<Test>();
				let _ = remove_not_needed_storage::<Test>();

				// Assert post migration
				assert_ok!(post_migrate::<Test>());
			})
		}
	}
}
