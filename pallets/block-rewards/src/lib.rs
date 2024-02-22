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
//! different accounts with different currencies as well as configuring an
//! annual treasury inflation. The distribution happens when a session (a
//! constant time interval) finalizes. Users cannot stake manually as their
//! collator membership is synchronized via a provider.
//! Thus, when new collators join, they will automatically be staked and
//! vice-versa when collators leave, they are unstaked.
//!
//! The BlockRewards pallet provides functions for:
//!
//! - Claiming the reward given for a staked currency. The reward will be the
//!   native network's token.
//! - Admin methods to configure the reward amount for collators and the annual
//!   treasury inflation.
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
	self,
	rewards::{AccountRewards, CurrencyGroupChange, GroupRewards},
	Seconds, TimeAsSecs,
};
use cfg_types::fixed_point::FixedPointNumberExtension;
use frame_support::{
	pallet_prelude::*,
	storage::transactional,
	traits::{
		fungible::{Inspect as FungibleInspect, Mutate as FungibleMutate},
		fungibles::Mutate,
		tokens::{Balance, Fortitude, Precision},
		OneSessionHandler,
	},
	DefaultNoBound, PalletId,
};
use frame_system::pallet_prelude::*;
use num_traits::sign::Unsigned;
pub use pallet::*;
use sp_runtime::{
	traits::{AccountIdConversion, EnsureAdd, EnsureMul, Zero},
	FixedPointNumber, FixedPointOperand, SaturatedConversion, Saturating,
};
use sp_std::{mem, vec::Vec};
pub use weights::WeightInfo;

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
	/// Number of current collators.
	/// NOTE: Updated automatically and thus not adjustable via extrinsic.
	pub collator_count: u32,
	/// The annual treasury inflation rate
	pub(crate) treasury_inflation_rate: T::Rate,
	/// The timestamp of the last update used for inflation proration
	pub(crate) last_update: Seconds,
}

impl<T: Config> Default for SessionData<T> {
	fn default() -> Self {
		Self {
			collator_count: 0,
			collator_reward: T::Balance::zero(),
			treasury_inflation_rate: T::Rate::zero(),
			last_update: Seconds::zero(),
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
	pub collator_reward: Option<T::Balance>,
	treasury_inflation_rate: Option<T::Rate>,
	last_update: Seconds,
}

#[frame_support::pallet]
pub mod pallet {

	use super::*;

	pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Required origin for admin purposes for configuring groups and
		/// currencies.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Type used to handle balances.
		type Balance: Balance + MaxEncodedLen + FixedPointOperand + MaybeSerializeDeserialize;

		#[pallet::constant]
		type ExistentialDeposit: Get<Self::Balance>;

		/// Type used to handle group weights.
		type Weight: Parameter + MaxEncodedLen + EnsureAdd + Unsigned + FixedPointOperand + Default;

		/// The reward system used.
		type Rewards: GroupRewards<Balance = Self::Balance, GroupId = u32>
			+ AccountRewards<
				Self::AccountId,
				Balance = Self::Balance,
				CurrencyId = <Self as Config>::CurrencyId,
			> + CurrencyGroupChange<GroupId = u32, CurrencyId = <Self as Config>::CurrencyId>;

		/// The type used to handle currency minting and burning for collators.
		type Tokens: Mutate<Self::AccountId, AssetId = <Self as Config>::CurrencyId, Balance = Self::Balance>
			+ FungibleMutate<Self::AccountId>
			+ FungibleInspect<Self::AccountId, Balance = Self::Balance>;

		/// The currency type of the artificial block rewards currency.
		type CurrencyId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// The identifier of the artificial block rewards currency which is
		/// minted and burned for collators.
		#[pallet::constant]
		type StakeCurrencyId: Get<<Self as Config>::CurrencyId>;

		/// The amount of the artificial block rewards currency which is minted
		/// and burned for collators.
		#[pallet::constant]
		type StakeAmount: Get<<Self as Config>::Balance>;

		/// The identifier of the collator group.
		#[pallet::constant]
		type StakeGroupId: Get<u32>;

		#[pallet::constant]
		type MaxCollators: Get<u32> + TypeInfo + sp_std::fmt::Debug + Clone + PartialEq;

		/// Treasury pallet
		type TreasuryPalletId: Get<PalletId>;

		/// The identifier type for an authority.
		type AuthorityId: Member
			+ Parameter
			+ sp_runtime::RuntimeAppPublic
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The inflation rate type
		type Rate: Parameter
			+ Member
			+ FixedPointNumberExtension
			+ TypeInfo
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The source of truth for the current time in seconds
		type Time: TimeAsSecs;

		/// Information of runtime weights
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
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
			treasury_inflation_rate: T::Rate,
			last_changes: SessionChanges<T>,
		},
		SessionAdvancementFailed {
			error: DispatchError,
		},
	}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub collators: Vec<T::AccountId>,
		pub collator_reward: T::Balance,
		pub treasury_inflation_rate: T::Rate,
		pub last_update: Seconds,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				collators: Default::default(),
				collator_reward: Default::default(),
				treasury_inflation_rate: Default::default(),
				last_update: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			T::Rewards::attach_currency(T::StakeCurrencyId::get(), T::StakeGroupId::get()).expect(
				"Should be able to attach default block rewards staking currency to collator group",
			);

			ActiveSessionData::<T>::mutate(|session_data| {
				session_data.collator_count = self.collators.len().saturated_into();
				session_data.collator_reward = self.collator_reward;
				session_data.treasury_inflation_rate = self.treasury_inflation_rate;
				session_data.last_update = self.last_update;
			});

			// Enables rewards already in genesis session.
			for collator in &self.collators {
				Pallet::<T>::do_init_collator(collator)
					.expect("Should not panic when initiating genesis collators for block rewards");
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

			T::Rewards::claim_reward(T::StakeCurrencyId::get(), &account_id).map(|_| ())
		}

		/// Admin method to set the reward amount for a collator used for the
		/// next sessions. Current session is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_collator_reward())]
		#[pallet::call_index(1)]
		pub fn set_collator_reward_per_session(
			origin: OriginFor<T>,
			collator_reward_per_session: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextSessionChanges::<T>::mutate(|c| {
				c.collator_reward = Some(collator_reward_per_session);
			});

			Ok(())
		}

		/// Admin method to set the treasury inflation rate for the next
		/// sessions. Current session is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_total_reward())]
		#[pallet::call_index(2)]
		pub fn set_annual_treasury_inflation_rate(
			origin: OriginFor<T>,
			treasury_inflation_rate: T::Rate,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextSessionChanges::<T>::mutate(|c| {
				c.treasury_inflation_rate = Some(treasury_inflation_rate);
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Mint default amount of stake for target address and deposit stake.
	/// Enables receiving rewards onwards.
	///
	/// Weight (6 reads, 6 writes):
	///  * mint_into (2 reads, 2 writes): Account, TotalIssuance
	///  * deposit_stake (4 reads, 4 writes): Currency, Group, StakeAccount,
	///    Account
	pub(crate) fn do_init_collator(who: &T::AccountId) -> DispatchResult {
		<T::Tokens as Mutate<T::AccountId>>::mint_into(
			T::StakeCurrencyId::get(),
			who,
			T::StakeAmount::get().saturating_add(T::ExistentialDeposit::get()),
		)?;
		T::Rewards::deposit_stake(T::StakeCurrencyId::get(), who, T::StakeAmount::get())
	}

	/// Withdraw currently staked amount for target address and immediately burn
	/// it. Disables receiving rewards onwards.
	pub(crate) fn do_exit_collator(who: &T::AccountId) -> DispatchResult {
		let amount = T::Rewards::account_stake(T::StakeCurrencyId::get(), who);
		T::Rewards::withdraw_stake(T::StakeCurrencyId::get(), who, amount)?;

		// NOTE: We currently must leave the `ED` in the account if it otherwise
		//       would get killed and down the line our orml-tokens prevents
		//       that.
		//
		//       I.e. this means stake currency issuance will grow over time if many
		//       collators leave and join.
		<T::Tokens as Mutate<T::AccountId>>::burn_from(
			T::StakeCurrencyId::get(),
			who,
			amount,
			Precision::Exact,
			Fortitude::Polite,
		)
		.map(|_| ())
	}

	/// Calculates the inflation proration based on the annual configuration and
	/// the session duration in seconds
	pub(crate) fn calculate_epoch_treasury_inflation(
		annual_inflation_rate: T::Rate,
		last_update: Seconds,
	) -> T::Balance {
		let total_issuance = <T::Tokens as FungibleInspect<T::AccountId>>::total_issuance();
		let session_duration = T::Time::now().saturating_sub(last_update);
		let inflation_proration =
			cfg_types::pools::saturated_rate_proration(annual_inflation_rate, session_duration);

		inflation_proration.saturating_mul_int(total_issuance)
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
						.ensure_mul(session_data.collator_count.into())?;
					T::Rewards::reward_group(T::StakeGroupId::get(), total_collator_reward)?;

					// Handle treasury inflation
					let treasury_inflation = Self::calculate_epoch_treasury_inflation(
						session_data.treasury_inflation_rate,
						session_data.last_update,
					);
					if !treasury_inflation.is_zero() {
						let _ = <T::Tokens as FungibleMutate<T::AccountId>>::mint_into(
							&T::TreasuryPalletId::get().into_account_truncating(),
							treasury_inflation,
						)
						.map_err(|e| {
							log::error!(
								"Failed to mint treasury inflation for session due to error {:?}",
								e
							)
						});
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
					session_data.treasury_inflation_rate = changes
						.treasury_inflation_rate
						.unwrap_or(session_data.treasury_inflation_rate);
					session_data.collator_count = changes
						.collator_count
						.unwrap_or(session_data.collator_count);
					session_data.last_update = T::Time::now();

					Self::deposit_event(Event::NewSession {
						collator_reward: session_data.collator_reward,
						treasury_inflation_rate: session_data.treasury_inflation_rate,
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

// Should be instantiated after the original SessionHandler such that current
// and queued collators are up-to-date for the current session.
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
					// Moreover, this bound pallet_aura::Config::MaxAuthorities is assumed to be the
					// same T::Config::MaxCollators.
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
