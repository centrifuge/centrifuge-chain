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
