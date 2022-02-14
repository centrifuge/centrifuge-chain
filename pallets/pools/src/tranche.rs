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
use frame_support::sp_runtime::ArithmeticError;
use frame_support::sp_std::convert::TryInto;
use sp_arithmetic::traits::{BaseArithmetic, Unsigned};

/// Types alias for EpochExecutionTranches
pub(super) type EpochExecutionTranchesOf<T> = EpochExecutionTranches<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::TrancheWeight,
>;

/// Types alias for EpochExecutionTranche
#[allow(dead_code)]
pub(super) type EpochExecutionTrancheOf<T> = EpochExecutionTranche<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::TrancheWeight,
>;

/// Types alias for Tranches
pub(super) type TranchesOf<T> =
	Tranches<<T as Config>::Balance, <T as Config>::InterestRate, <T as Config>::TrancheWeight>;

/// Types alias for Tranche
pub(super) type TrancheOf<T> =
	Tranche<<T as Config>::Balance, <T as Config>::InterestRate, <T as Config>::TrancheWeight>;

/// Type that indicates the seniority of a tranche
type Seniority = u32;

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
pub struct Tranches<Balance, Rate, Weight>(pub(super) Vec<Tranche<Balance, Rate, Weight>>);

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

impl<Balance, Rate, Weight> Tranches<Balance, Rate, Weight>
where
	Balance: Zero + Copy + BaseArithmetic,
	Weight: Copy + From<u128>,
	Rate: One,
{
	pub fn from_input(tranches: impl Iterator<Item = TrancheInput<Rate>>, now: Moment) -> Self {
		Self(
			tranches
				.enumerate()
				.map(|(id, input)| Tranche {
					interest_per_sec: input.interest_per_sec.unwrap_or(One::one()),
					min_risk_buffer: input.min_risk_buffer.unwrap_or(Perquintill::zero()),
					// seniority increases as index since the order is from junior to senior
					seniority: input
						.seniority
						.unwrap_or(id.try_into().expect("MaxTranches is u32")),
					outstanding_invest_orders: Zero::zero(),
					outstanding_redeem_orders: Zero::zero(),

					debt: Zero::zero(),
					reserve: Zero::zero(),
					ratio: Perquintill::zero(),
					last_updated_interest: now,
					_phantom: Default::default(),
				})
				.collect(),
		)
	}

	pub fn num_tranches(&self) -> usize {
		self.0.len()
	}

	pub fn into_tranches(self) -> Vec<Tranche<Balance, Rate, Weight>> {
		self.0
	}

	pub fn as_tranche_slice(&self) -> &[Tranche<Balance, Rate, Weight>] {
		self.0.as_slice()
	}

	pub fn as_mut_tranche_slice(&mut self) -> &mut [Tranche<Balance, Rate, Weight>] {
		self.0.as_mut_slice()
	}

	pub fn supplies(&self) -> Result<Vec<Balance>, DispatchError> {
		self.0
			.iter()
			.map(|tranche| tranche.debt.checked_add(&tranche.reserve))
			.collect::<Option<Vec<_>>>()
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| {
					acc.checked_add(&tranche.debt)
						.and_then(|acc| acc.checked_add(&tranche.reserve))
				})
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn investments(&self) -> Vec<Balance> {
		self.0
			.iter()
			.map(|tranche| tranche.outstanding_invest_orders)
			.collect()
	}

	pub fn acc_investments(&self) -> Result<Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.outstanding_invest_orders))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn redemptions(&self) -> Vec<Balance> {
		self.0
			.iter()
			.map(|tranche| tranche.outstanding_redeem_orders)
			.collect()
	}

	pub fn acc_redemptions(&self) -> Result<Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.outstanding_redeem_orders))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn calculate_weights(&self) -> Vec<(Weight, Weight)> {
		let n_tranches: u32 = self.0.len().try_into().expect("MaxTranches is u32");
		let redeem_starts = 10u128.checked_pow(n_tranches).unwrap_or(u128::MAX);

		self.0
			.iter()
			.map(|tranche| {
				(
					10u128
						.checked_pow(
							n_tranches
								.checked_sub(tranche.seniority)
								.unwrap_or(u32::MAX),
						)
						.unwrap_or(u128::MAX)
						.into(),
					redeem_starts
						.checked_mul(10u128.pow(tranche.seniority.saturating_add(1)).into())
						.unwrap_or(u128::MAX)
						.into(),
				)
			})
			.collect()
	}

	pub fn min_risk_buffers(&self) -> Vec<Perquintill> {
		self.0
			.iter()
			.map(|tranche| tranche.min_risk_buffer)
			.collect()
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionTranches<Balance, BalanceRatio, Weight>(
	pub(super) Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>>,
);

impl<Balance, BalanceRatio, Weight> EpochExecutionTranches<Balance, BalanceRatio, Weight>
where
	Balance: Zero + Copy + BaseArithmetic + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	BalanceRatio: Copy,
{
	pub fn num_tranches(&self) -> usize {
		self.0.len()
	}

	pub fn into_tranches(self) -> Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		self.0
	}

	pub fn as_tranche_slice(&self) -> &[EpochExecutionTranche<Balance, BalanceRatio, Weight>] {
		self.0.as_slice()
	}

	pub fn as_mut_tranche_slice(
		&mut self,
	) -> &mut [EpochExecutionTranche<Balance, BalanceRatio, Weight>] {
		self.0.as_mut_slice()
	}

	pub fn prices(&self) -> Vec<BalanceRatio> {
		self.0.iter().map(|tranche| tranche.price).collect()
	}

	pub fn supplies_with_fulfillment(
		&self,
		fulfillments: &[TrancheSolution],
	) -> Result<Vec<Balance>, DispatchError> {
		self.0
			.iter()
			.zip(fulfillments)
			.map(|(tranche, solution)| {
				tranche
					.supply
					.checked_add(&solution.invest_fulfillment.mul_floor(tranche.invest))
					.and_then(|value| {
						value.checked_sub(&solution.redeem_fulfillment.mul_floor(tranche.redeem))
					})
			})
			.collect::<Option<Vec<_>>>()
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn acc_supply_with_fulfillment(
		&self,
		fulfillments: &[TrancheSolution],
	) -> Result<Balance, DispatchError> {
		self.supplies_with_fulfillment(fulfillments)?
			.iter()
			.fold(Some(Balance::zero()), |acc, add| {
				acc.and_then(|sum| sum.checked_add(add))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn supplies(&self) -> Vec<Balance> {
		self.0.iter().map(|tranche| tranche.supply).collect()
	}

	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.supply))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn investments(&self) -> Vec<Balance> {
		self.0.iter().map(|tranche| tranche.invest).collect()
	}

	pub fn acc_investments(&self) -> Result<Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.invest))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn redemptions(&self) -> Vec<Balance> {
		self.0.iter().map(|tranche| tranche.redeem).collect()
	}

	pub fn acc_redemptions(&self) -> Result<Balance, DispatchError> {
		self.0
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.redeem))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn calculate_weights(&self) -> Vec<(Weight, Weight)> {
		let n_tranches: u32 = self.0.len().try_into().expect("MaxTranches is u32");
		let redeem_starts = 10u128.checked_pow(n_tranches).unwrap_or(u128::MAX);

		self.0
			.iter()
			.map(|tranche| {
				(
					10u128
						.checked_pow(
							n_tranches
								.checked_sub(tranche.seniority)
								.unwrap_or(u32::MAX),
						)
						.unwrap_or(u128::MAX)
						.into(),
					redeem_starts
						.checked_mul(10u128.pow(tranche.seniority.saturating_add(1)).into())
						.unwrap_or(u128::MAX)
						.into(),
				)
			})
			.collect()
	}

	pub fn min_risk_buffers(&self) -> Vec<Perquintill> {
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
