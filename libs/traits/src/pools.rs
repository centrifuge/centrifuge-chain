// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! # A common trait lib for centrifuge
//!
//! This crate provides some common traits used by centrifuge.

use codec::{Decode, Encode};
use frame_support::{
	dispatch::{
		DispatchErrorWithPostInfo, DispatchResult, DispatchResultWithPostInfo, PostDispatchInfo,
	},
	scale_info::TypeInfo,
	traits::Get,
	Parameter,
};
use sp_runtime::{traits::Member, DispatchError};
use sp_std::{fmt::Debug, vec::Vec};

use crate::PriceValue;

/// A trait that can be used to fetch the nav and update nav for a given pool
pub trait PoolNAV<PoolId, Amount> {
	type ClassId;
	type Origin;
	// nav returns the nav and the last time it was calculated
	fn nav(pool_id: PoolId) -> Option<(Amount, u64)>;
	fn update_nav(pool_id: PoolId) -> Result<Amount, DispatchError>;
	fn initialise(origin: Self::Origin, pool_id: PoolId, class_id: Self::ClassId)
		-> DispatchResult;
}

/// A trait that can make sure an update is allowed on a pool or not.
pub trait PoolUpdateGuard {
	type PoolDetails;
	type ScheduledUpdateDetails;
	type Moment: Copy;

	fn released(
		pool: &Self::PoolDetails,
		update: &Self::ScheduledUpdateDetails,
		now: Self::Moment,
	) -> bool;
}

/// A trait that support pool inspection operations such as pool existence checks and pool admin of permission set.
pub trait PoolInspect<AccountId, CurrencyId> {
	type PoolId: Parameter + Member + Debug + Copy + Default + TypeInfo;
	type TrancheId: Parameter + Member + Debug + Copy + Default + TypeInfo;
	type Rate;
	type Moment;

	/// check if the pool exists
	fn pool_exists(pool_id: Self::PoolId) -> bool;
	fn tranche_exists(pool_id: Self::PoolId, tranche_id: Self::TrancheId) -> bool;
	fn get_tranche_token_price(
		pool_id: Self::PoolId,
		tranche_id: Self::TrancheId,
	) -> Option<PriceValue<CurrencyId, Self::Rate, Self::Moment>>;
}

/// A trait that support pool reserve operations such as withdraw and deposit
pub trait PoolReserve<AccountId, CurrencyId>: PoolInspect<AccountId, CurrencyId> {
	type Balance;

	/// Withdraw `amount` from the reserve to the `to` account.
	fn withdraw(pool_id: Self::PoolId, to: AccountId, amount: Self::Balance) -> DispatchResult;

	/// Deposit `amount` from the `from` account into the reserve.
	fn deposit(pool_id: Self::PoolId, from: AccountId, amount: Self::Balance) -> DispatchResult;
}

/// A trait that supports modifications of pools
pub trait PoolMutate<
	AccountId,
	Balance,
	PoolId,
	CurrencyId,
	Rate,
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
	MaxTranches: Get<u32>,
>
{
	type TrancheInput: Encode + Decode + Clone;
	type PoolChanges: Encode + Decode + Clone;
	type UpdateState: Encode + Decode + Clone;

	fn create(
		admin: AccountId,
		depositor: AccountId,
		pool_id: PoolId,
		tranche_inputs: Vec<Self::TrancheInput>,
		currency: CurrencyId,
		max_reserve: Balance,
		metadata: Option<Vec<u8>>,
	) -> DispatchResult;

	fn update(
		pool_id: PoolId,
		changes: Self::PoolChanges,
	) -> Result<(Self::UpdateState, PostDispatchInfo), DispatchErrorWithPostInfo>;

	fn execute_update(pool_id: PoolId) -> DispatchResultWithPostInfo;
}
