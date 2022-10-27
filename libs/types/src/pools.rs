// Copyright 2022 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::Moment;
use codec::{Decode, Encode};
use frame_support::{traits::Get, BoundedVec, RuntimeDebug};
use orml_traits::Change;
use scale_info::TypeInfo;

use super::*;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum UpdateState {
	NoExecution,
	Executed,
	Stored,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolChanges<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
	MaxTranches: Get<u32>,
{
	pub tranches: Change<BoundedVec<TrancheUpdate<Rate>, MaxTranches>>,
	pub tranche_metadata:
		Change<BoundedVec<TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>, MaxTranches>>,
	pub min_epoch_time: Change<Moment>,
	pub max_nav_age: Change<Moment>,
}

/// A representation of a pool identifier that can be converted to an account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolLocator<PoolId> {
	pub pool_id: PoolId,
}
