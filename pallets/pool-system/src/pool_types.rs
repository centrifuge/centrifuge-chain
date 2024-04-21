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

use cfg_traits::Seconds;
use cfg_types::{epoch::EpochState, pools::TrancheMetadata};
use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::{DispatchError, RuntimeDebug},
	traits::Get,
	BoundedVec,
};
use orml_traits::Change;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::traits::{BaseArithmetic, Unsigned};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, One, Zero},
	FixedPointNumber, FixedPointOperand, TypeId,
};
use sp_std::{cmp::PartialEq, vec::Vec};

use crate::tranches::{
	EpochExecutionTranches, TrancheEssence, TrancheInput, TrancheSolution, TrancheUpdate, Tranches,
};

// The TypeId impl we derive pool-accounts from
impl<PoolId> TypeId for PoolLocator<PoolId> {
	const TYPE_ID: [u8; 4] = *b"pool";
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ReserveDetails<Balance> {
	/// Investments will be allowed up to this amount.
	pub max: Balance,
	/// Current total amount of currency in the pool reserve.
	pub total: Balance,
	/// Current reserve that is available for originations.
	pub available: Balance,
}

impl<Balance> ReserveDetails<Balance>
where
	Balance: AtLeast32BitUnsigned + Copy + From<u64>,
{
	/// Update the total balance of the reserve based on the provided solution
	/// for in- and outflows of this epoc.
	pub fn deposit_from_epoch<BalanceRatio, Weight, TrancheCurrency, MaxExecutionTranches>(
		&mut self,
		epoch_tranches: &EpochExecutionTranches<
			Balance,
			BalanceRatio,
			Weight,
			TrancheCurrency,
			MaxExecutionTranches,
		>,
		solution: &[TrancheSolution],
	) -> DispatchResult
	where
		Weight: Copy + From<u128>,
		BalanceRatio: Copy,
		MaxExecutionTranches: Get<u32>,
	{
		let executed_amounts = epoch_tranches.fulfillment_cash_flows(solution)?;

		// Update the total/available reserve for the new total value of the pool
		let mut acc_investments = Balance::zero();
		let mut acc_redemptions = Balance::zero();
		for &(invest, redeem) in executed_amounts.iter() {
			acc_investments.ensure_add_assign(invest)?;
			acc_redemptions.ensure_add_assign(redeem)?;
		}
		self.total = self
			.total
			.ensure_add(acc_investments)?
			.ensure_sub(acc_redemptions)?;

		Ok(())
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ScheduledUpdateDetails<Rate, StringLimit, MaxTranches>
where
	StringLimit: Get<u32>,
	MaxTranches: Get<u32>,
{
	pub changes: PoolChanges<Rate, StringLimit, MaxTranches>,
	pub submitted_at: Seconds,
}

/// A representation of a pool identifier that can be converted to an account
/// address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolLocator<PoolId> {
	pub pool_id: PoolId,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct PoolDetails<
	CurrencyId,
	TrancheCurrency,
	EpochId,
	Balance,
	Rate,
	Weight,
	TrancheId,
	PoolId,
	MaxTranches,
> where
	Rate: FixedPointNumber<Inner = Balance>,
	Balance: FixedPointOperand + sp_arithmetic::MultiplyRational,
	MaxTranches: Get<u32>,
	TrancheCurrency: Into<CurrencyId>,
{
	/// Currency that the pool is denominated in (immutable).
	pub currency: CurrencyId,
	/// List of tranches, ordered junior to senior.
	pub tranches: Tranches<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId, MaxTranches>,
	/// Details about the parameters of the pool.
	pub parameters: PoolParameters,
	/// The status the pool is currently in.
	pub status: PoolStatus,
	/// Details about the epochs of the pool.
	pub epoch: EpochState<EpochId>,
	/// Details about the reserve (unused capital) in the pool.
	pub reserve: ReserveDetails<Balance>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum PoolStatus {
	Open,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct PoolParameters {
	/// Minimum duration for an epoch.
	pub min_epoch_time: Seconds,
	/// Maximum time between the NAV update and the epoch closing.
	pub max_nav_age: Seconds,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct PoolChanges<Rate, StringLimit, MaxTranches>
where
	StringLimit: Get<u32>,
	MaxTranches: Get<u32>,
{
	pub tranches: Change<BoundedVec<TrancheUpdate<Rate>, MaxTranches>>,
	pub tranche_metadata: Change<BoundedVec<TrancheMetadata<StringLimit>, MaxTranches>>,
	pub min_epoch_time: Change<Seconds>,
	pub max_nav_age: Change<Seconds>,
}

/// Information about the deposit that has been taken to create a pool
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]
pub struct PoolDepositInfo<AccountId, Balance> {
	pub depositor: AccountId,
	pub deposit: Balance,
}

/// The core metadata about the pool which we can attach to an event
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolEssence<CurrencyId, Balance, TrancheCurrency, Rate, StringLimit>
where
	CurrencyId: Copy,
	StringLimit: Get<u32>,
	TrancheCurrency: Into<CurrencyId>,
{
	/// Currency that the pool is denominated in (immutable).
	pub currency: CurrencyId,
	/// The maximum allowed reserve on a given pool
	pub max_reserve: Balance,
	/// Maximum time between the NAV update and the epoch closing.
	pub max_nav_age: Seconds,
	/// Minimum duration for an epoch.
	pub min_epoch_time: Seconds,
	/// Tranches on a pool
	pub tranches: Vec<TrancheEssence<TrancheCurrency, Rate, StringLimit>>,
}

impl<
		CurrencyId,
		TrancheCurrency,
		EpochId,
		Balance,
		Rate,
		Weight,
		TrancheId,
		PoolId,
		MaxTranches,
	>
	PoolDetails<
		CurrencyId,
		TrancheCurrency,
		EpochId,
		Balance,
		Rate,
		Weight,
		TrancheId,
		PoolId,
		MaxTranches,
	> where
	Balance:
		FixedPointOperand + BaseArithmetic + Unsigned + From<u64> + sp_arithmetic::MultiplyRational,
	CurrencyId: Copy,
	TrancheCurrency: Into<CurrencyId>,
	EpochId: BaseArithmetic + Copy,
	PoolId: Copy + Encode,
	Rate: FixedPointNumber<Inner = Balance>,
	TrancheCurrency: Copy + cfg_traits::investments::TrancheCurrency<PoolId, TrancheId>,
	TrancheId: Clone + From<[u8; 16]> + PartialEq,
	Weight: Copy + From<u128>,
	MaxTranches: Get<u32>,
{
	pub fn start_next_epoch(&mut self, now: Seconds) -> DispatchResult {
		self.epoch.current.ensure_add_assign(One::one())?;
		self.epoch.last_closed = now;
		// TODO: Remove and set state rather to EpochClosing or similar
		// Set available reserve to 0 to disable originations while the epoch is closed
		// but not executed
		self.reserve.available = Zero::zero();

		Ok(())
	}

	pub fn execute_previous_epoch(&mut self) -> DispatchResult {
		self.reserve.available = self.reserve.total;
		self.epoch
			.last_executed
			.ensure_add_assign(One::one())
			.map_err(Into::into)
	}

	/// Constructs the pool essence from tranche metadata stored in the asset
	/// registry.
	///
	/// Must not be used for the creation of a new pool as each tranche token is
	/// required to be registered in the registry which happens after the pool
	/// creation for Subquery convenience.
	///
	/// NOTE: Uses [essence_from_tranche_input] under the hood.
	pub fn essence_from_registry<AssetRegistry, AssetId, StringLimit>(
		&self,
	) -> Result<PoolEssence<CurrencyId, Balance, TrancheCurrency, Rate, StringLimit>, DispatchError>
	where
		AssetRegistry:
			orml_traits::asset_registry::Inspect<StringLimit = StringLimit, AssetId = CurrencyId>,
		StringLimit: Get<u32>,
	{
		let mut ids: Vec<TrancheCurrency> = Vec::new();
		let mut tranche_input: Vec<TrancheInput<Rate, StringLimit>> = Vec::new();

		for tranche in self.tranches.residual_top_slice().iter() {
			let metadata = AssetRegistry::metadata(
				&<AssetRegistry as orml_traits::asset_registry::Inspect>::AssetId::from(
					tranche.currency.into(),
				),
			)
			.ok_or(DispatchError::CannotLookup)?;

			ids.push(tranche.currency);
			tranche_input.push(TrancheInput {
				tranche_type: tranche.tranche_type,
				seniority: Some(tranche.seniority),
				metadata: TrancheMetadata {
					token_name: metadata.name,
					token_symbol: metadata.symbol,
				},
			});
		}

		Self::essence_from_tranche_input::<StringLimit>(self, ids, tranche_input)
	}

	/// Constructs the pool essence from the input tranche metadata such that.
	///
	/// Does not require each tranche token to be registered in some storage
	/// such that this function can be called when a new pool is created.
	pub fn essence_from_tranche_input<StringLimit>(
		&self,
		ids: Vec<TrancheCurrency>,
		tranche_input: Vec<TrancheInput<Rate, StringLimit>>,
	) -> Result<PoolEssence<CurrencyId, Balance, TrancheCurrency, Rate, StringLimit>, DispatchError>
	where
		StringLimit: Get<u32>,
		TrancheCurrency: Into<CurrencyId>,
	{
		let mut tranche_essence: Vec<TrancheEssence<TrancheCurrency, Rate, StringLimit>> =
			Vec::new();

		for (id, input) in ids.into_iter().zip(tranche_input) {
			tranche_essence.push(TrancheEssence {
				currency: id,
				ty: input.tranche_type,
				metadata: TrancheMetadata {
					token_name: input.metadata.token_name,
					token_symbol: input.metadata.token_symbol,
				},
			});
		}

		Ok(PoolEssence {
			currency: self.currency,
			max_reserve: self.reserve.max,
			max_nav_age: self.parameters.max_nav_age,
			min_epoch_time: self.parameters.min_epoch_time,
			tranches: tranche_essence,
		})
	}
}

pub mod changes {
	use frame_support::storage::bounded_btree_set::BoundedBTreeSet;
	use sp_std::collections::btree_set::BTreeSet;
	use strum::EnumCount;

	use super::*;

	/// Requirements to perform the change
	#[derive(
		Encode,
		Decode,
		Clone,
		PartialEq,
		Eq,
		PartialOrd,
		Ord,
		TypeInfo,
		RuntimeDebug,
		MaxEncodedLen,
		EnumCount,
	)]
	pub enum Requirement {
		/// Required time the change must be noted to be able to release it.
		/// Measured in seconds.
		DelayTime(u32),

		/// The change requires to be noted at least until the current epoch
		/// finalizes.
		NextEpoch,

		/// Evaluates if the change must be blocked if redemptions are locked.
		BlockedByLockedRedemptions,
	}

	/// Wrapper type to identify equality between variants,
	/// without taking into account their inner values
	#[derive(Encode, Decode, Clone, Eq, PartialOrd, Ord, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub struct UniqueRequirement(pub Requirement);

	impl PartialEq for UniqueRequirement {
		fn eq(&self, other: &Self) -> bool {
			match self.0 {
				Requirement::DelayTime(_) => {
					matches!(other.0, Requirement::DelayTime(_))
				}
				Requirement::NextEpoch => {
					matches!(other.0, Requirement::NextEpoch)
				}
				Requirement::BlockedByLockedRedemptions => {
					matches!(other.0, Requirement::BlockedByLockedRedemptions)
				}
			}
		}
	}

	impl From<Requirement> for UniqueRequirement {
		fn from(value: Requirement) -> Self {
			UniqueRequirement(value)
		}
	}

	/// Type representing the length of different variants
	pub struct MaxRequirements;

	impl Get<u32> for MaxRequirements {
		fn get() -> u32 {
			Requirement::COUNT as u32
		}
	}

	/// Defines a change proposal with a list of requirements that must be
	/// satisfied.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct PoolChangeProposal {
		pub requirements: BoundedBTreeSet<UniqueRequirement, MaxRequirements>,
	}

	impl PoolChangeProposal {
		pub fn new(requirements: impl IntoIterator<Item = Requirement>) -> Self {
			Self {
                requirements: BTreeSet::from_iter(requirements.into_iter().map(UniqueRequirement))
                    .try_into()
                    .expect(
                        "Cannot exist more unique requirements in a set than `MaxRequirements`, qed",
                    ),
            }
		}

		pub fn requirements(&self) -> impl Iterator<Item = Requirement> + '_ {
			self.requirements.iter().cloned().map(|req| req.0)
		}
	}

	/// A PoolChangeProposal with extra information about when it was noted.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct NotedPoolChange<ChangeProposal: Into<PoolChangeProposal>> {
		pub submitted_time: Seconds,
		pub change: ChangeProposal,
	}
}

pub use changes::PoolChangeProposal;
