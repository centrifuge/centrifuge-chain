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

/// Trait for converting a pool+tranche ID pair to a CurrencyId
///
/// This should be implemented in the runtime to convert from the
/// PoolId and TrancheId types to a CurrencyId that represents that
/// tranche.
///
/// The pool epoch logic assumes that every tranche has a UNIQUE
/// currency, but nothing enforces that. Failure to ensure currency
/// uniqueness will almost certainly cause some wild bugs.
use super::*;
use common_traits::{Combinator, Tranche as TrancheT};
use frame_support::sp_runtime::ArithmeticError;

// Types alias for EpochExecutionTranche
pub(super) type EpochExecutionTrancheOf<T> = EpochExecutionTranche<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::TrancheWeight,
>;

// Types alias for Tranche
pub(super) type TrancheOf<T> =
	Tranche<<T as Config>::Balance, <T as Config>::InterestRate, <T as Config>::TrancheWeight>;

/// Trait for converting a pool+tranche ID pair to a CurrencyId
///
/// This should be implemented in the runtime to convert from the
/// PoolId and TrancheId types to a CurrencyId that represents that
/// tranche.
///
/// The pool epoch logic assumes that every tranche has a UNIQUE
/// currency, but nothing enforces that. Failure to ensure currency
/// uniqueness will almost certainly cause some wild bugs.
pub trait TrancheToken<T: Config> {
	fn tranche_token(pool: T::PoolId, tranche: T::TrancheId) -> T::CurrencyId;
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct TrancheInput<Rate> {
	pub interest_per_sec: Option<Rate>,
	pub min_risk_buffer: Option<Perquintill>,
	pub seniority: Option<Seniority>,
}

/// A representation of a tranche identifier that can be used as a storage key
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct TrancheLocator<PoolId, TrancheId> {
	pub pool_id: PoolId,
	pub tranche_id: TrancheId,
}

impl<PoolId, TrancheId> TrancheLocator<PoolId, TrancheId> {
	pub(super) fn new(pool_id: PoolId, tranche_id: TrancheId) -> Self {
		TrancheLocator {
			pool_id,
			tranche_id,
		}
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct Tranches<T: Config>(Vec<TrancheOf<T>>);

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct Tranche<Balance, Rate, Weight> {
	pub(super) interest_per_sec: Rate,
	pub(super) min_risk_buffer: Perquintill,
	pub(super) seniority: Seniority,

	pub(super) outstanding_invest_orders: Balance,
	pub(super) outstanding_redeem_orders: Balance,

	pub(super) debt: Balance,
	pub(super) reserve: Balance,
	pub(super) ratio: Perquintill,
	pub(super) last_updated_interest: Moment,

	pub(super) _phantom: PhantomData<Weight>,
}

impl<T: Config> Tranches<T> {
	fn supply(&self) -> Vec<Option<T::Balance>> {
		self.0
			.iter()
			.map(|tranche| tranche.debt.checked_add(&tranche.reserve))
			.collect()
	}

	fn acc_supply(&self) -> Result<T::Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(T::Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| {
					acc.checked_add(&tranche.debt)
						.and_then(|acc| acc.checked_add(&tranche.reserve))
				})
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	fn investments(&self) -> Vec<T::Balance> {
		self.0
			.iter()
			.map(|tranche| tranche.outstanding_invest_orders)
			.collect()
	}

	fn acc_investments(&self) -> Result<T::Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(T::Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.outstanding_invest_orders))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	fn redemptions(&self) -> Vec<T::Balance> {
		self.0
			.iter()
			.map(|tranche| tranche.outstanding_redeem_orders)
			.collect()
	}

	fn acc_redemptions(&self) -> Result<T::Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(T::Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.outstanding_redeem_orders))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	fn calculate_weight(&self) -> Vec<(T::TrancheWeight, T::TrancheWeight)> {
		let redeem_starts = 10u128.checked_pow(self.0.len()).unwrap_or(u128::MAX);

		self.0
			.iter()
			.map(|tranche| {
				(
					10u128
						.checked_pow(
							n_tranches
								.checked_sub(&tranche.seniority)
								.unwrap_or(u32::MAX),
						)
						.unwrap_or(u128::MAX)
						.into(),
					redeem_starts
						.checked_mul(10u128.pow(&tranche.seniority.saturating_add(1)).into())
						.unwrap_or(u128::MAX)
						.into(),
				)
			})
			.collect()
	}

	fn risk_buffers(&self) -> Vec<Perquintill> {
		self.0
			.iter()
			.map(|tranche| tranche.min_risk_buffer)
			.collect()
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionTranches<T: Config>(Vec<EpochExecutionTrancheOf<T>>);

impl<T: Config> EpochExecutionTranches<T> {
	fn supply(&self) -> Vec<T::Balance> {
		self.0.iter().map(|tranche| tranche.supply).collect()
	}

	fn acc_supply(&self) -> Result<T::Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(T::Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.supply))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	fn investments(&self) -> Vec<T::Balance> {
		self.0.iter().map(|tranche| tranche.invest).collect()
	}

	fn acc_investments(&self) -> Result<T::Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(T::Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.invest))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	fn redemptions(&self) -> Vec<T::Balance> {
		self.0.iter().map(|tranche| tranche.redeem).collect()
	}

	fn acc_redemptions(&self) -> Result<T::Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(T::Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.redeem))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	fn calculate_weight(&self) -> Vec<(T::TrancheWeight, T::TrancheWeight)> {
		let redeem_starts = 10u128.checked_pow(self.0.len()).unwrap_or(u128::MAX);

		self.0
			.iter()
			.map(|tranche| {
				(
					10u128
						.checked_pow(
							n_tranches
								.checked_sub(&tranche.seniority)
								.unwrap_or(u32::MAX),
						)
						.unwrap_or(u128::MAX)
						.into(),
					redeem_starts
						.checked_mul(10u128.pow(&tranche.seniority.saturating_add(1)).into())
						.unwrap_or(u128::MAX)
						.into(),
				)
			})
			.collect()
	}

	fn risk_buffers(&self) -> Vec<Perquintill> {
		self.0
			.iter()
			.map(|tranche| tranche.min_risk_buffer)
			.collect()
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionTranche<Balance, BalanceRatio, Weight> {
	pub(super) supply: Balance,
	pub(super) price: BalanceRatio,
	pub(super) invest: Balance,
	pub(super) redeem: Balance,
	pub(super) min_risk_buffer: Perquintill,
	pub(super) seniority: Seniority,

	pub(super) _phantom: PhantomData<Weight>,
}
