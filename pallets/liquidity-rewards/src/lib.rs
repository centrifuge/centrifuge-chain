// Copyright 2022 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

pub use cfg_traits::rewards::{
	AccountRewards, CurrencyGroupChange, DistributedRewards, GroupRewards,
};
use frame_support::traits::tokens::{AssetId, Balance};
use sp_runtime::FixedPointOperand;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Required origin for admin purposes for configuring groups and currencies.
		type AdminOrigin: EnsureOrigin<Self::Origin>;

		/// Type used to handle balances.
		type Balance: Balance + MaxEncodedLen + FixedPointOperand;

		/// Type used to identify currencies.
		type CurrencyId: AssetId + MaxEncodedLen + Clone;

		/// Type used to identify groups.
		type GroupId: Parameter;

		/// Type used to handle group weights.
		type Weight: Parameter;

		/// The reward system used.
		type Rewards: GroupRewards<Balance = Self::Balance, GroupId = Self::GroupId>
			+ AccountRewards<Self::AccountId, Balance = Self::Balance, CurrencyId = Self::CurrencyId>
			+ CurrencyGroupChange<GroupId = Self::GroupId, CurrencyId = Self::CurrencyId>
			+ DistributedRewards<Balance = Self::Balance, GroupId = Self::GroupId>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type Something<T> = StorageValue<_, u32>;

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)] // TODO
		pub fn stake(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			T::Rewards::deposit_stake(currency_id, &account_id, amount)
		}

		#[pallet::weight(10_000)] // TODO
		pub fn unstake(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			T::Rewards::withdraw_stake(currency_id, &account_id, amount)
		}

		#[pallet::weight(10_000)] // TODO
		pub fn claim_reward(origin: OriginFor<T>, currency_id: T::CurrencyId) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			T::Rewards::claim_reward(currency_id, &account_id).map(|_| ())
		}

		#[pallet::weight(10_000)] // TODO
		pub fn set_group_weight(
			origin: OriginFor<T>,
			group_id: T::GroupId,
			weight: T::Weight,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)] // TODO
		pub fn set_next_epoch_total_reward(
			origin: OriginFor<T>,
			balance: T::Balance,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)] // TODO
		pub fn set_blocks_per_epoch(origin: OriginFor<T>, balance: T::Balance) -> DispatchResult {
			todo!()
		}

		#[pallet::weight(10_000)] // TODO
		pub fn attach_currency(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			group_id: T::GroupId,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			T::Rewards::attach_currency(currency_id, group_id)
		}
	}
}
