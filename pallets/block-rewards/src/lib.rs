// Copyright 2021 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
//! # BlockRewards Pallet
//!
//! The BlockRewards pallet provides functionality for distributing rewards to
//! different accounts with different currencies.
//! The distribution happens when an session (a constant time interval) finalizes.
//! Users cannot stake manually as their collator membership is syncronized via
//! a provider.
//! Thus, when new collators join, they will automatically be staked and vice-versa
//! when collators leave, they are unstaked.
//!
//! The BlockRewards pallet provides functions for:
//!
//! - Claiming the reward given for a staked currency. The reward will be the native network's token.
//! - Admin methods to configure the reward amount for collators and an optional beneficiary.
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod migrations;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use cfg_traits::{
	ops::{EnsureAdd, EnsureMul, EnsureSub},
	rewards::{AccountRewards, CurrencyGroupChange, GroupRewards},
};
use frame_support::{
	pallet_prelude::*,
	storage::transactional,
	traits::{
		fungibles::Mutate, tokens::Balance, Currency as CurrencyT, OnUnbalanced, OneSessionHandler,
	},
	DefaultNoBound,
};
use frame_system::pallet_prelude::*;
use num_traits::sign::Unsigned;
pub use pallet::*;
use sp_runtime::{traits::Zero, FixedPointOperand, SaturatedConversion, Saturating};
use sp_std::{mem, vec::Vec};
use weights::WeightInfo;

#[derive(
	Encode, Decode, DefaultNoBound, Clone, TypeInfo, MaxEncodedLen, PartialEq, RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(T))]
pub struct CollatorChanges<T: Config> {
	pub inc: BoundedVec<T::AccountId, T::MaxCollators>,
	pub out: BoundedVec<T::AccountId, T::MaxCollators>,
}

/// Type that contains the associated data of an session
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, PartialEq, Eq, RuntimeDebugNoBound)]
#[scale_info(skip_type_params(T))]
pub struct SessionData<T: Config> {
	/// Amount of rewards per session for a single collator.
	pub(crate) collator_reward: T::Balance,
	/// Total amount of rewards per session
	/// NOTE: Is ensured to be at least collator_reward * collator_count.
	pub(crate) total_reward: T::Balance,
	/// Number of current collators.
	/// NOTE: Updated automatically and thus not adjustable via extrinsic.
	pub collator_count: u32,
}

impl<T: Config> Default for SessionData<T> {
	fn default() -> Self {
		Self {
			collator_reward: T::Balance::zero(),
			total_reward: T::Balance::zero(),
			collator_count: 0,
		}
	}
}

/// Type that contains the pending update.
#[derive(
	PartialEq, Clone, DefaultNoBound, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(T))]
pub struct SessionChanges<T: Config> {
	pub collators: CollatorChanges<T>,
	pub collator_count: Option<u32>,
	collator_reward: Option<T::Balance>,
	total_reward: Option<T::Balance>,
}

pub(crate) type DomainIdOf<T> = <<T as Config>::Domain as TypedGet>::Type;
pub(crate) type NegativeImbalanceOf<T> = <<T as Config>::Currency as CurrencyT<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {

	use super::*;

	pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Required origin for admin purposes for configuring groups and currencies.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Type used to handle balances.
		type Balance: Balance
			+ MaxEncodedLen
			+ FixedPointOperand
			+ Into<<<Self as Config>::Currency as CurrencyT<Self::AccountId>>::Balance>
			+ MaybeSerializeDeserialize;

		/// Domain identification used by this pallet
		type Domain: TypedGet;

		/// Type used to handle group weights.
		type Weight: Parameter + MaxEncodedLen + EnsureAdd + Unsigned + FixedPointOperand + Default;

		/// The reward system used.
		type Rewards: GroupRewards<Balance = Self::Balance, GroupId = u32>
			+ AccountRewards<
				Self::AccountId,
				Balance = Self::Balance,
				CurrencyId = (DomainIdOf<Self>, <Self as Config>::CurrencyId),
			> + CurrencyGroupChange<
				GroupId = u32,
				CurrencyId = (DomainIdOf<Self>, <Self as Config>::CurrencyId),
			>;

		/// The type used to handle currency minting and burning for collators.
		type Currency: Mutate<Self::AccountId, AssetId = <Self as Config>::CurrencyId, Balance = Self::Balance>
			+ CurrencyT<Self::AccountId>;

		/// The currency type of the artificial block rewards currency.
		type CurrencyId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// The identifier of the artificial block rewards currency which is minted and burned for collators.
		#[pallet::constant]
		type StakeCurrencyId: Get<<Self as Config>::CurrencyId>;

		/// The amount of the artificial block rewards currency which is minted and burned for collators.
		#[pallet::constant]
		type StakeAmount: Get<<Self as Config>::Balance>;

		/// The identifier of the collator group.
		#[pallet::constant]
		type StakeGroupId: Get<u32>;

		/// Max number of changes of the same type enqueued to apply in the next session.
		/// Max calls to [`Pallet::set_collator_reward()`] or to [`Pallet::set_total_reward()`] with
		/// the same id.
		#[pallet::constant]
		type MaxChangesPerSession: Get<u32> + TypeInfo + sp_std::fmt::Debug + Clone + PartialEq;

		#[pallet::constant]
		type MaxCollators: Get<u32> + TypeInfo + sp_std::fmt::Debug + Clone + PartialEq;

		/// Target of receiving non-collator-rewards.
		/// NOTE: If set to none, collators are the only group receiving rewards.
		type Beneficiary: OnUnbalanced<NegativeImbalanceOf<Self>>;

		/// The identifier type for an authority.
		type AuthorityId: Member
			+ Parameter
			+ sp_runtime::RuntimeAppPublic
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// Information of runtime weightsk
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	/// Data associated to the current session.
	#[pallet::storage]
	#[pallet::getter(fn active_session_data)]
	pub(super) type ActiveSessionData<T: Config> = StorageValue<_, SessionData<T>, ValueQuery>;

	/// Pending update data used when the current session finalizes.
	/// Once it's used for the update, it's reset.
	#[pallet::storage]
	#[pallet::getter(fn next_session_changes)]
	pub(super) type NextSessionChanges<T: Config> = StorageValue<_, SessionChanges<T>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NewSession {
			collator_reward: T::Balance,
			total_reward: T::Balance,
			last_changes: SessionChanges<T>,
		},
		SessionAdvancementFailed {
			error: DispatchError,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Limit of max calls with same id to [`Pallet::set_collator_reward()`] or
		/// [`Pallet::set_total_reward()`] reached.
		MaxChangesPerSessionReached,
		InsufficientTotalReward,
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub collators: Vec<T::AccountId>,
		pub collator_reward: T::Balance,
		pub total_reward: T::Balance,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				collators: Default::default(),
				collator_reward: Default::default(),
				total_reward: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			T::Rewards::attach_currency(
				(T::Domain::get(), T::StakeCurrencyId::get()),
				T::StakeGroupId::get(),
			)
			.map_err(|e| log::error!("Failed to attach currency to collator group: {:?}", e))
			.ok();

			ActiveSessionData::<T>::mutate(|session_data| {
				session_data.collator_count = self.collators.len().saturated_into();
				session_data.collator_reward = self.collator_reward;
				session_data.total_reward = self.total_reward;
			});

			// Enables rewards already in genesis session.
			for collator in &self.collators {
				Pallet::<T>::do_init_collator(collator)
					.map_err(|e| {
						log::error!("Failed to init genesis collators for rewards: {:?}", e);
					})
					.ok();
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claims the reward the associated to a currency.
		/// The reward will be transferred to the target account.
		#[pallet::weight(T::WeightInfo::claim_reward())]
		#[pallet::call_index(0)]
		pub fn claim_reward(origin: OriginFor<T>, account_id: T::AccountId) -> DispatchResult {
			ensure_signed(origin)?;

			T::Rewards::claim_reward((T::Domain::get(), T::StakeCurrencyId::get()), &account_id)
				.map(|_| ())
		}

		/// Admin method to set the reward amount for a collator used for the next sessions.
		/// Current session is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_collator_reward())]
		#[pallet::call_index(1)]
		pub fn set_collator_reward(
			origin: OriginFor<T>,
			collator_reward_per_session: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextSessionChanges::<T>::try_mutate(|changes| {
				let current = ActiveSessionData::<T>::get();
				let total_collator_rewards = collator_reward_per_session.saturating_mul(
					changes
						.collator_count
						.unwrap_or(current.collator_count)
						.into(),
				);
				let total_rewards = changes.total_reward.unwrap_or(current.total_reward);
				ensure!(
					total_rewards >= total_collator_rewards,
					Error::<T>::InsufficientTotalReward
				);

				changes.collator_reward = Some(collator_reward_per_session);
				Ok(())
			})
		}

		/// Admin method to set the total reward distribution for the next sessions.
		/// Current session is not affected by this call.
		///
		/// Throws if total_reward < collator_reward * collator_count.
		#[pallet::weight(T::WeightInfo::set_total_reward())]
		#[pallet::call_index(2)]
		pub fn set_total_reward(
			origin: OriginFor<T>,
			total_reward_per_session: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextSessionChanges::<T>::try_mutate(|changes| {
				let current = ActiveSessionData::<T>::get();
				let total_collator_rewards = changes
					.collator_reward
					.unwrap_or(current.collator_reward)
					.saturating_mul(
						changes
							.collator_count
							.unwrap_or(current.collator_count)
							.into(),
					);
				ensure!(
					total_reward_per_session >= total_collator_rewards,
					Error::<T>::InsufficientTotalReward
				);

				changes.total_reward = Some(total_reward_per_session);
				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Mint default amount of stake for target address and deposit stake.
	/// Enables receiving rewards onwards.
	///
	/// Weight (6 reads, 6 writes):
	///  * mint_into (2 reads, 2 writes): Account, TotalIssuance
	///  * deposit_stake (4 reads, 4 writes): Currency, Group, StakeAccount, Account
	pub(crate) fn do_init_collator(who: &T::AccountId) -> DispatchResult {
		T::Currency::mint_into(T::StakeCurrencyId::get(), who, T::StakeAmount::get())?;
		T::Rewards::deposit_stake(
			(T::Domain::get(), T::StakeCurrencyId::get()),
			who,
			T::StakeAmount::get(),
		)
	}

	/// Withdraw currently staked amount for target address and immediately burn it.
	/// Disables receiving rewards onwards.
	pub(crate) fn do_exit_collator(who: &T::AccountId) -> DispatchResult {
		let amount = T::Rewards::account_stake((T::Domain::get(), T::StakeCurrencyId::get()), who);
		T::Rewards::withdraw_stake((T::Domain::get(), T::StakeCurrencyId::get()), who, amount)?;
		T::Currency::burn_from(T::StakeCurrencyId::get(), who, amount).map(|_| ())
	}

	/// Apply session changes and distribute rewards.
	///
	/// NOTE: Noop if any call fails.
	fn do_advance_session() {
		let mut num_joining = 0u32;
		let mut num_leaving = 0u32;

		transactional::with_storage_layer(|| -> DispatchResult {
			NextSessionChanges::<T>::try_mutate(|changes| -> DispatchResult {
				ActiveSessionData::<T>::try_mutate(|session_data| {
					// Reward collator group of last session
					let total_collator_reward = session_data
						.collator_reward
						.ensure_mul(session_data.collator_count.into())?
						.min(session_data.total_reward);
					T::Rewards::reward_group(T::StakeGroupId::get(), total_collator_reward)?;

					// Handle remaining reward
					let remaining = session_data
						.total_reward
						.ensure_sub(total_collator_reward)?;
					if !remaining.is_zero() {
						let reward = T::Currency::issue(remaining.into());
						// If configured, assigns reward to Beneficiary, else automatically drops it
						T::Beneficiary::on_unbalanced(reward);
					}

					num_joining = changes.collators.inc.len().saturated_into();
					num_leaving = changes.collators.out.len().saturated_into();

					// Apply collator set changes AFTER rewarding
					for leaving in changes.collators.out.iter() {
						Self::do_exit_collator(leaving)?;
					}
					for joining in changes.collators.inc.iter() {
						Self::do_init_collator(joining)?;
					}

					// Apply session changes
					session_data.collator_reward = changes
						.collator_reward
						.unwrap_or(session_data.collator_reward);
					session_data.total_reward =
						changes.total_reward.unwrap_or(session_data.total_reward);
					session_data.collator_count = changes
						.collator_count
						.unwrap_or(session_data.collator_count);

					Self::deposit_event(Event::NewSession {
						total_reward: session_data.total_reward,
						collator_reward: session_data.collator_reward,
						last_changes: mem::take(changes),
					});

					Ok(())
				})
			})
		})
		.map_err(|error| {
			Self::deposit_event(Event::SessionAdvancementFailed { error });
		})
		.ok();
	}
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::AuthorityId;
}

// Should be instantiated after the original SessionHandler such that current and queued collators are up-to-date for the current session.
impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
	type Key = T::AuthorityId;

	fn on_genesis_session<'a, I: 'a>(_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// Handled in genesis builder.
	}

	fn on_new_session<'a, I: 'a>(_: bool, validators: I, queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// MUST be called before updating collator set changes.
		// Else the timing is off.
		Self::do_advance_session();
		let current = validators
			.map(|(acc_id, _)| acc_id.clone())
			.collect::<Vec<_>>();
		let next = queued_validators
			.map(|(acc_id, _)| acc_id.clone())
			.collect::<Vec<_>>();

		// Prepare collator set changes for next session.
		if current != next {
			// Prepare for next session
			NextSessionChanges::<T>::mutate(
				|SessionChanges {
				     collators,
				     collator_count,
				     ..
				 }| {
					let inc = next
						.clone()
						.into_iter()
						.filter(|n| !current.iter().any(|curr| curr == n))
						.collect::<Vec<_>>();
					let out = current
						.clone()
						.into_iter()
						.filter(|curr| !next.iter().any(|n| n == curr))
						.collect::<Vec<_>>();
					collators.inc = BoundedVec::<_, T::MaxCollators>::truncate_from(inc);
					collators.out = BoundedVec::<_, T::MaxCollators>::truncate_from(out);

					// Should never require saturation as queued_validator length is bounded by u32.
					// Moreover, this bound pallet_aura::Config::MaxAuthorities is assumed to be the same
					// T::Config::MaxCollators.
					*collator_count = Some(next.len().saturated_into::<u32>());
				},
			);
		}
	}

	fn on_before_session_ending() {
		// we don't care.
	}

	fn on_disabled(_validator_index: u32) {
		// we don't care.
	}
}
