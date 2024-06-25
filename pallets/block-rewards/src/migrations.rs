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
	pallet_prelude::Weight,
	traits::{Get, GetStorageVersion, OnRuntimeUpgrade},
};
#[cfg(feature = "try-runtime")]
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
	use cfg_traits::rewards::{AccountRewards, CurrencyGroupChange, GroupRewards};
	use num_traits::Zero;
	use sp_runtime::SaturatedConversion;

	use super::*;

	const LOG_PREFIX: &str = "InitBlockRewards";
	pub struct InitBlockRewards<T, CollatorReward, AnnualTreasuryInflationPercent>(
		PhantomData<(T, CollatorReward, AnnualTreasuryInflationPercent)>,
	);

	impl<T, CollatorReward, AnnualTreasuryInflationPercent> OnRuntimeUpgrade
		for InitBlockRewards<T, CollatorReward, AnnualTreasuryInflationPercent>
	where
		T: frame_system::Config + Config<Balance = u128> + pallet_collator_selection::Config,
		T::Balance: From<u128>,
		CollatorReward: Get<T::Balance>,
		AnnualTreasuryInflationPercent: Get<u32>,
	{
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, TryRuntimeError> {
			let collators = pallet_collator_selection::Pallet::<T>::assemble_collators();
			assert!(!collators.is_empty());

			assert!(!CollatorReward::get().is_zero());

			log::info!("{LOG_PREFIX} Pre migration checks successful");

			Ok(collators.encode())
		}

		// Weight: 2 + collator_count reads and writes
		fn on_runtime_upgrade() -> Weight {
			log::info!("{LOG_PREFIX} Initiating migration");

			log::info!("{LOG_PREFIX} Checking whether group is ready");
			let group_is_ready = T::Rewards::is_ready(T::StakeGroupId::get());

			log::info!("{LOG_PREFIX} Checking whether session data is set");
			let session_data_already_set =
				pallet::ActiveSessionData::<T>::get() != Default::default();

			log::info!("{LOG_PREFIX} Getting list of collators");
			let collators = pallet_collator_selection::Pallet::<T>::assemble_collators();

			let collators_are_staked = collators.clone().into_iter().all(|c| {
				log::info!("{LOG_PREFIX} Checking stake of collator {c:?}");
				!T::Rewards::account_stake(T::StakeCurrencyId::get(), &c).is_zero()
			});
			let mut weight =
				T::DbWeight::get().reads(2u64.saturating_add(collators.len().saturated_into()));

			if group_is_ready && session_data_already_set && collators_are_staked {
				log::info!(
					"{LOG_PREFIX} Migration not necessary because all data is already initialized"
				);
				return weight;
			}

			if !group_is_ready {
				log::info!("{LOG_PREFIX} Attaching currency to collator group");

				T::Rewards::attach_currency(T::StakeCurrencyId::get(), T::StakeGroupId::get())
					.map_err(|e| {
						log::error!("Failed to attach currency to collator group: {:?}", e)
					})
					.ok();

				weight.saturating_accrue(T::DbWeight::get().writes(1));
			}

			if !session_data_already_set {
				log::info!("{LOG_PREFIX} Setting session data");

				pallet::ActiveSessionData::<T>::set(SessionData::<T> {
					collator_count: collators.len().saturated_into(),
					collator_reward: CollatorReward::get(),
					treasury_inflation_rate: inflation_rate::<T>(
						AnnualTreasuryInflationPercent::get(),
					),
					last_update: T::Time::now(),
				});
				weight.saturating_accrue(T::DbWeight::get().writes(1));
			}

			if !collators_are_staked {
				for collator in collators.iter() {
					if T::Rewards::account_stake(T::StakeCurrencyId::get(), collator).is_zero() {
						log::info!("{LOG_PREFIX} Adding stake for collator {collator:?}");
						// NOTE: Benching not required as num of collators <= 10.
						Pallet::<T>::do_init_collator(collator)
							.map_err(|e| {
								log::error!(
									"Failed to init genesis collators for rewards: {:?}",
									e
								);
							})
							.ok();
						weight.saturating_accrue(T::DbWeight::get().reads_writes(6, 6));
					}
				}
			}

			log::info!("{LOG_PREFIX} Migration complete");

			Pallet::<T>::current_storage_version().put::<Pallet<T>>();
			weight.saturating_add(T::DbWeight::get().writes(1))
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: sp_std::vec::Vec<u8>) -> Result<(), TryRuntimeError> {
			assert_eq!(
				Pallet::<T>::on_chain_storage_version(),
				Pallet::<T>::current_storage_version(),
				"On-chain storage version should be updated"
			);
			let collators: sp_std::vec::Vec<T::AccountId> =
				Decode::decode(&mut pre_state.as_slice())
					.expect("pre_upgrade provides a valid state; qed");

			assert_ne!(Pallet::<T>::active_session_data(), Default::default());

			for collator in collators.iter() {
				assert!(!T::Rewards::account_stake(T::StakeCurrencyId::get(), collator,).is_zero())
			}

			log::info!("{LOG_PREFIX} Post migration checks successful");

			Ok(())
		}
	}
}
