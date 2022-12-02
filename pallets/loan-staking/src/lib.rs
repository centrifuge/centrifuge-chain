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

use codec::HasCompact;
use cfg_primitives::Moment;
use cfg_traits::{Permissions};
use cfg_types::{PermissionScope, PoolRole, Role};
use frame_support::{pallet_prelude::*, scale_info::TypeInfo, transactional, BoundedVec};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, BadOrigin},
	FixedPointNumber, FixedPointOperand,
};
use sp_std::vec::Vec;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod weights;

/*
Config:
Loans
MaxProposalsPerLoan

*/

pub struct Proposal<Balance, Pricing> {
	pricing: Pricing,
	staked_amount: Balance,
  stakers: BoundedVec<AccountId, MaxStakersPerProposal> // TODO: maybe should be a map?
}

pub struct LoanDetails<Balance, MaxProposalsPerLoan> {
	/// List of non-accepted proposals
	/// Cleared once a proposal was accepted
	proposals: Option<BoundedVec<Proposal<Balance>, MaxProposalsPerLoan>>,
	accepted_proposal: Option<Proposal<Balance>>,
	repaid_amount: Balance, // TODO: retrieve from loans?
	written_off_amount: Balance, // TODO: retrieve from loans?
}

pub enum MinStake<Balance> {
	Absolute(Balance),
	RelativeToMaxBorrowAmount(Perquintill),
}

pub enum ProposerSet {
	/// Accounts with the Borrower role on the pool can propose a loan
	Borrowers,
	/// Accounts with the PricingAdmin role on the pool can propose a loan
	PricingAdmins,
	/// Only external stakers can also propose pricing for new loans
	ExternalStakers,
}

pub enum StakerRestrictions {
  OnlyProposer,
  OnlyExternal,
  Combined { min_proposer_stake: Perquintill, min_external_stake: Perquintill }
}

pub struct PoolStakingParameters<Balance> {
	proposer_set: ProposerSet,
	external_staker_set: MemberlistId, // TODO
	min_stake_per_loan: MinStake<Balance>,
  staker_restrictions: StakerRestrictions,
	/// % of the repaid amount that is minted in residual tranche tokens for the stakers
	reward_rate: Rate,
}

/**
 * Three configurable options:
 * - Centralized pricing: proposer_set = Borrowers, min_proposer_stake = 100%, min_external_stake = 0%
 * - Centralized pricing w/ verification agent: ??? (verification agent shouldn't need to stake)
 * - Decentralized pricing: min_external_stake > 0
 * 
 * Accept == PricingAdmin equivalent action
 */

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ From<u128>
			+ TypeInfo
			+ TryInto<u64>;

		type PoolId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ core::fmt::Debug;

		type LoanId: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Copy
			+ TypeInfo
			+ From<u128>
			+ IsType<InstanceIdOf<Self>>;

    // TODO: type LoanInfo

    #[pallet::constant]
    type MaxProposalsPerLoan: Get<u32> + Copy + Member + scale_info::TypeInfo;

    #[pallet::constant]
    type MaxStakesPerAccount: Get<u32> + Copy + Member + scale_info::TypeInfo;
  
		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId, Moment>,
			Error = DispatchError,
		>;

		type Tokens: Mutate<Self::AccountId>
		+ Inspect<
			Self::AccountId,
			AssetId = CurrencyIdOf<Self>,
			Balance = <Self as pallet::Config>::Balance,
		> + Transfer<Self::AccountId>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn get_pools)]
	pub(crate) type ActiveStakes<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::PoolId,
		BoundedVec<T::LoanId, T::MaxStakesPerAccount>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		
	}

	#[pallet::error]
	pub enum Error<T> {
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {

    #[pallet::weight(80_000_000)]
		#[transactional]
		pub fn propose(
			origin: OriginFor<T>,
      pool_id: T::PoolId,
			loan_id: T::LoanId,
      pricing: T::Pricing,
      stake: T::Balance,
		) -> DispatchResult {
      // TODO
			// Check that loan is Created
			// Check that balance of first-loss tranche tokens is sufficient

			// Create proposal
			// Lock first-loss tranche tokens
			// Insert in ActiveStakes

      Ok(())
    }

    #[pallet::weight(80_000_000)]
		#[transactional]
		pub fn stake(
			origin: OriginFor<T>,
      pool_id: T::PoolId,
			loan_id: T::LoanId,
      pricing: T::Pricing,
      stake: T::Balance,
		) -> DispatchResult {
      // TODO

			// Check that proposal exists
			// Check that proposal was not accepted yet
			// Check that balance of first-loss tranche tokens is sufficient

			// Add stake to proposal
			// Lock first-loss tranche tokens
			// Insert in ActiveStakes

      Ok(())
    }

    #[pallet::weight(80_000_000)]
		#[transactional]
		pub fn unstake(
			origin: OriginFor<T>,
      pool_id: T::PoolId,
			loan_id: T::LoanId,
      pricing: T::Pricing,
		) -> DispatchResult {
      // TODO

			// Remove stake from proposal
			// Unlock first-loss tranche tokens

      Ok(())
    }

    #[pallet::weight(80_000_000)]
		#[transactional]
		pub fn accept(
			origin: OriginFor<T>,
      pool_id: T::PoolId,
			loan_id: T::LoanId,
      pricing: T::Pricing,
		) -> DispatchResult {
      // TODO

			// Price loan
			// Store proposal as accepted

      Ok(())
    }

    #[pallet::weight(80_000_000)]
		#[transactional]
		pub fn collect(
			origin: OriginFor<T>,
      pool_id: T::PoolId,
			loan_id: T::LoanId,
		) -> DispatchResult {
      // TODO

			// Check that Loan is closed or written-off or 100% ??

			// Remove from ActiveStakes

      Ok(())
    }

    #[pallet::weight(80_000_000)]
		#[transactional]
		pub fn update_pool_parameters(
			origin: OriginFor<T>,
      pool_id: T::PoolId,
      parameters: PoolStakingParameters<T::Balance>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who,
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				BadOrigin
			);

      // TODO
      Ok(())
    }

	}
}

// TODO: impl on_repay and on_write_off handlers
// on_repay: mint new first-loss tranche tokens
// on_write_off: burn first-loss tranche tokens; how to handle un-write-offs?