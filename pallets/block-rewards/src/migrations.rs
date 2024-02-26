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

use cfg_traits::TimeAsSecs;
use frame_support::{
	dispatch::GetStorageVersion,
	inherent::Vec,
	pallet_prelude::{StorageVersion, Weight},
	traits::{Get, OnRuntimeUpgrade},
};
#[cfg(feature = "try-runtime")]
use num_traits::Zero;
use parity_scale_codec::{Decode, Encode};
use sp_runtime::FixedPointNumber;
#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;
use sp_std::marker::PhantomData;

use crate::{pallet, Config, Pallet, SessionData};

fn inflation_rate<T: Config>(percent: u32) -> T::Rate {
	T::Rate::saturating_from_rational(percent, 100)
}

pub mod init {
	#[cfg(feature = "try-runtime")]
	use cfg_traits::rewards::AccountRewards;
	use cfg_traits::rewards::CurrencyGroupChange;
	use sp_runtime::{BoundedVec, SaturatedConversion};

	use super::*;

	const LOG_PREFIX: &str = "InitBlockRewards";
	pub struct InitBlockRewards<T, CollatorReward, AnnualTreasuryInflationPercent>(
		PhantomData<(T, CollatorReward, AnnualTreasuryInflationPercent)>,
	);

	fn get_collators<T: pallet_collator_selection::Config>() -> Vec<T::AccountId> {
		let candidates = BoundedVec::<
			T::AccountId,
			<T as pallet_collator_selection::Config>::MaxCandidates,
		>::truncate_from(
			pallet_collator_selection::Pallet::<T>::candidates()
				.into_iter()
				.map(|c| c.who)
				.collect(),
		);
		pallet_collator_selection::Pallet::<T>::assemble_collators(candidates)
	}

	impl<T, CollatorReward, AnnualTreasuryInflationPercent> OnRuntimeUpgrade
		for InitBlockRewards<T, CollatorReward, AnnualTreasuryInflationPercent>
	where
		T: frame_system::Config + Config<Balance = u128> + pallet_collator_selection::Config,
		<T as Config>::Balance: From<u128>,
		CollatorReward: Get<<T as Config>::Balance>,
		AnnualTreasuryInflationPercent: Get<u32>,
	{
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
			assert_eq!(
				Pallet::<T>::on_chain_storage_version(),
				StorageVersion::new(0),
				"On-chain storage version should be 0 (default)"
			);
			let collators = get_collators::<T>();
			assert!(!collators.is_empty());

			assert!(!CollatorReward::get().is_zero());

			log::info!("{LOG_PREFIX} Pre migration checks successful");

			Ok(collators.encode())
		}

		// Weight: 2 + collator_count reads and writes
		fn on_runtime_upgrade() -> Weight {
			if Pallet::<T>::on_chain_storage_version() == StorageVersion::new(0) {
				log::info!("{LOG_PREFIX} Initiating migration");
				let mut weight: Weight = Weight::zero();

				let collators = get_collators::<T>();
				weight.saturating_accrue(T::DbWeight::get().reads(2));

				<T as Config>::Rewards::attach_currency(
					<T as Config>::StakeCurrencyId::get(),
					<T as Config>::StakeGroupId::get(),
				)
				.map_err(|e| log::error!("Failed to attach currency to collator group: {:?}", e))
				.ok();

				pallet::ActiveSessionData::<T>::set(SessionData::<T> {
					collator_count: collators.len().saturated_into(),
					collator_reward: CollatorReward::get(),
					treasury_inflation_rate: inflation_rate::<T>(
						AnnualTreasuryInflationPercent::get(),
					),
					last_update: T::Time::now(),
				});
				weight.saturating_accrue(T::DbWeight::get().writes(1));

				for collator in collators.iter() {
					// NOTE: Benching not required as num of collators <= 10.
					Pallet::<T>::do_init_collator(collator)
						.map_err(|e| {
							log::error!("Failed to init genesis collators for rewards: {:?}", e);
						})
						.ok();
					weight.saturating_accrue(T::DbWeight::get().reads_writes(6, 6));
				}
				Pallet::<T>::current_storage_version().put::<Pallet<T>>();
				weight.saturating_add(T::DbWeight::get().writes(1))
			} else {
				// wrong storage version
				log::info!(
					"{LOG_PREFIX} Migration did not execute. This probably should be removed"
				);
				T::DbWeight::get().reads_writes(1, 0)
			}
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), TryRuntimeError> {
			assert_eq!(
				Pallet::<T>::on_chain_storage_version(),
				Pallet::<T>::current_storage_version(),
				"On-chain storage version should be updated"
			);
			let collators: Vec<T::AccountId> = Decode::decode(&mut pre_state.as_slice())
				.expect("pre_upgrade provides a valid state; qed");

			assert_eq!(
				Pallet::<T>::active_session_data(),
				SessionData::<T> {
					collator_count: collators.len().saturated_into(),
					collator_reward: CollatorReward::get(),
					treasury_inflation_rate: inflation_rate::<T>(
						AnnualTreasuryInflationPercent::get()
					),
					last_update: T::Time::now(),
				}
			);

			for collator in collators.iter() {
				assert!(!<T as Config>::Rewards::account_stake(
					<T as Config>::StakeCurrencyId::get(),
					collator,
				)
				.is_zero())
			}

			log::info!("{LOG_PREFIX} Post migration checks successful");

			Ok(())
		}
	}
}

pub mod v2 {
	use frame_support::{
		pallet_prelude::ValueQuery, storage_alias, DefaultNoBound, RuntimeDebugNoBound,
	};
	use parity_scale_codec::MaxEncodedLen;
	use scale_info::TypeInfo;

	use super::*;
	use crate::{CollatorChanges, SessionChanges};

	const LOG_PREFIX: &str = "RelativeTreasuryInflation";

	#[derive(
		Encode, Decode, TypeInfo, DefaultNoBound, MaxEncodedLen, PartialEq, Eq, RuntimeDebugNoBound,
	)]
	#[scale_info(skip_type_params(T))]
	struct OldSessionData<T: Config> {
		pub collator_reward: T::Balance,
		pub total_reward: T::Balance,
		pub collator_count: u32,
	}

	#[derive(
		PartialEq,
		Clone,
		DefaultNoBound,
		Encode,
		Decode,
		TypeInfo,
		MaxEncodedLen,
		RuntimeDebugNoBound,
	)]
	#[scale_info(skip_type_params(T))]
	struct OldSessionChanges<T: Config> {
		pub collators: CollatorChanges<T>,
		pub collator_count: Option<u32>,
		pub collator_reward: Option<T::Balance>,
		pub total_reward: Option<T::Balance>,
	}

	#[storage_alias]
	type ActiveSessionData<T: Config> = StorageValue<Pallet<T>, OldSessionData<T>, ValueQuery>;
	#[storage_alias]
	type NextSessionChanges<T: Config> = StorageValue<Pallet<T>, OldSessionChanges<T>, ValueQuery>;

	pub struct RelativeTreasuryInflationMigration<T, InflationRate>(
		PhantomData<(T, InflationRate)>,
	);

	impl<T, InflationPercentage> OnRuntimeUpgrade
		for RelativeTreasuryInflationMigration<T, InflationPercentage>
	where
		T: Config,
		InflationPercentage: Get<u32>,
	{
		fn on_runtime_upgrade() -> Weight {
			if Pallet::<T>::on_chain_storage_version() == StorageVersion::new(1) {
				let active = ActiveSessionData::<T>::take();
				let next = NextSessionChanges::<T>::take();

				pallet::ActiveSessionData::<T>::put(SessionData {
					collator_reward: active.collator_reward,
					collator_count: active.collator_count,
					treasury_inflation_rate: inflation_rate::<T>(InflationPercentage::get()),
					last_update: T::Time::now(),
				});
				log::info!("{LOG_PREFIX} Translated ActiveSessionData");

				pallet::NextSessionChanges::<T>::put(SessionChanges {
					collators: next.collators,
					collator_count: next.collator_count,
					collator_reward: next.collator_reward,
					treasury_inflation_rate: Some(inflation_rate::<T>(InflationPercentage::get())),
					last_update: T::Time::now(),
				});
				log::info!("{LOG_PREFIX} Translated NextSessionChanges");
				Pallet::<T>::current_storage_version().put::<Pallet<T>>();

				T::DbWeight::get().reads_writes(1, 5)
			} else {
				log::info!("{LOG_PREFIX} BlockRewards pallet already on version 2, migration can be removed");
				T::DbWeight::get().reads(1)
			}
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
			assert_eq!(
				Pallet::<T>::on_chain_storage_version(),
				StorageVersion::new(1),
			);
			assert!(
				Pallet::<T>::on_chain_storage_version() < Pallet::<T>::current_storage_version()
			);

			let active = ActiveSessionData::<T>::get();
			let next = NextSessionChanges::<T>::get();

			log::info!("{LOG_PREFIX} PRE UPGRADE: Finished");

			Ok((active, next).encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), TryRuntimeError> {
			let (old_active, old_next): (OldSessionData<T>, OldSessionChanges<T>) =
				Decode::decode(&mut pre_state.as_slice()).expect("Pre state valid; qed");
			let active = pallet::ActiveSessionData::<T>::get();
			let next = pallet::NextSessionChanges::<T>::get();

			assert_eq!(old_active.collator_reward, active.collator_reward);
			assert_eq!(old_active.collator_count, active.collator_count);
			assert_eq!(old_next.collators, next.collators);
			assert_eq!(old_next.collator_count, next.collator_count);
			assert_eq!(old_next.collator_reward, next.collator_reward);
			assert_eq!(
				next.treasury_inflation_rate,
				Some(inflation_rate::<T>(InflationPercentage::get()))
			);
			assert_eq!(
				Pallet::<T>::current_storage_version(),
				Pallet::<T>::on_chain_storage_version()
			);

			log::info!("{LOG_PREFIX} POST UPGRADE: Finished");
			Ok(())
		}
	}
}
