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
	use cfg_traits::TrancheCurrency as _;
	use cfg_types::{CurrencyId, TrancheCurrency};

	use crate::*;

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
		pub tranches: Vec<Tranche<Balance, Rate, Weight, Currency>>,
		ids: Vec<TrancheId>,
		salt: TrancheSalt<PoolId>,
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
		pub tranches: Tranches<Balance, Rate, Weight, CurrencyId, TrancheId, PoolId>,
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
	type OldPools<T: Config> = StorageMap<
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
	type OldEpochExecution<T: Config> = StorageMap<
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
	type ToBeClearedEpoch<T: Config> = StorageDoubleMap<
		Pallet<T>,
		Blake2_128Concat,
		<T as Config>::TrancheId,
		Blake2_128Concat,
		<T as Config>::EpochId,
		OldEpochDetails<<T as Config>::Rate>,
	>;

	#[frame_support::storage_alias]
	pub type ToBeClearedOrder<T: Config> = StorageDoubleMap<
		Pallet<T>,
		Blake2_128Concat,
		<T as Config>::TrancheId,
		Blake2_128Concat,
		<T as frame_system::Config>::AccountId,
		UserOrder<<T as Config>::Balance, <T as Config>::EpochId>,
	>;

	pub fn migrate_tranches<T: Config>() -> Weight
	where
		T::TrancheId: From<[u8; 16]> + Into<[u8; 16]>,
		T::PoolId: From<PoolId> + Into<PoolId>,
		T::TrancheCurrency: From<TrancheCurrency>,
		T::CurrencyId: Into<CurrencyId>,
	{
		let mut weight = 0u64;

		// Migrate PoolDetails
		let mut loops = 0u64;
		Pool::<T>::translate::<
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
						CurrencyId::Tranche(_pool_id, tranche_id) => tranche_id,
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

		weight
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
		EpochExecution::<T>::translate::<
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

			let details = OldPools::<T>::get(pool_id)
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

		weight
	}

	pub fn remove_not_needed_storage<T: Config>() -> Weight {
		let mut weight = 0u64;

		// Remove EpochDetails
		let loops = ToBeClearedEpoch::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		// Remove Order
		let loops = ToBeClearedOrder::<T>::clear(u32::MAX, None).loops;
		weight += loops as u64 * (T::DbWeight::get().write + T::DbWeight::get().read);

		weight
	}

	use cfg_primitives::PoolId;
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure; // Not in prelude for try-runtime

	#[cfg(feature = "try-runtime")]
	pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
		Ok(())
	}

	#[cfg(feature = "try-runtime")]
	pub fn post_migrate<T: Config>() -> Result<(), &'static str> {
		Ok(())
	}

	#[cfg(test)]
	#[cfg(feature = "try-runtime")]
	mod test {
		use frame_support::assert_ok;

		use super::*;
		use crate::{
			mock::{new_test_ext, Origin, Test},
			{self as pallet_anchors},
		};
	}
}
