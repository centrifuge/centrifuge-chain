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

use cfg_primitives::Moment;
use frame_support::traits::Get;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::UnixTime, RuntimeDebug};
use scale_info::{build::Fields, Path, Type, TypeInfo};
use orml_traits::Change;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{pools::PoolChanges, tranches::{TrancheSolution, EpochExecutionTranches}};

/// The solutions struct for epoch solution
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum EpochSolution<Balance> {
	Healthy(HealthySolution<Balance>),
	Unhealthy(UnhealthySolution<Balance>),
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct HealthySolution<Balance> {
	pub solution: Vec<TrancheSolution>,
	pub score: Balance,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct UnhealthySolution<Balance> {
	pub state: Vec<UnhealthyState>,
	pub solution: Vec<TrancheSolution>,
	// The risk buffer score per tranche (less junior tranche) for this solution
	pub risk_buffer_improvement_scores: Option<Vec<Balance>>,
	// The reserve buffer score for this solution
	pub reserve_improvement_score: Option<Balance>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum UnhealthyState {
	MaxReserveViolated,
	MinRiskBufferViolated,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct EpochState<EpochId> {
	/// Current epoch that is ongoing.
	pub current: EpochId,
	/// Time when the last epoch was closed.
	pub last_closed: Moment,
	/// Last epoch that was executed.
	pub last_executed: EpochId,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct ScheduledUpdateDetails<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
	MaxTranches: Get<u32>,
{
	pub changes: PoolChanges<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>,
	pub scheduled_time: Moment,
}

/// The information for a currently executing epoch
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct EpochExecutionInfo<Balance, BalanceRatio, EpochId, Weight, BlockNumber, TrancheCurrency>
{
	epoch: EpochId,
	nav: Balance,
	reserve: Balance,
	max_reserve: Balance,
	tranches: EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency>,
	best_submission: Option<EpochSolution<Balance>>,
	challenge_period_end: Option<BlockNumber>,
}
