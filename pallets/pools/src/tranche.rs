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
use common_traits::TrancheToken as TrancheTokenT;
#[cfg(test)]
use common_types::CurrencyId;
use frame_support::sp_runtime::ArithmeticError;
use frame_support::sp_std::convert::TryInto;
use rev_slice::{RevSlice, SliceExt};
use sp_arithmetic::traits::{checked_pow, BaseArithmetic, Unsigned};

/// Types alias for EpochExecutionTranche
#[allow(dead_code)]
pub(super) type EpochExecutionTrancheOf<T> = EpochExecutionTranche<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::TrancheWeight,
>;

/// Types alias for Tranches
pub(super) type TranchesOf<T> = Tranches<
	<T as Config>::Balance,
	<T as Config>::InterestRate,
	<T as Config>::TrancheWeight,
	<T as Config>::CurrencyId,
>;

/// Types alias for Tranche
pub(super) type TrancheOf<T> = Tranche<
	<T as Config>::Balance,
	<T as Config>::InterestRate,
	<T as Config>::TrancheWeight,
	<T as Config>::CurrencyId,
>;

/// Type that indicates the seniority of a tranche
pub type Seniority = u32;
pub type TrancheInput<Rate> = (TrancheType<Rate>, Option<Seniority>);

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

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum TrancheType<Rate> {
	Residual,
	NonResidual {
		interest_per_sec: Rate,
		min_risk_buffer: Perquintill,
	},
}

impl<Rate> TrancheType<Rate>
where
	Rate: PartialOrd + PartialEq,
{
	/// Compares tranches with the following schema:
	///
	/// * (Residual, Residual) => true
	/// * (Residual, NonResidual) => true,
	/// * (NonResidual, Residual) => false,
	/// * (NonResidual, NonResidual) =>
	/// 		interest rate of next tranche must be smaller
	///         equal to the interest rate of self.
	///
	pub fn valid_next_tranche(&self, next: &TrancheType<Rate>) -> bool {
		match (self, next) {
			(TrancheType::Residual, TrancheType::Residual) => true,
			(TrancheType::Residual, TrancheType::NonResidual { .. }) => true,
			(TrancheType::NonResidual { .. }, TrancheType::Residual) => false,
			(
				TrancheType::NonResidual {
					interest_per_sec: ref interest_prev,
					..
				},
				TrancheType::NonResidual {
					interest_per_sec: ref interest_next,
					..
				},
			) => interest_prev >= interest_next,
		}
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Tranche<Balance, Rate, Weight, Currency>
where
	Rate: FixedPointNumber<Inner = Balance>,
	Balance: FixedPointOperand,
{
	pub(super) tranche_type: TrancheType<Rate>,
	pub(super) seniority: Seniority,
	pub(super) currency: Currency,

	pub(super) outstanding_invest_orders: Balance,
	pub(super) outstanding_redeem_orders: Balance,

	pub(super) debt: Balance,
	pub(super) reserve: Balance,
	pub(super) ratio: Perquintill,
	pub(super) last_updated_interest: Moment,

	pub(super) _phantom: PhantomData<Weight>,
}

#[cfg(test)]
impl<Balance, Rate, Weight> Default for Tranche<Balance, Rate, Weight, CurrencyId>
where
	Balance: One + Zero,
	Rate: FixedPointNumber<Inner = Balance> + One,
	Balance: FixedPointOperand + One + Zero,
{
	fn default() -> Self {
		Self {
			tranche_type: TrancheType::Residual,
			seniority: 1,
			currency: CurrencyId::Tranche(0, 0),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			debt: Zero::zero(),
			reserve: Zero::zero(),
			ratio: Perquintill::one(),
			last_updated_interest: 0,
			_phantom: PhantomData::default(),
		}
	}
}

impl<Balance, Rate, Weight, Currency> Tranche<Balance, Rate, Weight, Currency>
where
	Balance: Zero + Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
	Rate: FixedPointNumber<Inner = Balance> + One + Copy,
	Balance: FixedPointOperand,
	Weight: Copy + From<u128>,
{
	pub fn order_as_currency<BalanceRatio>(
		&self,
		price: &BalanceRatio,
	) -> Result<(Balance, Balance), DispatchError>
	where
		BalanceRatio: FixedPointNumber<Inner = Balance>,
	{
		Ok((
			self.outstanding_invest_orders,
			price
				.checked_mul_int(self.outstanding_redeem_orders)
				.ok_or(ArithmeticError::Overflow)?,
		))
	}

	pub fn balance(&self) -> Result<Balance, DispatchError> {
		self.debt
			.checked_add(&self.reserve)
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn free_balance(&self) -> Result<Balance, DispatchError> {
		Ok(self.reserve)
	}

	pub fn accrue(&mut self, now: Moment) -> DispatchResult {
		let delta = now - self.last_updated_interest;
		let interest = self.interest_per_sec();
		// NOTE: `checked_pow` can return 1 for 0^0 which is fine
		//       for us, as we simply have the same debt if this happens
		let total_interest = checked_pow(
			interest,
			delta
				.try_into()
				.map_err(|_| DispatchError::Other("Usize should be at least 64 bits."))?,
		)
		.ok_or(ArithmeticError::Overflow)?;
		self.debt = total_interest
			.checked_mul_int(self.debt)
			.ok_or(ArithmeticError::Overflow)?;
		self.last_updated_interest = now;

		Ok(())
	}

	pub fn min_risk_buffer(&self) -> Perquintill {
		match &self.tranche_type {
			TrancheType::Residual => Perquintill::zero(),
			TrancheType::NonResidual {
				interest_per_sec: ref _interest_per_sec,
				ref min_risk_buffer,
			} => min_risk_buffer.clone(),
		}
	}

	pub fn interest_per_sec(&self) -> Rate {
		match &self.tranche_type {
			TrancheType::Residual => One::one(),
			TrancheType::NonResidual {
				ref interest_per_sec,
				min_risk_buffer: ref _min_risk_buffer,
			} => interest_per_sec.clone(),
		}
	}

	pub fn debt(&mut self, now: Moment) -> Result<Balance, DispatchError> {
		self.accrue(now)?;
		Ok(self.debt)
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Tranches<Balance, Rate, Weight, Currency>
where
	Rate: FixedPointNumber<Inner = Balance>,
	Balance: FixedPointOperand,
{
	tranches: Vec<Tranche<Balance, Rate, Weight, Currency>>,
}

impl<Balance, Rate, Weight, CurrencyId> Tranches<Balance, Rate, Weight, CurrencyId>
where
	CurrencyId: Copy,
	Balance: Zero + Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	Rate: One + Copy + FixedPointNumber<Inner = Balance>,
{
	pub fn from_input<
		PoolId: Copy,
		TrancheId: TryFrom<usize>,
		TrancheToken: TrancheTokenT<PoolId, TrancheId, CurrencyId>,
	>(
		pool_id: PoolId,
		tranche_inputs: Vec<TrancheInput<Rate>>,
		now: Moment,
	) -> Result<Self, DispatchError> {
		let mut tranches = Vec::with_capacity(tranche_inputs.len());

		for (id, (tranche_type, seniority)) in tranche_inputs.into_iter().enumerate() {
			tranches.push(Tranche {
				tranche_type,
				// seniority increases as index since the order is from junior to senior
				seniority: seniority
					.unwrap_or(id.try_into().map_err(|_| ArithmeticError::Overflow)?),
				currency: TrancheToken::tranche_token(
					pool_id,
					id.try_into().map_err(|_| ArithmeticError::Overflow)?,
				),

				outstanding_invest_orders: Zero::zero(),
				outstanding_redeem_orders: Zero::zero(),

				debt: Zero::zero(),
				reserve: Zero::zero(),
				ratio: Perquintill::zero(),
				last_updated_interest: now,
				_phantom: Default::default(),
			})
		}

		Ok(Self { tranches })
	}

	pub fn new(tranches: Vec<Tranche<Balance, Rate, Weight, CurrencyId>>) -> Self {
		Self { tranches }
	}

	pub fn fold<R, F>(&self, start: R, mut f: F) -> Result<R, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>, R) -> Result<R, DispatchError>,
	{
		let mut iter = self.tranches.iter();
		let mut r = if let Some(tranche) = iter.next() {
			f(tranche, start)?
		} else {
			return Ok(start);
		};

		while let Some(tranche) = iter.next() {
			r = f(tranche, r)?;
		}
		Ok(r)
	}

	pub fn combine<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in &self.tranches {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, CurrencyId>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in &mut self.tranches {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with<R, I, W, F>(&self, with: I, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, CurrencyId>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter_mut().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn fold_rev<R, F>(&self, start: R, mut f: F) -> Result<R, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>, R) -> Result<R, DispatchError>,
	{
		let mut iter = self.tranches.iter().rev();
		let mut r = if let Some(tranche) = iter.next() {
			f(tranche, start)?
		} else {
			return Ok(start);
		};

		while let Some(tranche) = iter.next() {
			r = f(tranche, r)?;
		}
		Ok(r)
	}

	pub fn combine_rev<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter().rev() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_rev<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, CurrencyId>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter_mut().rev() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_rev<R, I, W, F>(&self, with: I, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, CurrencyId>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter().rev().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut_rev<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, CurrencyId>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter_mut().rev().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn orders_as_currency<BalanceRatio>(
		&self,
		prices: &Vec<BalanceRatio>,
	) -> Result<Vec<(Balance, Balance)>, DispatchError>
	where
		BalanceRatio: FixedPointNumber<Inner = Balance>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for (tranche, price) in self.tranches.iter().zip(prices) {
			res.push(tranche.order_as_currency(price)?);
		}
		Ok(res)
	}

	pub fn calculate_prices<BalanceRatio, Tokens, AccountId>(
		&mut self,
		total_assets: Balance,
		now: Moment,
	) -> Result<Vec<BalanceRatio>, DispatchError>
	where
		BalanceRatio: FixedPointNumber<Inner = Balance>,
		Tokens: Inspect<AccountId, AssetId = CurrencyId, Balance = Balance>,
	{
		let mut remaining_assets = total_assets;
		let pool_is_zero = total_assets == Zero::zero();

		// we are gonna reverse the order
		// such that prices are calculated from most senior to junior
		// there by all the remaining assets are given to the most junior tranche
		let junior_tranche_id = 0;
		let rev_prices = self
			.tranches
			.iter_mut()
			.enumerate()
			.rev()
			.map(|(tranche_id, tranche)| {
				let total_issuance = Tokens::total_issuance(tranche.currency);

				if pool_is_zero || total_issuance == Zero::zero() {
					Ok(One::one())
				} else if tranche_id == junior_tranche_id {
					BalanceRatio::checked_from_rational(remaining_assets, total_issuance)
						.ok_or(ArithmeticError::Overflow.into())
				} else {
					tranche.accrue(now)?;
					let tranche_value = tranche.balance()?;

					let tranche_value = if tranche_value > remaining_assets {
						remaining_assets = Zero::zero();
						remaining_assets
					} else {
						remaining_assets -= tranche_value;
						tranche_value
					};
					BalanceRatio::checked_from_rational(tranche_value, total_issuance)
						.ok_or(ArithmeticError::Overflow.into())
				}
			})
			.collect::<Result<Vec<BalanceRatio>, DispatchError>>()?;

		// NOTE: For some reason the compiler does shit with the code (or I am too stupid, which is
		//       more likely) and does optimize away or something if I rev -> map -> rev. So doing this
		//       manually again here.
		//
		// TODO: Put into for loop and allocate upfront with capacity.
		Ok(rev_prices.into_iter().rev().collect())
	}

	pub fn num_tranches(&self) -> usize {
		self.tranches.len()
	}

	pub fn into_tranches(self) -> Vec<Tranche<Balance, Rate, Weight, CurrencyId>> {
		self.tranches
	}

	pub fn non_residual_tranches(&self) -> Option<&[Tranche<Balance, Rate, Weight, CurrencyId>]> {
		if let Some((_head, tail)) = self.tranches.as_slice().split_first() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn non_residual_tranches_mut(
		&mut self,
	) -> Option<&mut [Tranche<Balance, Rate, Weight, CurrencyId>]> {
		if let Some((_head, tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn residual_tranche(&self) -> Option<&Tranche<Balance, Rate, Weight, CurrencyId>> {
		if let Some((head, _tail)) = self.tranches.as_slice().split_first() {
			Some(head)
		} else {
			None
		}
	}

	pub fn residual_tranche_mut(
		&mut self,
	) -> Option<&mut Tranche<Balance, Rate, Weight, CurrencyId>> {
		if let Some((head, _tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(head)
		} else {
			None
		}
	}

	pub fn senior_to_junior_slice(&self) -> &RevSlice<Tranche<Balance, Rate, Weight, CurrencyId>> {
		self.tranches.rev()
	}

	pub fn senior_to_junior_slice_mut(
		&mut self,
	) -> &mut RevSlice<Tranche<Balance, Rate, Weight, CurrencyId>> {
		self.tranches.rev_mut()
	}

	pub fn junior_to_senior_slice(&self) -> &[Tranche<Balance, Rate, Weight, CurrencyId>] {
		self.tranches.as_slice()
	}

	pub fn junior_to_senior_slice_mut(
		&mut self,
	) -> &mut [Tranche<Balance, Rate, Weight, CurrencyId>] {
		self.tranches.as_mut_slice()
	}

	pub fn supplies(&self) -> Result<Vec<Balance>, DispatchError> {
		self.tranches
			.iter()
			.map(|tranche| tranche.debt.checked_add(&tranche.reserve))
			.collect::<Option<Vec<_>>>()
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		self.tranches
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
		self.tranches
			.iter()
			.map(|tranche| tranche.outstanding_invest_orders)
			.collect()
	}

	pub fn acc_investments(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.outstanding_invest_orders))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn redemptions(&self) -> Vec<Balance> {
		self.tranches
			.iter()
			.map(|tranche| tranche.outstanding_redeem_orders)
			.collect()
	}

	pub fn acc_redemptions(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.outstanding_redeem_orders))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn calculate_weights(&self) -> Vec<(Weight, Weight)> {
		let n_tranches: u32 = self.tranches.len().try_into().expect("MaxTranches is u32");
		let redeem_starts = 10u128.checked_pow(n_tranches).unwrap_or(u128::MAX);

		self.tranches
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
		self.tranches
			.iter()
			.map(|tranche| tranche.min_risk_buffer())
			.collect()
	}

	pub fn senorities(&self) -> Vec<Seniority> {
		self.tranches
			.iter()
			.map(|tranche| tranche.seniority)
			.collect::<Vec<_>>()
	}

	pub fn rebalance_tranches(
		&mut self,
		now: Moment,
		pool_total_reserve: Balance,
		pool_nav: Balance,
		tranche_ratios: &[Perquintill],
		executed_amounts: &[(Balance, Balance)],
	) -> DispatchResult {
		// Calculate the new fraction of the total pool value that each tranche contains
		// This is based on the tranche values at time of epoch close.
		let total_assets = pool_total_reserve
			.checked_add(&pool_nav)
			.ok_or(ArithmeticError::Overflow)?;

		// Calculate the new total asset value for each tranche
		// This uses the current state of the tranches, rather than the cached epoch-close-time values.
		let mut total_assets = total_assets;
		let tranche_assets = self.combine_with_mut_rev(
			executed_amounts.iter().rev(),
			|tranche, (invest, redeem)| {
				tranche.accrue(now)?;

				tranche
					.debt
					.checked_add(&tranche.reserve)
					.and_then(|value| value.checked_add(invest))
					.and_then(|value| value.checked_sub(redeem))
					.map(|value| {
						if value > total_assets {
							let assets = total_assets;
							total_assets = Zero::zero();
							assets
						} else {
							total_assets = total_assets.saturating_sub(value);
							value
						}
					})
					.ok_or(ArithmeticError::Overflow.into())
			},
		)?;

		// Rebalance tranches based on the new tranche asset values and ratios
		let mut remaining_nav = pool_nav;
		let mut remaining_reserve = pool_total_reserve;
		self.combine_with_mut_rev(
			tranche_ratios.iter().rev().zip(tranche_assets.iter()),
			|tranche, (ratio, value)| {
				tranche.ratio = *ratio;
				if tranche.tranche_type == TrancheType::Residual {
					tranche.debt = remaining_nav;
					tranche.reserve = remaining_reserve;
				} else {
					tranche.debt = ratio.mul_ceil(pool_nav);
					if tranche.debt > *value {
						tranche.debt = *value;
					}
					tranche.reserve = value.saturating_sub(tranche.debt);
					remaining_nav -= tranche.debt;
					remaining_reserve -= tranche.reserve;
				}
				Ok(())
			},
		)
		.map(|_| ())
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

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionTranches<Balance, BalanceRatio, Weight> {
	tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>>,
}

impl<Balance, BalanceRatio, Weight> EpochExecutionTranches<Balance, BalanceRatio, Weight>
where
	Balance: Zero + Copy + BaseArithmetic + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	BalanceRatio: Copy,
{
	pub fn new(tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>>) -> Self {
		Self { tranches }
	}

	pub fn non_residual_tranches(
		&self,
	) -> Option<&[EpochExecutionTranche<Balance, BalanceRatio, Weight>]> {
		if let Some((_head, tail)) = self.tranches.as_slice().split_first() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn non_residual_tranches_mut(
		&mut self,
	) -> Option<&mut [EpochExecutionTranche<Balance, BalanceRatio, Weight>]> {
		if let Some((_head, tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn residual_tranche(
		&self,
	) -> Option<&EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		if let Some((head, _tail)) = self.tranches.as_slice().split_first() {
			Some(head)
		} else {
			None
		}
	}

	pub fn residual_tranche_mut(
		&mut self,
	) -> Option<&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		if let Some((head, _tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(head)
		} else {
			None
		}
	}

	pub fn num_tranches(&self) -> usize {
		self.tranches.len()
	}

	pub fn into_tranches(self) -> Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		self.tranches
	}

	pub fn senior_to_junior_slice(
		&self,
	) -> &RevSlice<EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		self.tranches.rev()
	}

	pub fn senior_to_junior_slice_mut(
		&mut self,
	) -> &mut RevSlice<EpochExecutionTranche<Balance, BalanceRatio, Weight>> {
		self.tranches.rev_mut()
	}

	pub fn junior_to_senior_slice(
		&self,
	) -> &[EpochExecutionTranche<Balance, BalanceRatio, Weight>] {
		self.tranches.as_slice()
	}

	pub fn junior_to_senior_slice_mut(
		&mut self,
	) -> &mut [EpochExecutionTranche<Balance, BalanceRatio, Weight>] {
		self.tranches.as_mut_slice()
	}

	pub fn fold<R, F>(&self, start: R, mut f: F) -> Result<R, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			R,
		) -> Result<R, DispatchError>,
	{
		let mut iter = self.tranches.iter();
		let mut r = if let Some(tranche) = iter.next() {
			f(tranche, start)?
		} else {
			return Ok(start);
		};

		while let Some(tranche) = iter.next() {
			r = f(tranche, r)?;
		}
		Ok(r)
	}

	pub fn combine<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&EpochExecutionTranche<Balance, BalanceRatio, Weight>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in &self.tranches {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in &mut self.tranches {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with<R, I, W, F>(&self, with: I, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter_mut().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn fold_rev<R, F>(&self, start: R, mut f: F) -> Result<R, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			R,
		) -> Result<R, DispatchError>,
	{
		let mut iter = self.tranches.iter().rev();
		let mut r = if let Some(tranche) = iter.next() {
			f(tranche, start)?
		} else {
			return Ok(start);
		};

		while let Some(tranche) = iter.next() {
			r = f(tranche, r)?;
		}
		Ok(r)
	}

	pub fn combine_rev<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&EpochExecutionTranche<Balance, BalanceRatio, Weight>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter().rev() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_rev<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.tranches.iter_mut().rev() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_rev<R, I, W, F>(&self, with: I, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter().rev().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut_rev<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		// TODO: Would be nice to error out when with is larger than tranches...
		let iter = self.tranches.iter_mut().rev().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn prices(&self) -> Vec<BalanceRatio> {
		self.tranches.iter().map(|tranche| tranche.price).collect()
	}

	pub fn supplies_with_fulfillment(
		&self,
		fulfillments: &[TrancheSolution],
	) -> Result<Vec<Balance>, DispatchError> {
		self.combine_with(fulfillments, |tranche, solution| {
			tranche
				.supply
				.checked_add(&solution.invest_fulfillment.mul_floor(tranche.invest))
				.and_then(|value| {
					value.checked_sub(&solution.redeem_fulfillment.mul_floor(tranche.redeem))
				})
				.ok_or(ArithmeticError::Overflow.into())
		})
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
		self.tranches.iter().map(|tranche| tranche.supply).collect()
	}

	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.supply))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn investments(&self) -> Vec<Balance> {
		self.tranches.iter().map(|tranche| tranche.invest).collect()
	}

	pub fn acc_investments(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.invest))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn redemptions(&self) -> Vec<Balance> {
		self.tranches.iter().map(|tranche| tranche.redeem).collect()
	}

	pub fn acc_redemptions(&self) -> Result<Balance, DispatchError> {
		self.tranches
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.redeem))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn calculate_weights(&self) -> Vec<(Weight, Weight)> {
		let n_tranches: u32 = self.tranches.len().try_into().expect("MaxTranches is u32");
		let redeem_starts = 10u128.checked_pow(n_tranches).unwrap_or(u128::MAX);

		self.tranches
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
		self.tranches
			.iter()
			.map(|tranche| tranche.min_risk_buffer)
			.collect()
	}
}

pub(crate) fn calculate_risk_buffers<Balance, BalanceRatio>(
	tranche_supplies: &[Balance],
	tranche_prices: &[BalanceRatio],
) -> Result<Vec<Perquintill>, DispatchError>
where
	BalanceRatio: Copy + FixedPointNumber,
	Balance: Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
{
	let tranche_values: Vec<_> = tranche_supplies
		.iter()
		.zip(tranche_prices)
		.map(|(supply, price)| price.checked_mul_int(supply.clone()))
		.collect::<Option<Vec<_>>>()
		.ok_or(ArithmeticError::Overflow)?;

	let pool_value = tranche_values
		.iter()
		.fold(Some(Zero::zero()), |sum: Option<Balance>, tranche_value| {
			sum.and_then(|sum| sum.checked_add(tranche_value))
		})
		.ok_or(ArithmeticError::Overflow)?;

	// Iterate over the tranches senior => junior.
	// Buffer of most senior tranche is pool value - senior tranche value.
	// Buffer of each subordinate tranche is the buffer of the
	// previous more senior tranche - this tranche value.
	let mut remaining_subordinate_value = pool_value.clone();
	let mut risk_buffers: Vec<Perquintill> = tranche_values
		.iter()
		.rev()
		.map(|tranche_value| {
			remaining_subordinate_value = remaining_subordinate_value
				.checked_sub(tranche_value)
				.unwrap_or(Zero::zero());
			Perquintill::from_rational(remaining_subordinate_value, pool_value)
		})
		.collect::<Vec<Perquintill>>();

	risk_buffers.reverse();

	Ok(risk_buffers)
}

#[cfg(test)]
pub mod test {
	use super::*;

	#[test]
	fn reverse_slice_panics_on_out_of_bounds() {}

	#[test]
	fn reverse_works_for_both_tranches() {}
}
