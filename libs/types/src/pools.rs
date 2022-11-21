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

use cfg_primitives::types::Balance;
use cfg_traits::InvestmentProperties;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::UnixTime, RuntimeDebug};
use scale_info::{build::Fields, Path, Type, TypeInfo};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Zero, Perquintill};
use sp_std::{
	cmp::{Ord, PartialEq, PartialOrd},
	marker::PhantomData,
};

/// A representation of a pool identifier that can be converted to an account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolLocator<PoolId> {
	pub pool_id: PoolId,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolDetails<
	CurrencyId,
	TrancheCurrency,
	EpochId,
	Balance,
	Rate,
	MetaSize,
	Weight,
	TrancheId,
	PoolId,
> where
	MetaSize: Get<u32> + Copy,
	Rate: FixedPointNumber<Inner = Balance>,
	Balance: FixedPointOperand,
{
	/// Currency that the pool is denominated in (immutable).
	pub currency: CurrencyId,
	/// List of tranches, ordered junior to senior.
	pub tranches: Tranches<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId>,
	/// Details about the parameters of the pool.
	pub parameters: PoolParameters,
	/// Metadata that specifies the pool.
	pub metadata: Option<BoundedVec<u8, MetaSize>>,
	/// The status the pool is currently in.
	pub status: PoolStatus,
	/// Details about the epochs of the pool.
	pub epoch: EpochState<EpochId>,
	/// Details about the reserve (unused capital) in the pool.
	pub reserve: ReserveDetails<Balance>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum PoolStatus {
	Open,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolParameters {
	/// Minimum duration for an epoch.
	pub min_epoch_time: Moment,
	/// Maximum time between the NAV update and the epoch closing.
	pub max_nav_age: Moment,
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

/// Information about the deposit that has been taken to create a pool
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct PoolDepositInfo<AccountId, Balance> {
	pub depositor: AccountId,
	pub deposit: Balance,
}

/// The core metadata about the pool which we can attach to an event
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolEssence<
	CurrencyId,
	Balance,
	TrancheCurrency,
	Rate,
	MaxTokenNameLength,
	MaxTokenSymbolLength,
> where
	CurrencyId: Copy,
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	/// Currency that the pool is denominated in (immutable).
	pub currency: CurrencyId,
	/// The maximum allowed reserve on a given pool
	pub max_reserve: Balance,
	/// Maximum time between the NAV update and the epoch closing.
	pub max_nav_age: Moment,
	/// Minimum duration for an epoch.
	pub min_epoch_time: Moment,
	/// Tranches on a pool
	pub tranches:
		Vec<TrancheEssence<TrancheCurrency, Rate, MaxTokenNameLength, MaxTokenSymbolLength>>,
}

impl<CurrencyId, TrancheCurrency, EpochId, Balance, Rate, MetaSize, Weight, TrancheId, PoolId>
	PoolDetails<
		CurrencyId,
		TrancheCurrency,
		EpochId,
		Balance,
		Rate,
		MetaSize,
		Weight,
		TrancheId,
		PoolId,
	> where
	Balance: FixedPointOperand + BaseArithmetic + Unsigned + From<u64>,
	CurrencyId: Copy,
	EpochId: BaseArithmetic,
	MetaSize: Get<u32> + Copy,
	PoolId: Copy + Encode,
	Rate: FixedPointNumber<Inner = Balance>,
	TrancheCurrency: Copy + cfg_traits::TrancheCurrency<PoolId, TrancheId>,
	TrancheId: Clone + From<[u8; 16]> + PartialEq,
	Weight: Copy + From<u128>,
{
	pub fn start_next_epoch(&mut self, now: Moment) -> DispatchResult {
		self.epoch.current += One::one();
		self.epoch.last_closed = now;
		// TODO: Remove and set state rather to EpochClosing or similar
		// Set available reserve to 0 to disable originations while the epoch is closed but not executed
		self.reserve.available = Zero::zero();

		Ok(())
	}

	fn execute_previous_epoch(&mut self) -> DispatchResult {
		self.reserve.available = self.reserve.total;
		self.epoch.last_executed += One::one();
		Ok(())
	}

	pub fn essence<
		T: Config<
			CurrencyId = CurrencyId,
			Balance = Balance,
			TrancheCurrency = TrancheCurrency,
			Rate = Rate,
		>,
	>(
		&self,
	) -> Result<PoolEssenceOf<T>, DispatchError> {
		let mut tranches: Vec<
			TrancheEssence<
				T::TrancheCurrency,
				T::Rate,
				T::MaxTokenNameLength,
				T::MaxTokenSymbolLength,
			>,
		> = Vec::new();

		for tranche in self.tranches.residual_top_slice().iter() {
			let metadata = T::AssetRegistry::metadata(&self.currency).ok_or(AssetMetadata {
				decimals: 0,
				name: Vec::new(),
				symbol: Vec::new(),
				existential_deposit: (),
				location: None,
				additional: (),
			});

			tranches.push(TrancheEssence {
				currency: tranche.currency.into(),
				ty: tranche.tranche_type.into(),
				metadata: TrancheMetadata {
					token_name: BoundedVec::try_from(metadata.clone().unwrap().name)
						.unwrap_or(BoundedVec::default()),
					token_symbol: BoundedVec::try_from(metadata.unwrap().symbol)
						.unwrap_or(BoundedVec::default()),
				},
			});
		}

		Ok(PoolEssence {
			currency: self.currency,
			max_reserve: self.reserve.max.into(),
			max_nav_age: self.parameters.max_nav_age,
			min_epoch_time: self.parameters.min_epoch_time,
			tranches,
		})
	}
}
