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
#[cfg(test)]
use cfg_primitives::{Balance, PoolId, TrancheId, TrancheWeight};
use cfg_traits::TrancheCurrency as TrancheCurrencyT;
#[cfg(test)]
use cfg_types::{fixed_point::Rate, tokens::TrancheCurrency};
use cfg_types::{
	pools::TrancheMetadata,
	tokens::{CrossChainTransferability, CustomMetadata},
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	sp_runtime::ArithmeticError,
	traits::{fungibles::Inspect, Get, Len},
	Blake2_128, BoundedVec, Parameter, RuntimeDebug, StorageHasher,
};
use orml_traits::asset_registry::AssetMetadata;
use rev_slice::{RevSlice, SliceExt};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{checked_pow, BaseArithmetic, Unsigned};
use sp_runtime::{
	traits::{EnsureAdd, EnsureFixedPointNumber, EnsureInto, Member, One, Zero},
	DispatchError, FixedPointNumber, FixedPointOperand, Perquintill,
};
use sp_std::{marker::PhantomData, ops::Deref, vec::Vec};

/// Type that indicates the seniority of a tranche
pub type Seniority = u32;

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub struct TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	pub tranche_type: TrancheType<Rate>,
	pub seniority: Option<Seniority>,
	pub metadata: TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct TrancheUpdate<Rate> {
	pub tranche_type: TrancheType<Rate>,
	pub seniority: Option<Seniority>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TrancheLoc<TrancheId> {
	Index(TrancheIndex),
	Id(TrancheId),
}

/// The core metadata about a tranche which we can attach to an event
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct TrancheEssence<TrancheCurrency, Rate, MaxTokenNameLength, MaxTokenSymbolLength>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	/// Currency that the tranche is denominated in
	pub currency: TrancheCurrency,
	/// Type of the tranche (Residual or NonResidual)
	pub ty: TrancheType<Rate>,
	/// Metadata of a Tranche
	pub metadata: TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>,
}

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum TrancheType<Rate> {
	Residual,
	NonResidual {
		interest_rate_per_sec: Rate,
		min_risk_buffer: Perquintill,
	},
}

impl<Rate> TrancheType<Rate>
where
	Rate: PartialOrd + PartialEq,
{
	/// Compares tranches with the following schema:
	///
	/// * (Residual, Residual) => false
	/// * (Residual, NonResidual) => true,
	/// * (NonResidual, Residual) => false,
	/// * (NonResidual, NonResidual) => interest rate of next tranche must be
	///   smaller equal to the interest rate of self.
	pub fn valid_next_tranche(&self, next: &TrancheType<Rate>) -> bool {
		match (self, next) {
			(TrancheType::Residual, TrancheType::Residual) => false,
			(TrancheType::Residual, TrancheType::NonResidual { .. }) => true,
			(TrancheType::NonResidual { .. }, TrancheType::Residual) => false,
			(
				TrancheType::NonResidual {
					interest_rate_per_sec: ref interest_prev,
					..
				},
				TrancheType::NonResidual {
					interest_rate_per_sec: ref interest_next,
					..
				},
			) => interest_prev >= interest_next,
		}
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Tranche<Balance, Rate, Weight, CurrencyId> {
	pub tranche_type: TrancheType<Rate>,
	pub seniority: Seniority,
	pub currency: CurrencyId,

	pub debt: Balance,
	pub reserve: Balance,
	pub loss: Balance,
	pub ratio: Perquintill,
	pub last_updated_interest: Moment,

	pub _phantom: PhantomData<Weight>,
}

#[cfg(test)]
impl Default for Tranche<Balance, Rate, TrancheWeight, TrancheCurrency> {
	fn default() -> Self {
		Self {
			tranche_type: TrancheType::Residual,
			seniority: 1,
			currency: TrancheCurrency::generate(0, [0u8; 16]),
			debt: Zero::zero(),
			reserve: Zero::zero(),
			loss: Zero::zero(),
			ratio: Perquintill::one(),
			last_updated_interest: 0,
			_phantom: PhantomData::default(),
		}
	}
}

impl<Balance, Rate, Weight, Currency> Tranche<Balance, Rate, Weight, Currency>
where
	Balance: Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
	Rate: FixedPointNumber<Inner = Balance> + One + Copy,
	Balance: FixedPointOperand,
	Weight: Copy + From<u128>,
{
	/// Returns the sum of the debt and reserve amounts.
	pub fn balance(&self) -> Result<Balance, ArithmeticError> {
		self.debt.ensure_add(self.reserve)
	}

	/// Returns the reserve amount.
	pub fn free_balance(&self) -> Result<Balance, ArithmeticError> {
		Ok(self.reserve)
	}

	/// Update the debt of a Tranche by multiplying with the accrued interest
	/// since the last update:     debt = debt * interest_rate_per_second ^ (now
	/// - last_update)
	pub fn accrue(&mut self, now: Moment) -> Result<(), ArithmeticError> {
		let delta = now - self.last_updated_interest;
		let interest = self.interest_rate_per_sec();
		// NOTE: `checked_pow` can return 1 for 0^0 which is fine
		//       for us, as we simply have the same debt if this happens
		let total_interest =
			checked_pow(interest, delta.ensure_into()?).ok_or(ArithmeticError::Overflow)?;

		self.debt = total_interest.ensure_mul_int(self.debt)?;
		self.last_updated_interest = now;

		Ok(())
	}

	/// Returns the min risk buffer of a non-residual Tranche or zero.
	pub fn min_risk_buffer(&self) -> Perquintill {
		match &self.tranche_type {
			TrancheType::Residual => Perquintill::zero(),
			TrancheType::NonResidual {
				min_risk_buffer, ..
			} => *min_risk_buffer,
		}
	}

	/// Returns the interest rate per second for a non-residual Tranche or one.
	pub fn interest_rate_per_sec(&self) -> Rate {
		match &self.tranche_type {
			TrancheType::Residual => One::one(),
			TrancheType::NonResidual {
				interest_rate_per_sec,
				..
			} => *interest_rate_per_sec,
		}
	}

	/// Updates the debt by applying the accrued interest rate since the last
	/// update moment and returns it.
	pub fn debt(&mut self, now: Moment) -> Result<Balance, DispatchError> {
		self.accrue(now)?;
		Ok(self.debt)
	}

	pub fn create_asset_metadata(
		&self,
		decimals: u32,
		token_name: Vec<u8>,
		token_symbol: Vec<u8>,
	) -> AssetMetadata<Balance, CustomMetadata>
	where
		Balance: Zero,
		Currency: Encode,
		CustomMetadata: Parameter + Member + TypeInfo,
	{
		AssetMetadata {
			decimals,
			name: token_name,
			symbol: token_symbol,
			existential_deposit: Zero::zero(),
			location: None,
			additional: CustomMetadata {
				mintable: false,
				permissioned: true,
				pool_currency: false,
				transferability: CrossChainTransferability::LiquidityPools,
			},
		}
	}
}

/// The index type for tranches
///
/// The `TrancheIndex` can be seen as an normal index into a vector, just
/// specified here as new-type to make this clear. U64 in order to keep the
/// public api clear.
/// In contrast to a `TrancheId` a `TrancheIndex` is not unique and does NOT
/// refer to a specific tranche, but rather to a specific tranche-location in
/// the tranche-structure of a pool.
//
// Example:
//
// Given the following tranche structure:
// ----
// Tranche-A     -> Index: 0, Id: Blake2_128::hash(pool_id + 0)
// Tranche-B     -> Index: 1, Id: Blake2_128::hash(pool_id + 1)
// Tranche-C     -> Index: 2, Id: Blake2_128::hash(pool_id + 2)
// ----
//
// Now replacing Tranche-B with Tranche-D
// ----
// Tranche-A     -> Index: 0, Id: Blake2_128::hash(pool_id + 0)
// Tranche-D     -> Index: 1, Id: Blake2_128::hash(pool_id + 3)
// Tranche-C     -> Index: 2, Id: Blake2_128::hash(pool_id + 2)
// ----
//
// One can see, that the index of Tranche-B and Tranche-D are equal
// but their ids will be different.
pub type TrancheIndex = u64;

/// The salt type for tranches
///
/// This type is used to generate unique but deterministic ids
/// for tranches of a pool.
///
/// The assumption for uniqueness only holds as long as pool-ids
/// are not reusable!
pub type TrancheSalt<PoolId> = (TrancheIndex, PoolId);

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Tranches<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId, MaxTranches>
where
	MaxTranches: Get<u32>,
{
	pub tranches: BoundedVec<Tranche<Balance, Rate, Weight, TrancheCurrency>, MaxTranches>,
	pub ids: BoundedVec<TrancheId, MaxTranches>,
	pub salt: TrancheSalt<PoolId>,
}

#[cfg(test)]
impl
	Tranches<Balance, Rate, TrancheWeight, TrancheCurrency, TrancheId, PoolId, crate::mock::MaxTranches>
{
	pub fn new(
		pool: PoolId,
		tranches: Vec<Tranche<Balance, Rate, TrancheWeight, TrancheCurrency>>,
	) -> Result<Self, DispatchError> {
		let mut ids = Vec::with_capacity(tranches.len());
		let mut salt = (0, pool);

		for (index, _tranche) in tranches.iter().enumerate() {
			ids.push(Tranches::<
				Balance,
				Rate,
				TrancheWeight,
				TrancheCurrency,
				TrancheId,
				PoolId,
				crate::mock::MaxTranches,
			>::id_from_salt(salt));
			salt = (index.ensure_add(1)?.ensure_into()?, pool);
		}

		Ok(Self {
			tranches: BoundedVec::<
				Tranche<Balance, Rate, TrancheWeight, TrancheCurrency>,
				crate::mock::MaxTranches,
			>::truncate_from(tranches),
			ids: BoundedVec::<TrancheId, crate::mock::MaxTranches>::truncate_from(ids),
			salt,
		})
	}
}

// The solution struct for a specific tranche
#[derive(
	Encode, Decode, Copy, Clone, Eq, PartialEq, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TrancheSolution {
	pub invest_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
}

impl<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId, MaxTranches>
	Tranches<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId, MaxTranches>
where
	TrancheCurrency: Copy + TrancheCurrencyT<PoolId, TrancheId>,
	Balance: Zero + Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	Rate: One + Copy + FixedPointNumber<Inner = Balance>,
	TrancheId: Clone + From<[u8; 16]> + sp_std::cmp::PartialEq,
	PoolId: Copy + Encode,
	MaxTranches: Get<u32>,
{
	pub fn from_input<MaxTokenNameLength, MaxTokenSymbolLength>(
		pool: PoolId,
		tranche_inputs: Vec<TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>>,
		now: Moment,
	) -> Result<Self, DispatchError>
	where
		MaxTokenNameLength: Get<u32>,
		MaxTokenSymbolLength: Get<u32>,
	{
		let tranches = BoundedVec::with_bounded_capacity(tranche_inputs.len());
		let ids = BoundedVec::with_bounded_capacity(tranche_inputs.len());
		let salt = (0, pool);
		let mut tranches = Tranches {
			tranches,
			ids,
			salt,
		};

		for (index, tranche_input) in tranche_inputs.into_iter().enumerate() {
			tranches.add::<MaxTokenNameLength, MaxTokenSymbolLength>(
				index.ensure_into()?,
				tranche_input,
				now,
			)?;
		}

		Ok(tranches)
	}

	pub fn tranche_currency(&self, id: TrancheLoc<TrancheId>) -> Option<TrancheCurrency> {
		self.get_tranche(id).map(|tranche| tranche.currency)
	}

	pub fn tranche_id(&self, id: TrancheLoc<TrancheId>) -> Option<TrancheId> {
		match id {
			// to provide same validating behaviour as given by index tranche_id
			TrancheLoc::Id(id) => self.ids.iter().find(|x| **x == id).map(|_| id),
			TrancheLoc::Index(index) => index
				.try_into()
				.ok()
				.and_then(|index: usize| self.ids.deref().get(index).cloned()),
		}
	}

	pub fn tranche_index(&self, id: &TrancheLoc<TrancheId>) -> Option<TrancheIndex> {
		match id {
			TrancheLoc::Index(index) if *index < self.ids.len() as u64 => Some(*index),
			TrancheLoc::Index(_) => None,
			TrancheLoc::Id(id) => self
				.ids
				.iter()
				.position(|curr_id| curr_id == id)
				.and_then(|index| index.try_into().ok()),
		}
	}

	pub fn get_mut_tranche(
		&mut self,
		id: TrancheLoc<TrancheId>,
	) -> Option<&mut Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		match id {
			TrancheLoc::Index(index) => {
				let index: Option<usize> = index.try_into().ok();
				if let Some(index) = index {
					self.tranches.get_mut(index)
				} else {
					None
				}
			}
			TrancheLoc::Id(id) => {
				let index = self.tranche_index(&TrancheLoc::Id(id));
				if let Some(index) = index {
					let index: Option<usize> = index.try_into().ok();
					if let Some(index) = index {
						self.tranches.get_mut(index)
					} else {
						None
					}
				} else {
					None
				}
			}
		}
	}

	pub fn get_tranche(
		&self,
		id: TrancheLoc<TrancheId>,
	) -> Option<&Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		match id {
			TrancheLoc::Index(index) => {
				let index: Option<usize> = index.try_into().ok();
				if let Some(index) = index {
					self.tranches.deref().get(index)
				} else {
					None
				}
			}
			TrancheLoc::Id(id) => {
				let index = self.tranche_index(&TrancheLoc::Id(id));
				if let Some(index) = index {
					let index: Option<usize> = index.try_into().ok();
					if let Some(index) = index {
						self.tranches.deref().get(index)
					} else {
						None
					}
				} else {
					None
				}
			}
		}
	}

	/// Defines how a given salt will be transformed into
	/// a TrancheId.
	fn id_from_salt(salt: TrancheSalt<PoolId>) -> TrancheId {
		Blake2_128::hash(salt.encode().as_slice()).into()
	}

	/// Generate ids after the following schema:
	/// * salt: The salt is a counter in our case that will always go up, even
	///   if we remove tranches.
	/// * pool-id: The pool id is ensured to be unique on-chain
	///
	/// -> tranche id = Twox128::hash(salt)
	fn next_id(&mut self) -> Result<TrancheId, DispatchError> {
		let id =
			Tranches::<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId, MaxTranches>::id_from_salt(
				self.salt,
			);
		self.salt = (self.salt.0.ensure_add(1)?, self.salt.1);
		Ok(id)
	}

	fn create_tranche(
		&mut self,
		index: TrancheIndex,
		id: TrancheId,
		tranche_type: TrancheType<Rate>,
		seniority: Option<Seniority>,
		now: Moment,
	) -> Result<Tranche<Balance, Rate, Weight, TrancheCurrency>, DispatchError> {
		let tranche = Tranche {
			tranche_type,
			// seniority increases as index since the order is from junior to senior
			seniority: seniority.unwrap_or(index.ensure_into()?),
			currency: TrancheCurrency::generate(self.salt.1, id),
			debt: Zero::zero(),
			reserve: Zero::zero(),
			loss: Zero::zero(),
			ratio: Perquintill::zero(),
			last_updated_interest: now,
			_phantom: Default::default(),
		};
		Ok(tranche)
	}

	pub fn replace<MaxTokenNameLength, MaxTokenSymbolLength>(
		&mut self,
		at: TrancheIndex,
		tranche: TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>,
		now: Moment,
	) -> DispatchResult
	where
		MaxTokenNameLength: Get<u32>,
		MaxTokenSymbolLength: Get<u32>,
	{
		let at_idx = at;
		let i_at: usize = at.ensure_into()?;
		ensure!(
			    i_at <= self.tranches.len(),
			    DispatchError::Other(
				      "Must add tranches either in between others or at the end. This should be catched somewhere else."
			    )
		  );

		let id = self.next_id()?;
		let new_tranche = self.create_tranche(
			at_idx,
			id.clone(),
			tranche.tranche_type,
			tranche.seniority,
			now,
		)?;

		self.validate_insert(at, &new_tranche)?;
		self.remove(at)?;
		self.tranches
			.try_insert(i_at, new_tranche)
			.map_err(|_| ArithmeticError::Overflow)?;
		self.ids
			.try_insert(i_at, id)
			.map_err(|_| ArithmeticError::Overflow)?;
		Ok(())
	}

	pub fn validate_insert(
		&self,
		at: TrancheIndex,
		tranche: &Tranche<Balance, Rate, Weight, TrancheCurrency>,
	) -> DispatchResult {
		let i_at: usize = at.ensure_into()?;
		if i_at == 0 {
			ensure!(
				tranche.tranche_type == TrancheType::Residual,
				DispatchError::Other(
					"Top tranche must be a residual one. This should be catched somewhere else"
				)
			);
		} else {
			ensure!(
				self.tranches
					.get(i_at - 1)
					.expect("at is <= len and is not zero. An element before at must exist. qed.")
					.tranche_type
					.valid_next_tranche(&tranche.tranche_type),
				DispatchError::Other(
					"Invalid next tranche type. This should be catched somewhere else."
				)
			);
			if i_at < self.tranches.len() - 1 {
				ensure!(
					tranche.tranche_type.valid_next_tranche(
						&self.tranches
							.get(i_at + 1)
							.expect(
								  "at is <= len and is not zero. An element before at must exist. qed.",
							)
							.tranche_type
						),
					DispatchError::Other(
						"Invalid following tranche type. This should be catched somewhere else."
					)
				)
			}
		}
		Ok(())
	}

	pub fn add<MaxTokenNameLength, MaxTokenSymbolLength>(
		&mut self,
		at: TrancheIndex,
		tranche: TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>,
		now: Moment,
	) -> DispatchResult
	where
		MaxTokenNameLength: Get<u32>,
		MaxTokenSymbolLength: Get<u32>,
	{
		let at_idx = at;
		let i_at: usize = at.ensure_into()?;
		ensure!(
			at <= self.tranches.deref().len().ensure_into()?,
			DispatchError::Other(
				"Must add tranches either in between others or at the end. This should be catched somewhere else."
			)
		);

		let id = self.next_id()?;
		let new_tranche = self.create_tranche(
			at_idx,
			id.clone(),
			tranche.tranche_type,
			tranche.seniority,
			now,
		)?;

		self.validate_insert(at, &new_tranche)?;
		self.tranches
			.try_insert(i_at, new_tranche)
			.map_err(|_| ArithmeticError::Overflow)?;
		self.ids
			.try_insert(i_at, id)
			.map_err(|_| ArithmeticError::Overflow)?;

		Ok(())
	}

	/// Removing should only be possible if the Tranche at the given index has
	/// zero balance and is not the residual one.
	pub fn remove(&mut self, at: TrancheIndex) -> DispatchResult {
		self.get_tranche(TrancheLoc::Index(at))
			.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))
			.and_then(|tranche| -> DispatchResult {
				ensure!(
					tranche.tranche_type != TrancheType::Residual,
					DispatchError::Other("Must not remove residual tranche.")
				);

				ensure!(
					tranche.balance()?.is_zero(),
					DispatchError::Other(
						"Must not remove non-residual tranche with non-zero balance."
					)
				);

				Ok(())
			})?;

		let at: usize = at.ensure_into()?;
		self.tranches.remove(at);
		self.ids.remove(at);

		Ok(())
	}

	pub fn ids_non_residual_top(&self) -> Vec<TrancheId> {
		// TODO: Investigate refactor after rebasing to main
		// self.ids.iter().rev().map(|id| id.clone()).collect()
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		self.ids.iter().rev().for_each(|id| res.push(id.clone()));
		res
	}

	pub fn of_pool(&self) -> PoolId {
		self.salt.1
	}

	pub fn combine_non_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, TrancheCurrency>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		for tranche in self.non_residual_top_slice() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_non_residual_top<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, TrancheCurrency>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		for tranche in self.non_residual_top_slice_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_non_residual_top<R, I, W, F>(
		&self,
		with: I,
		f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, TrancheCurrency>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		combine_with(
			self.tranches.deref().len(),
			self.non_residual_top_slice().iter(),
			with,
			f,
		)
	}

	pub fn combine_with_mut_non_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut Tranche<Balance, Rate, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		combine_with_mut(
			self.tranches.len(),
			self.non_residual_top_slice_mut().iter_mut(),
			with,
			f,
		)
	}

	pub fn ids_residual_top(&self) -> Vec<TrancheId> {
		self.ids.clone().into_inner()
	}

	pub fn combine_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, TrancheCurrency>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		for tranche in self.residual_top_slice() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_residual_top<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&mut Tranche<Balance, Rate, Weight, TrancheCurrency>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		for tranche in self.residual_top_slice_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_residual_top<R, I, W, F>(
		&self,
		with: I,
		f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, TrancheCurrency>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		combine_with(
			self.tranches.deref().len(),
			self.residual_top_slice().iter(),
			with,
			f,
		)
	}

	pub fn combine_with_mut_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut Tranche<Balance, Rate, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		combine_with_mut(
			self.tranches.len(),
			self.residual_top_slice_mut().iter_mut(),
			with,
			f,
		)
	}

	/// Returns the current prices of the tranches based on the current NAV and
	/// each tranche's balance and total issuance at this exact moment.
	/// The correctness of the waterfall is ensured by starting at the top
	/// non-residual tranch.
	pub fn calculate_prices<BalanceRatio, Tokens, AccountId>(
		&mut self,
		total_assets: Balance,
		now: Moment,
	) -> Result<Vec<BalanceRatio>, DispatchError>
	where
		BalanceRatio: FixedPointNumber<Inner = Balance>,
		Tokens: Inspect<AccountId, Balance = Balance>,
		TrancheCurrency: Into<<Tokens as Inspect<AccountId>>::AssetId>,
	{
		let mut remaining_assets = total_assets;
		let pool_is_zero = total_assets.is_zero();

		// we are gonna reverse the order
		// such that prices are calculated from most senior to junior
		// there by all the remaining assets are given to the most junior tranche
		let mut prices = self.combine_mut_non_residual_top(|tranche| {
			// initial supply * accrued interest
			let total_issuance = Tokens::total_issuance(tranche.currency.into());

			if total_issuance.is_zero() {
				Ok(One::one())
			} else if pool_is_zero {
				Ok(Zero::zero())
			} else if tranche.tranche_type == TrancheType::Residual {
				Ok(BalanceRatio::ensure_from_rational(
					remaining_assets,
					total_issuance,
				)?)
			} else {
				tranche.accrue(now)?;
				let tranche_balance = tranche.balance()?;

				// Indicates that a tranche has been wiped out and/or a tranche has
				// lost value due to defaults.
				let tranche_value = if tranche_balance > remaining_assets {
					let left_over_assets = remaining_assets;
					remaining_assets = Zero::zero();
					left_over_assets
				} else {
					remaining_assets = remaining_assets.ensure_sub(tranche_balance)?;
					tranche_balance
				};
				Ok(BalanceRatio::ensure_from_rational(
					tranche_value,
					total_issuance,
				)?)
			}
		})?;

		// NOTE: We always pass around data in order Residual-to-NonResidual.
		//       -> So we need to reverse here again.
		prices.reverse();
		Ok(prices)
	}

	pub fn num_tranches(&self) -> usize {
		self.tranches.deref().len()
	}

	pub fn into_tranches(
		self,
	) -> BoundedVec<Tranche<Balance, Rate, Weight, TrancheCurrency>, MaxTranches> {
		self.tranches
	}

	pub fn non_residual_tranches(
		&self,
	) -> Option<&[Tranche<Balance, Rate, Weight, TrancheCurrency>]> {
		self.tranches
			.deref()
			.as_slice()
			.iter()
			.position(|tranche| matches!(tranche.tranche_type, TrancheType::NonResidual { .. }))
			.and_then(|index| self.tranches.as_slice().get(index..))
	}

	pub fn non_residual_tranches_mut(
		&mut self,
	) -> Option<&mut [Tranche<Balance, Rate, Weight, TrancheCurrency>]> {
		self.tranches
			.deref()
			.as_slice()
			.iter()
			.position(|tranche| matches!(tranche.tranche_type, TrancheType::NonResidual { .. }))
			.and_then(|index| self.tranches.get_mut(index..))
	}

	pub fn residual_tranche(&self) -> Option<&Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		self.tranches
			.deref()
			.as_slice()
			.iter()
			.position(|tranche| matches!(tranche.tranche_type, TrancheType::Residual))
			.and_then(|index| self.tranches.as_slice().get(index))
	}

	pub fn residual_tranche_mut(
		&mut self,
	) -> Option<&mut Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		self.tranches
			.deref()
			.as_slice()
			.iter()
			.position(|tranche| matches!(tranche.tranche_type, TrancheType::Residual))
			.and_then(|index| self.tranches.get_mut(index))
	}

	pub fn non_residual_top_slice(
		&self,
	) -> &RevSlice<Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		self.tranches.deref().rev()
	}

	pub fn non_residual_top_slice_mut(
		&mut self,
	) -> &mut RevSlice<Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		self.tranches.as_mut().rev_mut()
	}

	pub fn residual_top_slice(&self) -> &[Tranche<Balance, Rate, Weight, TrancheCurrency>] {
		self.tranches.deref().as_slice()
	}

	pub fn residual_top_slice_mut(
		&mut self,
	) -> &mut [Tranche<Balance, Rate, Weight, TrancheCurrency>] {
		self.tranches.as_mut()
	}

	/// Returns each tranche's total supply starting at the residual top.
	pub fn supplies(&self) -> Result<Vec<Balance>, DispatchError> {
		Ok(self
			.residual_top_slice()
			.iter()
			.map(|tranche| tranche.debt.ensure_add(tranche.reserve))
			.collect::<Result<_, _>>()?)
	}

	/// Returns the total supply over all tranches starting at the residual top.
	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		Ok(self
			.residual_top_slice()
			.iter()
			.try_fold(Balance::zero(), |sum, tranche| {
				sum.ensure_add(tranche.debt)?.ensure_add(tranche.reserve)
			})?)
	}

	/// Returns each tranche's min risk buffer starting at the residual top.
	pub fn min_risk_buffers(&self) -> Vec<Perquintill> {
		self.residual_top_slice()
			.iter()
			.map(|tranche| tranche.min_risk_buffer())
			.collect()
	}

	/// Returns each tranche's seniority starting at the residual top.
	pub fn seniorities(&self) -> Vec<Seniority> {
		self.residual_top_slice()
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
		let total_assets = pool_total_reserve.ensure_add(pool_nav)?;

		// Calculate the new total asset value for each tranche
		// This uses the current state of the tranches, rather than the cached
		// epoch-close-time values.
		let mut total_assets = total_assets;
		let tranche_assets = self.combine_with_mut_non_residual_top(
			executed_amounts.iter().rev(),
			|tranche, &(invest, redeem)| {
				tranche.accrue(now)?;

				let value = tranche
					.debt
					.ensure_add(tranche.reserve)?
					.ensure_add(invest)?
					.ensure_sub(redeem)?;

				if value > total_assets {
					let assets = total_assets;
					total_assets = Zero::zero();
					Ok(assets)
				} else {
					total_assets = total_assets.ensure_sub(value)?;
					Ok(value)
				}
			},
		)?;

		// Rebalance tranches based on the new tranche asset values and ratios
		let mut remaining_nav = pool_nav;
		let mut remaining_reserve = pool_total_reserve;
		self.combine_with_mut_non_residual_top(
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
					remaining_nav =
						remaining_nav
							.checked_sub(&tranche.debt)
							.ok_or(DispatchError::Other(
							"Corrupted pool-state. Pool NAV should be able to handle tranche debt substraction.",
						))?;
					remaining_reserve =
						remaining_reserve
							.checked_sub(&tranche.reserve)
							.ok_or(DispatchError::Other(
							"Corrupted pool-state. Pool reserve should be able to handle tranche reserve substraction.",
						))?;
				}
				Ok(())
			},
		)
		.map(|_| ())
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency> {
	pub currency: TrancheCurrency,
	pub supply: Balance,
	pub price: BalanceRatio,
	pub invest: Balance,
	pub redeem: Balance,
	pub min_risk_buffer: Perquintill,
	pub seniority: Seniority,

	pub _phantom: PhantomData<Weight>,
}

#[cfg(test)]
impl Default for EpochExecutionTranche<Balance, Rate, TrancheWeight, TrancheCurrency> {
	fn default() -> Self {
		Self {
			currency: TrancheCurrency::generate(0, [0u8; 16]),
			supply: 0,
			price: Rate::one(),
			invest: 0,
			redeem: 0,
			min_risk_buffer: Default::default(),
			seniority: 0,
			_phantom: Default::default(),
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency, MaxTranches>
where
	MaxTranches: Get<u32>,
{
	pub tranches: BoundedVec<
		EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
		MaxTranches,
	>,
}

/// Utility implementations for `EpochExecutionTranches`
impl<Balance, BalanceRatio, Weight, TrancheCurrency, MaxTranches>
	EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency, MaxTranches>
where
	Balance: Zero + Copy + BaseArithmetic + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	BalanceRatio: Copy,
	MaxTranches: Get<u32>,
{
	pub fn new(
		tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>>,
	) -> Self {
		Self {
			tranches: BoundedVec::truncate_from(tranches),
		}
	}

	pub fn non_residual_tranches(
		&self,
	) -> Option<&[EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>]> {
		self.tranches
			.deref()
			.as_slice()
			.split_first()
			.map(|(_head, tail)| tail)
			.filter(|tail| !tail.len().is_zero())
	}

	pub fn non_residual_tranches_mut(
		&mut self,
	) -> Option<&mut [EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>]> {
		self.tranches
			.get_mut(..)
			.and_then(|slice| slice.split_first_mut())
			.map(|(_head, tail)| tail)
			.filter(|tail| !tail.len().is_zero())
	}

	pub fn residual_tranche(
		&self,
	) -> Option<&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		self.tranches
			.deref()
			.as_slice()
			.split_first()
			.map(|(head, _tail)| head)
	}

	pub fn residual_tranche_mut(
		&mut self,
	) -> Option<&mut EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		self.tranches
			.get_mut(..)
			.and_then(|slice| slice.split_first_mut())
			.map(|(head, _tail)| head)
	}

	pub fn num_tranches(&self) -> usize {
		self.tranches.deref().len()
	}

	pub fn into_tranches(
		self,
	) -> BoundedVec<
		EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
		MaxTranches,
	> {
		self.tranches
	}

	pub fn non_residual_top_slice(
		&self,
	) -> &RevSlice<EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		self.tranches.deref().rev()
	}

	pub fn non_residual_top_slice_mut(
		&mut self,
	) -> &mut RevSlice<EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		self.tranches.as_mut().rev_mut()
	}

	pub fn residual_top_slice(
		&self,
	) -> &[EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>] {
		self.tranches.deref().as_slice()
	}

	pub fn residual_top_slice_mut(
		&mut self,
	) -> &mut [EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>] {
		self.tranches.as_mut()
	}

	pub fn combine_non_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		for tranche in self.non_residual_top_slice() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_non_residual_top<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		for tranche in self.non_residual_top_slice_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_non_residual_top<R, I, W, F>(
		&self,
		with: I,
		f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		combine_with(
			self.tranches.deref().len(),
			self.non_residual_top_slice().iter(),
			with,
			f,
		)
	}

	pub fn combine_with_mut_non_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		combine_with_mut(
			self.tranches.len(),
			self.non_residual_top_slice_mut().iter_mut(),
			with,
			f,
		)
	}

	pub fn combine_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		for tranche in self.residual_top_slice() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_mut_residual_top<R, F>(&mut self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.deref().len());
		for tranche in self.residual_top_slice_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_residual_top<R, I, W, F>(
		&self,
		with: I,
		f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		combine_with(
			self.tranches.deref().len(),
			self.residual_top_slice().iter(),
			with,
			f,
		)
	}

	pub fn combine_with_mut_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		combine_with_mut(
			self.tranches.len(),
			self.residual_top_slice_mut().iter_mut(),
			with,
			f,
		)
	}
}

/// Business logic implementations for `EpochExecutionTranches`
impl<Balance, BalanceRatio, Weight, TrancheCurrency, MaxTranches>
	EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency, MaxTranches>
where
	Balance: Zero + Copy + BaseArithmetic + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	BalanceRatio: Copy,
	MaxTranches: Get<u32>,
{
	pub fn prices(&self) -> Vec<BalanceRatio> {
		self.residual_top_slice()
			.iter()
			.map(|tranche| tranche.price)
			.collect()
	}

	pub fn fulfillment_cash_flows(
		&self,
		fulfillments: &[TrancheSolution],
	) -> Result<Vec<(Balance, Balance)>, DispatchError> {
		self.combine_with_residual_top(fulfillments, |tranche, solution| {
			Ok((
				solution.invest_fulfillment.mul_floor(tranche.invest),
				solution.redeem_fulfillment.mul_floor(tranche.redeem),
			))
		})
	}

	pub fn supplies_with_fulfillment(
		&self,
		fulfillments: &[TrancheSolution],
	) -> Result<Vec<Balance>, DispatchError> {
		self.combine_with_residual_top(fulfillments, |tranche, solution| {
			Ok(tranche
				.supply
				.ensure_add(solution.invest_fulfillment.mul_floor(tranche.invest))?
				.ensure_sub(solution.redeem_fulfillment.mul_floor(tranche.redeem))?)
		})
	}

	// Note: weight tuple contains (investment_weight, redemption weight)
	pub fn calculate_weights(&self) -> Vec<(Weight, Weight)> {
		let n_tranches: u32 = self.tranches.len().try_into().expect("MaxTranches is u32");
		let redeem_starts = 10u128.checked_pow(n_tranches).unwrap_or(u128::MAX);

		// The desired order priority is:
		// - Senior redemptions
		// - Junior redemptions
		// - Junior investments
		// - Senior investments
		// We ensure this by having a higher base weight for redemptions,
		// increasing the redemption weights by seniority,
		// and decreasing the investment weight by seniority.
		self.residual_top_slice()
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
						.checked_mul(
							10u128
								.checked_pow(tranche.seniority.saturating_add(1))
								.unwrap_or(u128::MAX),
						)
						.unwrap_or(u128::MAX)
						.into(),
				)
			})
			.collect()
	}

	pub fn min_risk_buffers(&self) -> Vec<Perquintill> {
		self.residual_top_slice()
			.iter()
			.map(|tranche| tranche.min_risk_buffer)
			.collect()
	}
}

pub fn calculate_risk_buffers<Balance, BalanceRatio>(
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
		.map(|(supply, price)| price.ensure_mul_int(*supply))
		.collect::<Result<_, _>>()?;

	let pool_value = tranche_values
		.iter()
		.try_fold(Balance::zero(), |sum, tranche_value| {
			sum.ensure_add(*tranche_value)
		})?;

	// Iterate over the tranches senior => junior.
	// Buffer of most senior tranche is pool value -y senior tranche value.
	// Buffer of each subordinate tranche is the buffer of the
	// previous more senior tranche - this tranche value.
	let mut remaining_subordinate_value = pool_value;
	let mut risk_buffers: Vec<Perquintill> = tranche_values
		.iter()
		.rev()
		.map(|tranche_value| {
			remaining_subordinate_value =
				remaining_subordinate_value.saturating_sub(*tranche_value);
			Perquintill::from_rational(remaining_subordinate_value, pool_value)
		})
		.collect::<Vec<Perquintill>>();

	risk_buffers.reverse();

	Ok(risk_buffers)
}

// TODO: Check whether these three helper functions should be moved inside a new
// trait. However, does not seem to make sense as we don't want to expose them.

/// Generic internal helper function for combining a tranches slice of either
/// [Tranches] or [EpochExecutionTranche] with a given iterator `with` under the
/// provided closure `f`.
///
/// Throws iff the sizes of the tranches slice and combining iterator mismatch.
fn combine_with<'t, R, T, IT, W, IW, F>(
	count: usize,
	mut tranche_slice: IT,
	with: IW,
	mut f: F,
) -> Result<Vec<R>, DispatchError>
where
	T: 't,
	F: FnMut(&'t T, W) -> Result<R, DispatchError>,
	IW: IntoIterator<Item = W>,
	IT: Iterator<Item = &'t T>,
{
	let mut res = Vec::with_capacity(count);
	let mut with_iter = with.into_iter();

	for _ in 0..count {
		match (tranche_slice.next(), with_iter.next()) {
			(Some(tranche), Some(w)) => res.push(f(tranche, w)?),
			_ => {
				return Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable",
				))
			}
		};
	}

	finalize_combine(res, tranche_slice.next(), with_iter.next())
}

/// Generic internal helper function for combining a mutable tranches slice of
/// either [Tranches] or [EpochExecutionTranche] with a given iterator `with`
/// under the provided closure `f`.
///
/// Throws iff the sizes of the tranches slice and combining iterator mismatch.
fn combine_with_mut<'t, R, T, IT, W, IW, F>(
	count: usize,
	mut tranche_slice: IT,
	with: IW,
	mut f: F,
) -> Result<Vec<R>, DispatchError>
where
	T: 't,
	F: FnMut(&'t mut T, W) -> Result<R, DispatchError>,
	IW: IntoIterator<Item = W>,
	IT: Iterator<Item = &'t mut T>,
{
	let mut res = Vec::with_capacity(count);
	let mut with_iter = with.into_iter();

	for _ in 0..count {
		match (tranche_slice.next(), with_iter.next()) {
			(Some(tranche), Some(w)) => res.push(f(tranche, w)?),
			_ => {
				return Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable",
				))
			}
		};
	}

	finalize_combine(res, tranche_slice.next(), with_iter.next())
}

/// Generic internal helper function for finalizing the combining of any mutable
/// tranches slice with a given iterator `with`.
///
/// Throws iff the combining iterator holds more elements than the tranche
/// slice. Else returns the provided resolution.
fn finalize_combine<R, T, W>(
	res: R,
	next_tranche: Option<T>,
	next_iter: Option<W>,
) -> Result<R, DispatchError> {
	match (next_tranche, next_iter) {
		(None, None) => Ok(res),
		_ => Err(DispatchError::Other(
			"Iterable contains more elements than Tranches slice",
		)),
	}
}

#[cfg(test)]
pub mod test {
	use cfg_primitives::{Balance, PoolId, TrancheId, TrancheWeight};
	use cfg_types::{
		fixed_point::{FixedPointNumberExtension, Rate},
		tokens::TrancheCurrency,
	};

	use super::*;
	use crate::mock::MaxTranches;

	type BalanceRatio = Rate;
	type TTrancheType = TrancheType<Rate>;
	type TTranche = Tranche<Balance, Rate, TrancheWeight, TrancheCurrency>;
	type TTranches =
		Tranches<Balance, Rate, TrancheWeight, TrancheCurrency, TrancheId, PoolId, MaxTranches>;

	const DEFAULT_POOL_ID: PoolId = 0;
	const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
	const DEBT_RES: u128 = 100_000_000;
	const DEBT_NONRES_1: u128 = 100_000_000;
	const DEBT_NONRES_2: u128 = 200_000_000;
	const RESERVE_RES: u128 = 100_000_000;
	const RESERVE_NONRES_1: u128 = 400_000_000;
	const RESERVE_NONRES_2: u128 = 100_000_000;
	const MAX_ROUNDING_PRECISION: u128 = 10u128.pow(6);

	#[derive(PartialEq)]
	struct TrancheWeights(Vec<(TrancheWeight, TrancheWeight)>);

	fn residual(id: u8) -> TTranche {
		residual_base(id, 0, 0, 0)
	}

	fn residual_base(id: u8, seniority: Seniority, debt: Balance, reserve: Balance) -> TTranche {
		TTranche {
			tranche_type: TrancheType::Residual,
			seniority: seniority,
			currency: TrancheCurrency::generate(DEFAULT_POOL_ID, [id; 16]),
			debt,
			reserve,
			loss: 0,
			ratio: Perquintill::zero(),
			last_updated_interest: 0,
			_phantom: PhantomData,
		}
	}
	fn non_residual(
		id: u8,
		interest_rate_in_perc: Option<u32>,
		buffer_in_perc: Option<u64>,
	) -> TTranche {
		non_residual_base(id, interest_rate_in_perc, buffer_in_perc, 0, 0, 0)
	}

	fn non_residual_base(
		id: u8,
		interest_rate_in_perc: Option<u32>,
		buffer_in_perc: Option<u64>,
		seniority: Seniority,
		debt: Balance,
		reserve: Balance,
	) -> TTranche {
		let interest_rate_per_sec = interest_rate_in_perc
			.map(|rate| Rate::saturating_from_rational(rate, 100))
			.unwrap_or(Rate::one())
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		let min_risk_buffer = buffer_in_perc
			.map(|buffer| Perquintill::from_rational(buffer, 100))
			.unwrap_or(Perquintill::zero());

		TTranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec,
				min_risk_buffer,
			},
			seniority: seniority,
			currency: TrancheCurrency::generate(DEFAULT_POOL_ID, [id; 16]),
			debt,
			reserve,
			loss: 0,
			ratio: Perquintill::zero(),
			last_updated_interest: 0,
			_phantom: PhantomData,
		}
	}

	/// Sets up three tranches: The default residual and two non residual ones
	/// with (10, 10, 0) and (5, 25, 0) for (interest_rate_per_sec,
	/// buffer_in_perc, seniority).
	fn default_tranches() -> TTranches {
		TTranches::new(
			DEFAULT_POOL_ID,
			vec![
				residual(0),
				non_residual(1, Some(10), Some(10)),
				non_residual(2, Some(5), Some(25)),
			],
		)
		.unwrap()
	}

	/// Sets up three tranches: The default residual and two non residual ones
	/// with (10, 10, 1) and (5, 25, 2) for (interest_rate_per_sec,
	/// buffer_in_perc, seniority).
	fn default_tranches_with_seniority() -> TTranches {
		TTranches::new(
			DEFAULT_POOL_ID,
			vec![
				residual_base(0, 0, 0, 0),
				non_residual_base(1, Some(10), Some(10), 1, 0, 0),
				non_residual_base(2, Some(5), Some(25), 2, 0, 0),
			],
		)
		.unwrap()
	}

	/// Sets up three tranches:
	///
	/// 	* Residual: 0% interest, 0% buffer, 100M debt, 100M reserve
	/// 	* Non Residual: 10% interest, 10% buffer, 100M debt, 400M reserve
	/// 	* Non Residual: 5% interest, 25% buffer, 200M debt, 100M reserve
	fn default_tranches_with_issuance() -> TTranches {
		TTranches::new(
			DEFAULT_POOL_ID,
			vec![
				residual_base(0, 0, DEBT_RES, RESERVE_RES),
				non_residual_base(1, Some(10), Some(10), 1, DEBT_NONRES_1, RESERVE_NONRES_1),
				non_residual_base(2, Some(5), Some(25), 2, DEBT_NONRES_2, RESERVE_NONRES_2),
			],
		)
		.unwrap()
	}

	fn tranche_to_epoch_execution_tranche(
		tranche: Tranche<Balance, Rate, TrancheWeight, TrancheCurrency>,
	) -> EpochExecutionTranche<Balance, BalanceRatio, TrancheWeight, TrancheCurrency> {
		EpochExecutionTranche {
			supply: tranche
				.reserve
				.checked_add(tranche.debt)
				.expect("Test EpochExecutionTranche supply calc overflow"),
			price: One::one(),
			invest: 0,
			redeem: 0,
			seniority: tranche.seniority,
			..Default::default()
		}
	}

	fn default_epoch_tranches(
	) -> EpochExecutionTranches<Balance, BalanceRatio, TrancheWeight, TrancheCurrency, MaxTranches>
	{
		let epoch_tranches = default_tranches_with_seniority()
			.into_tranches()
			.into_iter()
			.map(|tranche| tranche_to_epoch_execution_tranche(tranche))
			.collect();
		EpochExecutionTranches::new(epoch_tranches)
	}

	fn tranche_solution(i: Perquintill, r: Perquintill) -> TrancheSolution {
		TrancheSolution {
			invest_fulfillment: i,
			redeem_fulfillment: r,
		}
	}

	fn default_tranche_solution() -> TrancheSolution {
		tranche_solution(Perquintill::one(), Perquintill::one())
	}

	mod tranche_type {
		use super::*;

		#[test]
		fn tranche_type_valid_next_tranche_works() {
			let residual = TTrancheType::Residual;
			let non_residual_a = non_residual(1, Some(2), None).tranche_type;
			let non_residual_b = non_residual(2, Some(1), None).tranche_type;

			// Residual can not follow residual
			assert!(!residual.valid_next_tranche(&residual));

			// Residual can not follow non-residual
			assert!(!non_residual_a.valid_next_tranche(&residual));

			// Non-residual next must have smaller-equal interest rate
			assert!(!non_residual_b.valid_next_tranche(&non_residual_a));
			assert!(non_residual_a.valid_next_tranche(&non_residual_b));

			// Non-residual next must have greater-equal interest
			assert!(non_residual_b.valid_next_tranche(&non_residual_b));

			// Non-residual can follow residual
			assert!(residual.valid_next_tranche(&non_residual_b));
		}
	}

	mod tranche {

		use super::*;

		#[test]
		fn tranche_balance_is_debt_and_reserve() {
			let mut tranche = TTranche::default();
			tranche.debt = 100;
			tranche.reserve = 50;

			assert_eq!(150, tranche.balance().unwrap());
		}

		#[test]
		fn tranche_free_balance_is_reserve() {
			let mut tranche = TTranche::default();
			tranche.debt = 100;
			tranche.reserve = 50;

			assert_eq!(50, tranche.free_balance().unwrap());
		}

		#[test]
		fn tranche_accrues_correctly() {
			let mut tranche = non_residual(1, Some(10), None);
			tranche.debt = 100_000_000;
			tranche.accrue(SECS_PER_YEAR).unwrap();

			// After one year, we have 10% of interest, using APY and RPS compounding
			assert_eq!(110517092, tranche.debt)
		}

		#[test]
		fn tranche_returns_min_risk_correctly() {
			let tranche = non_residual(1, None, Some(20));
			assert_eq!(
				tranche.min_risk_buffer(),
				Perquintill::from_rational(20u64, 100u64),
			);
			assert!(residual(2).min_risk_buffer().is_zero());
		}

		#[test]
		fn tranche_returns_interest_rate_correctly() {
			let tranche = non_residual(1, Some(10), None);
			let interest_rate_per_sec = Rate::saturating_from_rational(10, 100)
				/ Rate::saturating_from_integer(SECS_PER_YEAR)
				+ One::one();
			assert_eq!(interest_rate_per_sec, tranche.interest_rate_per_sec())
		}

		#[test]
		fn tranche_accrues_debt_on_debt_call() {
			let mut tranche = non_residual(1, Some(10), None);
			tranche.debt = 100_000_000;

			// After one year, we have 10% of interest, using APY and RPS compounding
			assert_eq!(110517092, tranche.debt(SECS_PER_YEAR).unwrap())
		}

		#[test]
		#[should_panic]
		fn tranches_reverse_slice_panics_on_out_of_bounds() {
			let tranches = default_tranches();
			let slice_rev = tranches.non_residual_top_slice();
			let _panic = &slice_rev[3];
		}

		#[test]
		fn tranches_reverse_works() {
			let tranches = default_tranches();
			let slice_rev = tranches.non_residual_top_slice();
			assert_eq!(
				slice_rev[0].tranche_type,
				non_residual(2, Some(5), Some(25)).tranche_type
			);
			assert_eq!(
				slice_rev[1].tranche_type,
				non_residual(1, Some(10), Some(10)).tranche_type
			);
			assert_eq!(slice_rev[2].tranche_type, residual(0).tranche_type);
		}

		#[test]
		fn tranches_accrue_overflows_safely() {
			let mut tranche = non_residual(3, Some(10), None);

			tranche.debt = Balance::max_value() - 10;

			assert_eq!(
				tranche.accrue(SECS_PER_YEAR),
				Err(ArithmeticError::Overflow.into())
			)
		}

		#[test]
		fn create_asset_metadata_works() {
			let tranche = non_residual(3, Some(10), None);
			let decimals: u32 = 10;
			let name: Vec<u8> = "Glimmer".into();
			let symbol: Vec<u8> = "GLMR".into();
			let asset_metadata = tranche.create_asset_metadata(decimals, name, symbol);

			assert_eq!(asset_metadata.existential_deposit, 0);
			assert_eq!(asset_metadata.name[..], [71, 108, 105, 109, 109, 101, 114]);
			assert_eq!(asset_metadata.symbol[..], [71, 76, 77, 82]);
			assert_eq!(asset_metadata.decimals, decimals);
			assert_eq!(asset_metadata.location, None);
		}
	}

	mod tranches {
		use frame_support::assert_ok;

		use super::*;

		#[test]
		fn tranche_currency_works() {
			let tranches = default_tranches();
			assert_eq!(
				tranches.tranche_currency(TrancheLoc::Index(0)),
				Some(TrancheCurrency::generate(DEFAULT_POOL_ID, [0u8; 16]))
			);
			assert_eq!(
				tranches.tranche_currency(TrancheLoc::Index(1)),
				Some(TrancheCurrency::generate(DEFAULT_POOL_ID, [1u8; 16]))
			);
			assert_eq!(
				tranches.tranche_currency(TrancheLoc::Index(2)),
				Some(TrancheCurrency::generate(DEFAULT_POOL_ID, [2u8; 16]))
			);
			assert_eq!(tranches.tranche_currency(TrancheLoc::Index(3)), None);
		}

		#[test]
		fn tranche_id_from_tranches() {
			let tranches = default_tranches();

			// valid tranche id
			let expected_tranche_id: TrancheId = [
				59u8, 168, 10, 55, 120, 240, 78, 191, 69, 232, 6, 209, 154, 5, 32, 37,
			];

			// get by id with valid id
			assert_eq!(
				tranches.tranche_id(TrancheLoc::Id(expected_tranche_id)),
				Some(expected_tranche_id)
			);

			let invalid_tranche_id: TrancheId = [
				59u8, 0, 10, 0, 120, 240, 10, 191, 69, 232, 6, 10, 154, 5, 32, 37,
			];
			// get by id for nonexistent tranche
			assert_eq!(
				tranches.tranche_id(TrancheLoc::Id(invalid_tranche_id)),
				None
			);

			// get by index with valid index
			assert_eq!(
				tranches.tranche_id(TrancheLoc::Index(1)),
				Some(expected_tranche_id)
			);

			// get by invalid index
			assert_eq!(tranches.tranche_id(TrancheLoc::Index(3)), None)
		}

		#[test]
		fn tranche_index_from_tranches() {
			let tranches = default_tranches();

			let valid_tranche_id: TrancheId = [
				59u8, 168, 10, 55, 120, 240, 78, 191, 69, 232, 6, 209, 154, 5, 32, 37,
			];

			assert_eq!(
				tranches.tranche_index(&TrancheLoc::Id(valid_tranche_id)),
				Some(1)
			);

			let invalid_tranche_id: TrancheId = [
				59u8, 0, 10, 0, 120, 240, 10, 191, 69, 232, 6, 10, 154, 5, 32, 37,
			];

			assert_eq!(
				tranches.tranche_index(&TrancheLoc::Id(invalid_tranche_id)),
				None
			);

			assert_eq!(tranches.tranche_index(&TrancheLoc::Index(2)), Some(2));
			assert_eq!(tranches.tranche_index(&TrancheLoc::Index(3)), None);
		}

		#[test]
		fn get_mut_tranche_works() {
			let mut tranches = default_tranches_with_seniority();

			let mut tranche = tranches.get_mut_tranche(TrancheLoc::Index(2)).unwrap();
			tranche.debt = 25000;
			// ensure both correct tranche fetched, and tranche mutable
			assert_eq!((tranche.debt, tranche.seniority), (25000, 2));
			assert_eq!(tranches.get_mut_tranche(TrancheLoc::Index(3)), None);

			// test with id as opposed to index using ID of tranche at index 1
			let valid_tranche_id: TrancheId = [
				59u8, 168, 10, 55, 120, 240, 78, 191, 69, 232, 6, 209, 154, 5, 32, 37,
			];
			let mut tranche = tranches
				.get_mut_tranche(TrancheLoc::Id(valid_tranche_id))
				.unwrap();

			tranche.debt = 25001;
			// ensure both correct tranche fetched, and tranche mutable
			assert_eq!((tranche.debt, tranche.seniority), (25001, 1));

			let invalid_tranche_id: TrancheId = [
				59u8, 168, 10, 10, 10, 10, 10, 191, 69, 232, 6, 209, 154, 5, 32, 37,
			];
			assert_eq!(
				tranches.get_mut_tranche(TrancheLoc::Id(invalid_tranche_id)),
				None
			);
		}

		#[test]
		fn get_tranche_works() {
			let tranches = default_tranches_with_seniority();

			let tranche = tranches.get_tranche(TrancheLoc::Index(2)).unwrap();
			assert_eq!(tranche.seniority, 2);
			assert_eq!(tranches.get_tranche(TrancheLoc::Index(3)), None);

			// id for tranche at index 1, w/ seniority 1
			let valid_tranche_id: TrancheId = [
				59u8, 168, 10, 55, 120, 240, 78, 191, 69, 232, 6, 209, 154, 5, 32, 37,
			];
			let tranche = tranches
				.get_tranche(TrancheLoc::Id(valid_tranche_id))
				.unwrap();
			assert_eq!(tranche.seniority, 1);

			let invalid_tranche_id: TrancheId = [
				59u8, 168, 10, 10, 10, 10, 10, 191, 69, 232, 6, 209, 154, 5, 32, 37,
			];
			assert_eq!(
				tranches.get_tranche(TrancheLoc::Id(invalid_tranche_id)),
				None
			)
		}

		#[test]
		fn create_tranche_works() {
			let mut tranches = default_tranches_with_seniority();
			let tranche_id: TrancheId = [
				103u8, 57, 22, 242, 127, 45, 18, 102, 173, 154, 105, 163, 156, 150, 75, 194,
			];
			let int_per_sec = Rate::saturating_from_integer(SECS_PER_YEAR);
			let min_risk_buffer = Perquintill::from_rational(4u64, 5);

			// Create tranche with explicit seniority
			let new_tranche = tranches
				.create_tranche(
					3,
					tranche_id,
					TrancheType::NonResidual {
						interest_rate_per_sec: int_per_sec,
						min_risk_buffer: min_risk_buffer,
					},
					Some(5u32),
					// arbitrary static time val for "now"
					SECS_PER_YEAR,
				)
				.unwrap();

			assert!(match new_tranche {
				Tranche {
					tranche_type:
						TrancheType::NonResidual {
							interest_rate_per_sec: ir,
							min_risk_buffer: b,
						},
					debt: 0,
					reserve: 0,
					loss: 0,
					seniority: 5,
					currency: TrancheCurrency { .. },
					last_updated_interest: SECS_PER_YEAR,
					..
				} if b == min_risk_buffer && int_per_sec == ir => true,
				_ => false,
			});
			assert_eq!(
				new_tranche.currency,
				TrancheCurrency::generate(DEFAULT_POOL_ID, tranche_id)
			);
			assert_eq!(new_tranche.ratio, Perquintill::zero());

			// Create tranche with implicit seniority (through index)
			let new_tranche = tranches
				.create_tranche(
					3,
					tranche_id,
					TrancheType::NonResidual {
						interest_rate_per_sec: int_per_sec,
						min_risk_buffer: min_risk_buffer,
					},
					// By not providing seniority, it is derived from the index
					None,
					SECS_PER_YEAR,
				)
				.unwrap();

			assert_eq!(new_tranche.seniority, 3);

			// Create tranche with overflowing index
			let new_tranche = tranches.create_tranche(
				u64::MAX,
				tranche_id,
				TrancheType::NonResidual {
					interest_rate_per_sec: int_per_sec,
					min_risk_buffer: min_risk_buffer,
				},
				None,
				SECS_PER_YEAR,
			);
			assert_eq!(
				new_tranche,
				Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
			);
		}

		#[test]
		fn next_id_works() {
			let mut tranches = default_tranches();
			let next = tranches.next_id();
			let next_again = tranches.next_id();

			assert_ne!(next, next_again)
		}

		struct TokenNameLen;
		impl Get<u32> for TokenNameLen {
			fn get() -> u32 {
				16u32
			}
		}

		struct TokenSymLen;
		impl Get<u32> for TokenSymLen {
			fn get() -> u32 {
				8u32
			}
		}

		// Replace should work if interest is lower than tranche w/ lower index
		// ("next").
		#[test]
		fn replace_tranche_less_interest_than_next_works() {
			let mut tranches = default_tranches();

			// ensure we have an interest rate lower than the the left side tranche with a
			// lower index, e.g. lower than 10% at index 1
			let int_per_sec = Rate::one() / Rate::saturating_from_integer(SECS_PER_YEAR);
			let min_risk_buffer = Perquintill::from_rational(4u64, 5);
			let seniority = Some(5);
			let tranche_type = TrancheType::NonResidual {
				interest_rate_per_sec: int_per_sec,
				min_risk_buffer: min_risk_buffer,
			};
			let input = TrancheInput {
				// setting to easily testable value for tranche replacement
				seniority,
				tranche_type,
				metadata: TrancheMetadata {
					token_name: BoundedVec::<u8, TokenNameLen>::default(),
					token_symbol: BoundedVec::<u8, TokenSymLen>::default(),
				},
			};

			// verify replace tranche works with interest less than prev tranche as expected
			// replacing last tranche
			assert_ok!(tranches.replace(2, input, SECS_PER_YEAR));
			assert!(tranches
				.get_tranche(TrancheLoc::Index(2))
				.map(|tranche| {
					tranche.seniority == seniority.unwrap() && tranche.tranche_type == tranche_type
				})
				.unwrap());
			let removed_tranche = &default_tranches().tranches[2];
			assert!(tranches
				.tranches
				.iter()
				.find(|tranche| tranche == &removed_tranche)
				.is_none());
		}

		// Replace must not work if new interest rate is greater than tranche w/ lower
		// index ("next").
		#[test]
		fn replace_tranche_more_interest_than_next_throws() {
			let mut tranches = default_tranches();
			let min_risk_buffer = Perquintill::from_rational(4u64, 5);

			// ensure we have an interest rate larger than the a tranche with a lower index,
			// e.g. 10%
			let int_per_sec = Rate::saturating_from_rational(11, 100)
				/ Rate::saturating_from_integer(SECS_PER_YEAR)
				+ One::one();
			let input = TrancheInput {
				// setting to easily testable value for tranche replacement, should not be changed
				// from 0
				seniority: Some(5),
				tranche_type: TrancheType::NonResidual {
					interest_rate_per_sec: int_per_sec,
					min_risk_buffer: min_risk_buffer,
				},
				metadata: TrancheMetadata {
					token_name: BoundedVec::<u8, TokenNameLen>::default(),
					token_symbol: BoundedVec::<u8, TokenSymLen>::default(),
				},
			};

			let replace_res = tranches.replace(2, input, SECS_PER_YEAR);
			assert_eq!(
				replace_res,
				Err(DispatchError::Other(
					"Invalid next tranche type. This should be catched somewhere else."
				))
			);
			// verify unchanged from default val
			assert_eq!(tranches.tranches[2], default_tranches().tranches[2]);
		}

		// Replace should work if new interest rate is greater than tranche w/ higher
		// index ("following").
		#[test]
		fn replace_tranche_more_interest_than_following_works() {
			let mut tranches = default_tranches();

			let min_risk_buffer = Perquintill::from_rational(4u64, 5);
			// ensure we have an interest rate larger than the the right-side tranche with a
			// greater index, e.g. larger than 5% at index 2
			let int_per_sec = Rate::saturating_from_rational(6u64, 100)
				/ Rate::saturating_from_integer(SECS_PER_YEAR)
				+ One::one();
			let seniority = Some(5);
			let tranche_type = TrancheType::NonResidual {
				interest_rate_per_sec: int_per_sec,
				min_risk_buffer: min_risk_buffer,
			};
			let input = TrancheInput {
				seniority,
				tranche_type,
				metadata: TrancheMetadata {
					token_name: BoundedVec::<u8, TokenNameLen>::default(),
					token_symbol: BoundedVec::<u8, TokenSymLen>::default(),
				},
			};

			assert_ok!(tranches.replace(1, input, SECS_PER_YEAR));
			assert!(tranches
				.get_tranche(TrancheLoc::Index(1))
				.map(|tranche| {
					tranche.seniority == seniority.unwrap() && tranche.tranche_type == tranche_type
				})
				.unwrap());
			let removed_tranche = &default_tranches().tranches[1];
			assert!(tranches
				.tranches
				.iter()
				.find(|tranche| tranche == &removed_tranche)
				.is_none());
		}

		// Replace must not work if new interest rate is lower than tranche w/ greater
		// index ("following").
		#[test]
		fn replace_tranche_less_interest_than_following_throws() {
			let mut tranches = default_tranches();
			let min_risk_buffer = Perquintill::from_rational(4u64, 5);

			// ensure we have an interest rate lower than a tranche with a higher index,
			// e.g. 5%
			let int_per_sec = Rate::saturating_from_rational(4, 100)
				/ Rate::saturating_from_integer(SECS_PER_YEAR)
				+ One::one();
			let input = TrancheInput {
				seniority: Some(5),
				tranche_type: TrancheType::NonResidual {
					interest_rate_per_sec: int_per_sec,
					min_risk_buffer: min_risk_buffer,
				},
				metadata: TrancheMetadata {
					token_name: BoundedVec::<u8, TokenNameLen>::default(),
					token_symbol: BoundedVec::<u8, TokenSymLen>::default(),
				},
			};

			assert_eq!(
				tranches.replace(1, input, SECS_PER_YEAR),
				Err(DispatchError::Other(
					"Invalid following tranche type. This should be catched somewhere else."
				))
			);
		}

		#[test]
		fn validate_insert_works() {
			let mut tranches = default_tranches();

			let tranche_id: TrancheId = [
				103u8, 57, 22, 242, 127, 45, 18, 102, 173, 154, 105, 163, 156, 150, 75, 194,
			];
			let min_risk_buffer = Perquintill::from_rational(4u64, 5);
			let int_per_sec = Rate::saturating_from_integer(2)
				/ Rate::saturating_from_integer(SECS_PER_YEAR)
				+ One::one();

			// verify returns valid when interest greater than tranche following new tranche
			let new_tranche = tranches
				.create_tranche(
					3,
					tranche_id,
					TrancheType::NonResidual {
						interest_rate_per_sec: int_per_sec,
						min_risk_buffer: min_risk_buffer,
					},
					Some(5u32),
					SECS_PER_YEAR,
				)
				.unwrap();

			assert!(tranches.validate_insert(1, &new_tranche).is_ok());
			// verify error when tranche new_tranche is following  has lower interest
			assert!(tranches.validate_insert(1, &new_tranche).is_ok());

			// verify error returned when interest less than tranche following new tranche
			let int_per_sec = Rate::saturating_from_rational(1, 100)
				/ Rate::saturating_from_integer(SECS_PER_YEAR)
				+ One::one();
			let new_tranche = tranches
				.create_tranche(
					3,
					tranche_id,
					TrancheType::NonResidual {
						interest_rate_per_sec: int_per_sec,
						min_risk_buffer: min_risk_buffer,
					},
					Some(5u32),
					SECS_PER_YEAR,
				)
				.unwrap();

			assert!(tranches.validate_insert(1, &new_tranche).is_err());

			// verify ok when tranche new_tranche is following has higher interest rate
			assert!(tranches.validate_insert(2, &new_tranche).is_ok())
		}

		#[test]
		fn remove_tranches_happy_path_works() {
			let mut tranches = default_tranches();
			let tranches_pre_removal = default_tranches();

			// remove middle tranche
			assert_ok!(tranches.remove(1));
			assert_eq!(tranches.num_tranches(), 2);
			assert!(tranches.get_tranche(TrancheLoc::Index(2)).is_none());
			assert_eq!(
				tranches_pre_removal
					.get_tranche(TrancheLoc::Index(2))
					.unwrap(),
				tranches.get_tranche(TrancheLoc::Index(1)).unwrap()
			);

			// remove last remaining non-res tranche
			assert_ok!(tranches.remove(1));
			assert_eq!(tranches.non_residual_tranches(), None);
			assert_eq!(tranches.num_tranches(), 1);
			assert_eq!(
				tranches_pre_removal
					.get_tranche(TrancheLoc::Index(0))
					.unwrap(),
				tranches.get_tranche(TrancheLoc::Index(0)).unwrap()
			);
		}

		#[test]
		fn remove_tranches_throws() {
			let mut tranches = default_tranches_with_issuance();

			// attempt to remove outside of bounds
			assert_eq!(
				tranches.remove(3),
				Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
			);

			assert_eq!(
				tranches.remove(0),
				Err(DispatchError::Other("Must not remove residual tranche."))
			);

			assert_eq!(
				tranches.remove(1),
				Err(DispatchError::Other(
					"Must not remove non-residual tranche with non-zero balance."
				))
			);
			assert_eq!(
				tranches.remove(2),
				Err(DispatchError::Other(
					"Must not remove non-residual tranche with non-zero balance."
				))
			);
		}

		#[test]
		fn of_pool_tranches_works() {
			let mut tranches = default_tranches();
			assert_eq!(tranches.of_pool(), DEFAULT_POOL_ID);

			let new_pool_id = 42;
			tranches.salt = (24, 42);
			assert_eq!(tranches.of_pool(), new_pool_id);
		}

		#[test]
		fn combine_with_non_residual_top_tranches_works() {
			let mut i: Seniority = 0;
			assert_eq!(
				default_tranches()
					.combine_with_non_residual_top(&[220, 210, 250], |tranche, other_val| {
						i += 1;
						Ok((tranche.seniority + i, *other_val))
					})
					.unwrap(),
				[(1, 220), (2, 210), (3, 250)]
			);

			// tranches has smaller size than combinator
			assert_eq!(
				default_tranches()
					.combine_with_non_residual_top(&[220, 210, 250, 110], |tranche, other_val| {
						Ok((tranche.seniority, *other_val))
					}),
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice",
				))
			);

			// tranches has greater size than combinator
			assert_eq!(
				default_tranches()
					.combine_with_non_residual_top(&[220, 210], |tranche, other_val| {
						Ok((tranche.seniority, *other_val))
					}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);

			// combinator is empty
			assert_eq!(
				default_tranches().combine_with_non_residual_top(vec![], |tranche, _: u32| {
					Ok(tranche.seniority)
				}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);
		}

		#[test]
		fn combine_with_mut_non_residual_top_tranches_works() {
			let mut tranches = default_tranches();
			let values = [10, 20, 30];
			assert_eq!(
				tranches.combine_with_mut_non_residual_top(&values, |tranche, new_seniority| {
					tranche.seniority = *new_seniority;
					Ok(tranche.seniority)
				}),
				Ok(values.into())
			);

			// tranches has smaller size than combinator
			let values_too_many = [10, 20, 30, 40];
			assert_eq!(
				tranches
					.combine_with_mut_non_residual_top(&values_too_many, |tranche, other_val| {
						Ok(tranche.seniority + other_val)
					}),
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice",
				))
			);

			// tranches has greater size than combinator
			let mut tranches = default_tranches();
			let values_too_few = [10, 20];
			assert_eq!(
				tranches
					.combine_with_mut_non_residual_top(&values_too_few, |tranche, other_val| {
						Ok(tranche.seniority + other_val)
					}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);

			// combinator is empty
			assert_eq!(
				tranches.combine_with_mut_non_residual_top(vec![], |tranche, _: u32| {
					Ok(tranche.seniority)
				}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);
		}

		#[test]
		fn ids_residual_top_tranches_works() {
			let tranches = default_tranches();
			assert_eq!(tranches.ids, tranches.ids_residual_top());

			let rev_ids: Vec<TrancheId> = tranches.ids.clone().into_iter().rev().collect();
			assert_eq!(rev_ids, tranches.ids_non_residual_top());
		}

		#[test]
		fn combine_residual_top_tranches_works() {
			let mut i: Seniority = 0;
			assert_eq!(
				default_tranches()
					.combine_with_residual_top(&[220, 210, 250], |tranche, other_val| {
						i += 1;
						Ok((tranche.seniority + i, *other_val))
					})
					.unwrap(),
				[(1, 220), (2, 210), (3, 250)]
			);

			// tranches has smaller size than combinator
			assert_eq!(
				default_tranches()
					.combine_with_residual_top(&[220, 210, 250, 110], |tranche, other_val| {
						Ok((tranche.seniority, *other_val))
					}),
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice",
				))
			);

			// tranches has greater size than combinator
			assert_eq!(
				default_tranches().combine_with_residual_top(&[220, 210], |tranche, other_val| {
					Ok((tranche.seniority, *other_val))
				}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);

			// combinator is empty
			assert_eq!(
				default_tranches()
					.combine_with_residual_top(vec![], |tranche, _: u32| { Ok(tranche.seniority) }),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);
		}

		#[test]
		fn combine_with_mut_residual_top_tranches_works() {
			let mut tranches = default_tranches();
			let values = [10, 20, 30];
			assert_eq!(
				tranches.combine_with_mut_residual_top(&values, |tranche, new_seniority| {
					tranche.seniority = *new_seniority;
					Ok(tranche.seniority)
				}),
				Ok(values.into())
			);

			// tranches has smaller size than combinator
			let values_too_many = [10, 20, 30, 40];
			assert_eq!(
				tranches.combine_with_mut_residual_top(&values_too_many, |tranche, other_val| {
					Ok(tranche.seniority + other_val)
				}),
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice",
				))
			);

			// tranches has greater size than combinator
			let mut tranches = default_tranches();
			let values_too_few = [10, 20];
			assert_eq!(
				tranches.combine_with_mut_residual_top(&values_too_few, |tranche, other_val| {
					Ok(tranche.seniority + other_val)
				}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);

			// combinator is empty
			assert_eq!(
				tranches.combine_with_mut_residual_top(vec![], |tranche, _: u32| {
					Ok(tranche.seniority)
				}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);
		}

		mod calculate_prices {
			use super::*;

			/// Implements only `total_issuance` required for
			/// `calculate_prices`.
			struct TTokens(u64);
			impl Inspect<TrancheCurrency> for TTokens {
				type AssetId = TrancheCurrency;
				type Balance = u128;

				/// Mock value is sum of asset.pool_id and 100_000_0000.
				fn total_issuance(asset: Self::AssetId) -> Self::Balance {
					match asset.of_tranche() {
						// default most senior tranche currency id
						[2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2] => {
							DEBT_NONRES_2 + RESERVE_NONRES_2
						}
						// default least senior tranche currency id
						[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1] => {
							DEBT_NONRES_1 + RESERVE_NONRES_1
						}
						// default single residual tranche currency id
						[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] => DEBT_RES + RESERVE_RES,
						_ => 100_000_0000,
					}
				}

				fn minimum_balance(_asset: Self::AssetId) -> Self::Balance {
					todo!()
				}

				fn balance(_asset: Self::AssetId, _who: &TrancheCurrency) -> Self::Balance {
					todo!()
				}

				fn reducible_balance(
					_asset: Self::AssetId,
					_who: &TrancheCurrency,
					_keep_alive: bool,
				) -> Self::Balance {
					todo!()
				}

				fn can_deposit(
					_asset: Self::AssetId,
					_who: &TrancheCurrency,
					_amount: Self::Balance,
					_mint: bool,
				) -> frame_support::traits::tokens::DepositConsequence {
					todo!()
				}

				fn can_withdraw(
					_asset: Self::AssetId,
					_who: &TrancheCurrency,
					_amount: Self::Balance,
				) -> frame_support::traits::tokens::WithdrawConsequence<Self::Balance> {
					todo!()
				}

				fn asset_exists(_asset: Self::AssetId) -> bool {
					todo!()
				}
			}

			// No debt, reserve or APR for any tranche.
			#[test]
			fn no_debt_works() {
				let initial_assets = DEBT_RES + RESERVE_RES;

				// only residual has a price if there is no debt
				assert_eq!(
					default_tranches().calculate_prices::<_, TTokens, TrancheCurrency>(
						initial_assets,
						SECS_PER_YEAR
					),
					Ok(vec![Rate::one(), Rate::zero(), Rate::zero(),])
				);
				// price should be the same for longer time period as NAV does not change
				assert_eq!(
					default_tranches().calculate_prices::<_, TTokens, TrancheCurrency>(
						initial_assets,
						2 * SECS_PER_YEAR
					),
					Ok(vec![Rate::one(), Rate::zero(), Rate::zero(),])
				);

				// price should double if initial assets doubles
				assert_eq!(
					default_tranches().calculate_prices::<_, TTokens, TrancheCurrency>(
						initial_assets * 2,
						SECS_PER_YEAR
					),
					Ok(vec![
						Rate::saturating_from_rational(2, 1),
						Rate::zero(),
						Rate::zero(),
					])
				);
				// price should be half if initial asset amount is halfed
				assert_eq!(
					default_tranches().calculate_prices::<_, TTokens, TrancheCurrency>(
						initial_assets / 2,
						2 * SECS_PER_YEAR
					),
					Ok(vec![
						Rate::saturating_from_rational(1, 2),
						Rate::zero(),
						Rate::zero(),
					])
				);
			}

			// If amount of assets is zero, all price rates should be one.
			#[test]
			fn no_assets_works() {
				assert_eq!(
					default_tranches()
						.calculate_prices::<_, TTokens, TrancheCurrency>(0, SECS_PER_YEAR),
					Ok(vec![Rate::zero(), Rate::zero(), Rate::zero(),])
				);
			}

			#[test]
			fn no_issuance_works() {
				struct TTokensEmpty(u64);
				impl Inspect<TrancheCurrency> for TTokensEmpty {
					type AssetId = TrancheCurrency;
					type Balance = u128;

					fn total_issuance(_asset: Self::AssetId) -> Self::Balance {
						Self::Balance::zero()
					}

					fn minimum_balance(_asset: Self::AssetId) -> Self::Balance {
						todo!()
					}

					fn balance(_asset: Self::AssetId, _who: &TrancheCurrency) -> Self::Balance {
						todo!()
					}

					fn reducible_balance(
						_asset: Self::AssetId,
						_who: &TrancheCurrency,
						_keep_alive: bool,
					) -> Self::Balance {
						todo!()
					}

					fn can_deposit(
						_asset: Self::AssetId,
						_who: &TrancheCurrency,
						_amount: Self::Balance,
						_mint: bool,
					) -> frame_support::traits::tokens::DepositConsequence {
						todo!()
					}

					fn can_withdraw(
						_asset: Self::AssetId,
						_who: &TrancheCurrency,
						_amount: Self::Balance,
					) -> frame_support::traits::tokens::WithdrawConsequence<Self::Balance> {
						todo!()
					}

					fn asset_exists(_asset: Self::AssetId) -> bool {
						todo!()
					}
				}

				assert_eq!(
					default_tranches().calculate_prices::<_, TTokensEmpty, TrancheCurrency>(
						10u128.pow(10),
						SECS_PER_YEAR
					),
					Ok(vec![Rate::one(), Rate::one(), Rate::one(),])
				);
			}

			// Check price loss waterfall for different asset amounts.
			//
			// Each tranche has a different APR, debt, reserve and total issuance.
			// The sum of total issuance (initial NAV) for all three tranches is 1000.
			//
			// NOTE: Expected values checked against in https://docs.google.com/spreadsheets/d/16hpWBzGFxlhsIFYJYl1Im9BsNLKVjvJj8VUvECxqduE/edit#gid=543118716
			#[test]
			fn total_assets_works() {
				assert_eq!(
					default_tranches_with_issuance()
						.calculate_prices::<_, TTokens, TrancheCurrency>(
							1_100_000_000,
							SECS_PER_YEAR
						),
					Ok(vec![
						Rate::saturating_from_rational(1396143445, 1000000000),
						Rate::saturating_from_rational(1021034184, 1000000000),
						Rate::saturating_from_rational(103418073, 100000000),
					])
				);
				// reduce new NAV/total_assets by 200_000_000 to have loss in residual tranche
				assert_eq!(
					default_tranches_with_issuance()
						.calculate_prices::<_, TTokens, TrancheCurrency>(
							900_000_000,
							SECS_PER_YEAR
						),
					Ok(vec![
						Rate::saturating_from_rational(396143445u64, 1000000000u64),
						Rate::saturating_from_rational(1021034184, 1000000000),
						Rate::saturating_from_rational(103418073, 100000000),
					])
				);
				// reduce new NAV/total_assets by another 200 to have loss in first non-res
				// tranche
				assert_eq!(
					default_tranches_with_issuance()
						.calculate_prices::<_, TTokens, TrancheCurrency>(
							700_000_000,
							SECS_PER_YEAR
						),
					Ok(vec![
						Rate::zero(),
						Rate::saturating_from_rational(779491562, 1000000000),
						Rate::saturating_from_rational(103418073, 100000000),
					])
				);
				// reduce new NAV/total_assets by another 500 to have loss most senior tranche
				assert_eq!(
					default_tranches_with_issuance()
						.calculate_prices::<_, TTokens, TrancheCurrency>(
							100_000_000,
							SECS_PER_YEAR
						),
					Ok(vec![
						Rate::zero(),
						Rate::zero(),
						Rate::saturating_from_rational(1, 3),
					])
				);
			}

			// Check price evolution over course of multiple years without adjusting total
			// assets.
			//
			// Each tranche has a different APR, debt, reserve and total issuance.
			// The sum of total issuance (initial NAV) for all three tranches is 1000.
			#[test]
			fn last_update_works() {
				let mut tranches = default_tranches_with_issuance();
				assert_eq!(
					tranches.calculate_prices::<_, TTokens, TrancheCurrency>(
						1_100_000_000,
						SECS_PER_YEAR
					),
					Ok(vec![
						Rate::saturating_from_rational(1396143445, 1000000000),
						Rate::saturating_from_rational(1021034184, 1000000000),
						Rate::saturating_from_rational(103418073, 100000000),
					])
				);
				// increase time since last update by two to reduce res price
				assert_eq!(
					tranches.calculate_prices::<_, TTokens, TrancheCurrency>(
						1_100_000_000,
						2 * SECS_PER_YEAR
					),
					Ok(vec![
						Rate::saturating_from_rational(1284127705, 1000000000),
						Rate::saturating_from_rational(1044280552, 1000000000),
						Rate::saturating_from_rational(
							1070113943333333333333333333u128,
							1000000000000000000000000000u128
						),
					])
				);
				// increase time since last update by ten to reduce res and first non-res prices
				assert_eq!(
					tranches.calculate_prices::<_, TTokens, TrancheCurrency>(
						1_100_000_000,
						5 * SECS_PER_YEAR
					),
					Ok(vec![
						Rate::saturating_from_rational(89161395, 100000000),
						Rate::saturating_from_rational(1129744254, 1000000000),
						Rate::saturating_from_rational(
							1189350276666666666666666667u128,
							1000000000000000000000000000u128
						),
					])
				);
				// increase time since last update by twenty to reduce
				assert_eq!(
					tranches.calculate_prices::<_, TTokens, TrancheCurrency>(
						1_100_000_000,
						20 * SECS_PER_YEAR
					),
					Ok(vec![
						Rate::zero(),
						Rate::saturating_from_rational(91268727, 100000000),
						Rate::saturating_from_rational(
							2145521216666666666666666667u128,
							1000000000000000000000000000u128
						),
					])
				);
			}

			// The maximum precision before rounding errors is 10e-6% which should be fine
			#[test]
			fn rounding_works() {
				let mut tranches = default_tranches_with_issuance();
				assert_ok!(tranches.calculate_prices::<Rate, TTokens, TrancheCurrency>(
					1_100_000_000,
					SECS_PER_YEAR
				));

				for i in 2..200 {
					assert_eq!(
						tranches
							.calculate_prices::<Rate, TTokens, TrancheCurrency>(
								1_100_000_000,
								i * SECS_PER_YEAR
							)
							.unwrap()
							.into_iter()
							.map(|ratio| ratio.checked_mul_int_floor(MAX_ROUNDING_PRECISION))
							.collect::<Vec<Option<u128>>>(),
						default_tranches_with_issuance()
							.calculate_prices::<Rate, TTokens, TrancheCurrency>(
								1_100_000_000,
								i * SECS_PER_YEAR
							)
							.unwrap()
							.into_iter()
							.map(|ratio| ratio.checked_mul_int_floor(MAX_ROUNDING_PRECISION))
							.collect::<Vec<Option<u128>>>(),
						"rounding error after {} years",
						i
					);
				}
			}

			#[test]
			fn same_moment_works() {
				let mut tranches = default_tranches_with_issuance();
				let prices = tranches.calculate_prices::<Rate, TTokens, TrancheCurrency>(
					1_100_000_000,
					SECS_PER_YEAR,
				);
				// should be no change if the last update happened at the provided moment
				assert_eq!(
					prices,
					tranches.calculate_prices::<Rate, TTokens, TrancheCurrency>(
						1_100_000_000,
						SECS_PER_YEAR
					)
				);
			}
		}

		mod rebalance {
			use super::*;

			const TOTAL_ASSETS: Balance = DEBT_RES
				+ RESERVE_RES + DEBT_NONRES_1
				+ RESERVE_NONRES_1
				+ DEBT_NONRES_2 + RESERVE_NONRES_2;
			const RATIO_NONRES_1: Balance = DEBT_NONRES_1 + RESERVE_NONRES_1;
			const RATIO_NONRES_2: Balance = DEBT_NONRES_2 + RESERVE_NONRES_2;
			const DEFAULT_NAV: Balance = 1_234_567_890;

			// Compares tranches which were rebalanced with expected outcome for debt and
			// reserve.
			fn assert_rebalancing_eq(
				rebalance_tranches: TTranches,
				[(debt_res, reserve_res), (debt_nonres_1, reserve_nonres_1), (debt_nonres2, reserve_nonres_2)]: &[(Balance, Balance); 3],
			) {
				assert_eq!(&rebalance_tranches.tranches[0].debt, debt_res);
				assert_eq!(&rebalance_tranches.tranches[0].reserve, reserve_res);
				assert_eq!(&rebalance_tranches.tranches[1].debt, debt_nonres_1);
				assert_eq!(&rebalance_tranches.tranches[1].reserve, reserve_nonres_1);
				assert_eq!(&rebalance_tranches.tranches[2].debt, debt_nonres2);
				assert_eq!(&rebalance_tranches.tranches[2].reserve, reserve_nonres_2);
			}

			#[test]
			fn zero_setup_works() {
				let mut tranches = default_tranches_with_issuance();

				assert_ok!(tranches.rebalance_tranches(
					0,
					0,
					0,
					&[
						Perquintill::from_percent(0),
						Perquintill::from_percent(0),
						Perquintill::from_percent(0),
					],
					&[(0, 0), (0, 0), (0, 0)],
				));

				assert_eq!(tranches, default_tranches_with_seniority());
			}

			// Assing zero to moment, tranche ratios as well as expected in- and outflows.
			// As a result, target debt for non-res tranches should be zero.
			// Thus, expect changes for res tranche debt and reserve, as well as non-res
			// debts.
			#[test]
			fn no_timediff_ratios_amounts_works() {
				let mut tranches = default_tranches_with_issuance();

				assert_ok!(tranches.rebalance_tranches(
					0,
					TOTAL_ASSETS,
					// arbitrary amount which does not have any effect
					DEFAULT_NAV,
					&[
						Perquintill::from_percent(0),
						Perquintill::from_percent(0),
						Perquintill::from_percent(0),
					],
					&[(0, 0), (0, 0), (0, 0)],
				));

				assert_rebalancing_eq(
					tranches,
					&[
						(DEFAULT_NAV, DEBT_RES + RESERVE_RES),
						(0, RATIO_NONRES_1),
						(0, RATIO_NONRES_2),
					],
				);
			}

			#[test]
			fn timediff_without_ratios_amounts_works() {
				for year in 1..20 {
					let mut tranches_no_rebalance = default_tranches_with_issuance();
					let mut tranches_rebalance = default_tranches_with_issuance();

					let seconds = year * SECS_PER_YEAR;
					assert_ok!(&tranches_rebalance.rebalance_tranches(
						seconds,
						2 * TOTAL_ASSETS,
						DEFAULT_NAV,
						&[
							Perquintill::from_percent(0),
							Perquintill::from_percent(0),
							Perquintill::from_percent(0),
						],
						&[(0, 0), (0, 0), (0, 0)],
					));

					// apply rate to nonres debts
					assert_ok!(tranches_no_rebalance.tranches[1].accrue(seconds));
					assert_ok!(tranches_no_rebalance.tranches[2].accrue(seconds));
					let reserve_nonres_1 = tranches_no_rebalance.tranches[1].debt
						+ tranches_no_rebalance.tranches[1].reserve;
					let reserve_nonres_2 = tranches_no_rebalance.tranches[2].debt
						+ tranches_no_rebalance.tranches[2].reserve;

					assert_rebalancing_eq(
						tranches_rebalance,
						&[
							(
								DEFAULT_NAV,
								2 * TOTAL_ASSETS - reserve_nonres_1 - reserve_nonres_2,
							),
							(0, reserve_nonres_1),
							(0, reserve_nonres_2),
						],
					);
				}
			}

			// For failure we need to have
			// 	* non-res rates > 0% (the higher, the easier, thus 100% for simplicity)
			// 	* pool value (pool reserve + NAV) > ratio of the most senior non-res tranche
			//  * non-zero reserve and NAV
			#[test]
			fn nav_too_low_throws() {
				let mut tranches = default_tranches_with_issuance();
				assert_eq!(tranches.rebalance_tranches(
							0,
							RESERVE_NONRES_2 + 1,
							DEBT_NONRES_2,
							&[
								Perquintill::from_percent(0),
								Perquintill::from_percent(100),
								Perquintill::from_percent(100),
							],
							&[(0, 0), (0, 0), (0, 0)],
						), Err(DispatchError::Other("Corrupted pool-state. Pool NAV should be able to handle tranche debt substraction.")));
				assert_ok!(tranches.rebalance_tranches(
					0,
					RESERVE_NONRES_2 - 1,
					DEBT_NONRES_2,
					&[
						Perquintill::from_percent(0),
						Perquintill::from_percent(100),
						Perquintill::from_percent(100),
					],
					&[(0, 0), (0, 0), (0, 0)],
				));
			}

			// NAV needs to be greater 0. Else, expected debt is zero and we expect success.
			#[test]
			fn reserve_too_low_throws() {
				let mut tranches = default_tranches_with_issuance();
				let nav = 1;

				assert_eq!(tranches.rebalance_tranches(
							0,
							// min reserve to throw when NAV is 1
							0,
							nav,
							&[
								Perquintill::from_percent(0),
								Perquintill::from_percent(0),
								Perquintill::from_percent(0),
							],
							&[(0, 0), (0, 0), (0, 0)],
						), Err(DispatchError::Other("Corrupted pool-state. Pool reserve should be able to handle tranche reserve substraction.")));

				assert_eq!(tranches.rebalance_tranches(
							0,
							// max reserve to throw
							RESERVE_NONRES_1 + RESERVE_NONRES_2 - 2,
							// with min nav
							nav,
							&[
								Perquintill::from_percent(0),
								Perquintill::from_percent(0),
								Perquintill::from_percent(0),
							],
							&[(0, 0), (0, 0), (0, 0)],
						), Err(DispatchError::Other("Corrupted pool-state. Pool reserve should be able to handle tranche reserve substraction.")));

				assert_ok!(tranches.rebalance_tranches(
					0,
					RESERVE_NONRES_1 + RESERVE_NONRES_2 - 1,
					nav,
					&[
						Perquintill::from_percent(0),
						Perquintill::from_percent(0),
						Perquintill::from_percent(0),
					],
					&[(0, 0), (0, 0), (0, 0)],
				));
			}

			// Compare two setups which differ in their ratios and choose NAV such that
			// rebalanced non-res debts and reserves are equal for same tranche in.
			#[test]
			fn ratios_work() {
				let mut single_rate = default_tranches_with_issuance();
				let mut double_rate = default_tranches_with_issuance();
				let rates = [
					Perquintill::from_percent(0),
					Perquintill::from_percent(10),
					Perquintill::from_percent(15),
				];

				assert_ok!(single_rate.rebalance_tranches(
					0,
					TOTAL_ASSETS,
					DEFAULT_NAV,
					&rates,
					&[(0, 0), (0, 0), (0, 0)],
				));
				assert_ok!(double_rate.rebalance_tranches(
					0,
					TOTAL_ASSETS,
					DEFAULT_NAV / 2,
					&rates.map(|rate| rate + rate),
					&[(0, 0), (0, 0), (0, 0)],
				));

				assert_eq!(single_rate.tranches[1].debt, double_rate.tranches[1].debt);
				assert_eq!(
					single_rate.tranches[1].reserve,
					double_rate.tranches[1].reserve
				);
				assert_eq!(single_rate.tranches[2].debt, double_rate.tranches[2].debt);
				assert_eq!(
					single_rate.tranches[2].reserve,
					double_rate.tranches[2].reserve
				);
			}

			#[test]
			fn executed_amounts_work() {
				let mut tranches = default_tranches_with_issuance();
				let amounts = [
					(20_000_000, 10_000_000),
					(50_000_000, 10_000_000),
					(100_000_000, 10_000_000),
				];

				assert_ok!(tranches.rebalance_tranches(
					0,
					TOTAL_ASSETS,
					DEFAULT_NAV,
					&[
						Perquintill::from_percent(0),
						Perquintill::from_percent(0),
						Perquintill::from_percent(0),
					],
					&amounts
				));

				// Executed amounts only affect reserves
				assert_eq!(
					tranches.tranches[1].reserve,
					RATIO_NONRES_1 + amounts[1].0 - amounts[1].1
				);
				assert_eq!(
					tranches.tranches[2].reserve,
					RATIO_NONRES_2 + amounts[2].0 - amounts[2].1
				);
				assert_eq!(
					tranches.tranches[0].reserve,
					TOTAL_ASSETS - tranches.tranches[1].reserve - tranches.tranches[2].reserve
				);
			}
		}

		#[test]
		fn num_tranches_works() {
			let mut tranches = default_tranches();
			assert_eq!(tranches.num_tranches(), 3);

			// decrease size
			assert_ok!(tranches.remove(1));
			assert_eq!(tranches.num_tranches(), 2);

			// increase size
			let input = TrancheInput {
				seniority: Some(5),
				tranche_type: TrancheType::NonResidual {
					interest_rate_per_sec: Rate::one(),
					min_risk_buffer: Perquintill::from_percent(5),
				},
				metadata: TrancheMetadata {
					token_name: BoundedVec::<u8, TokenNameLen>::default(),
					token_symbol: BoundedVec::<u8, TokenSymLen>::default(),
				},
			};
			assert_ok!(tranches.add(2, input, 0u64));
			assert_eq!(tranches.num_tranches(), 3);
		}

		#[test]
		fn into_tranches_works() {
			let tranches = default_tranches();
			let tranches_2 = default_tranches();
			assert_eq!(tranches_2.into_tranches(), tranches.tranches);
		}

		#[test]
		fn non_residual_tranches_works() {
			let mut tranches = default_tranches();
			let non_res_tranches = tranches.non_residual_tranches().unwrap();
			assert_eq!(non_res_tranches.len(), 2);

			assert_eq!(
				non_res_tranches[0].tranche_type,
				non_residual(1, Some(10), Some(10)).tranche_type
			);
			assert_eq!(
				non_res_tranches[1].tranche_type,
				non_residual(2, Some(5), Some(25)).tranche_type
			);

			// remove non-residual tranches
			assert_ok!(tranches.remove(2));
			assert_ok!(tranches.remove(1));
			assert_eq!(tranches.non_residual_tranches(), None);
		}

		#[test]
		fn non_residual_tranches_mut_works() {
			let mut tranches = default_tranches();
			let non_res_tranches = tranches.non_residual_tranches_mut().unwrap();
			assert_eq!(non_res_tranches.len(), 2);

			assert_eq!(
				non_res_tranches[0].tranche_type,
				non_residual(1, Some(10), Some(10)).tranche_type
			);
			assert_eq!(
				non_res_tranches[1].tranche_type,
				non_residual(2, Some(5), Some(25)).tranche_type
			);

			// remove residual tranches
			assert_ok!(tranches.remove(2));
			assert_ok!(tranches.remove(1));
			assert_eq!(tranches.non_residual_tranches_mut(), None);
		}

		#[test]
		fn residual_tranche() {
			let mut tranches = default_tranches();
			assert_eq!(
				tranches.residual_tranche(),
				Some(&residual_base(0, 0, 0, 0))
			);

			// break assumption of existing residual tranche via private API for the sake of
			// the test
			tranches.tranches.remove(0);
			assert!(tranches.residual_tranche().is_none());
		}

		#[test]
		fn residual_tranche_mut_works() {
			let mut tranches = default_tranches();
			assert_eq!(
				tranches.residual_tranche_mut(),
				Some(&mut residual_base(0, 0, 0, 0))
			);

			// break assumption of existing residual tranche via private API for the sake of
			// the test
			tranches.tranches.remove(0);
			assert!(tranches.residual_tranche_mut().is_none());
		}

		#[test]
		fn non_residual_top_slice_works() {
			let tranches = default_tranches();
			assert_eq!(
				tranches.non_residual_top_slice().len(),
				tranches.num_tranches()
			);
			assert_eq!(
				tranches.non_residual_top_slice().first(),
				Some(&non_residual(2, Some(5), Some(25))),
			);
			assert_eq!(tranches.non_residual_top_slice().last(), Some(&residual(0)),);
		}
		#[test]
		fn non_residual_top_slice_mut_works() {
			let mut tranches = default_tranches();
			assert_eq!(
				tranches.non_residual_top_slice_mut().len(),
				tranches.num_tranches()
			);
			assert_eq!(
				tranches.non_residual_top_slice_mut().first_mut(),
				Some(&mut non_residual(2, Some(5), Some(25))),
			);
			assert_eq!(
				tranches.non_residual_top_slice_mut().last_mut(),
				Some(&mut residual(0)),
			);
		}

		#[test]
		fn residual_top_slice_works() {
			let tranches = default_tranches();
			assert_eq!(tranches.residual_top_slice().len(), tranches.num_tranches());
			assert_eq!(tranches.residual_top_slice().first(), Some(&residual(0)));
			assert_eq!(
				tranches.residual_top_slice().last(),
				Some(&non_residual(2, Some(5), Some(25))),
			);
		}

		#[test]
		fn residual_top_slice_mut_works() {
			let mut tranches = default_tranches();
			assert_eq!(
				tranches.residual_top_slice_mut().len(),
				tranches.num_tranches()
			);
			assert_eq!(
				tranches.residual_top_slice_mut().first_mut(),
				Some(&mut residual(0))
			);
			assert_eq!(
				tranches.residual_top_slice_mut().last_mut(),
				Some(&mut non_residual(2, Some(5), Some(25))),
			);
		}

		#[test]
		fn supplies_works() {
			let mut tranches = default_tranches();
			assert_eq!(tranches.supplies(), Ok(vec![0, 0, 0]));

			tranches.get_mut_tranche(TrancheLoc::Index(0)).unwrap().debt = 50;
			tranches
				.get_mut_tranche(TrancheLoc::Index(1))
				.unwrap()
				.reserve = 40;
			tranches.get_mut_tranche(TrancheLoc::Index(2)).unwrap().debt = 4;
			tranches
				.get_mut_tranche(TrancheLoc::Index(2))
				.unwrap()
				.reserve = 6;
			assert_eq!(tranches.supplies(), Ok(vec![50, 40, 10]));
		}

		#[test]
		fn acc_supply_works() {
			let mut tranches = default_tranches();
			assert_eq!(tranches.acc_supply(), Ok(0));

			tranches.get_mut_tranche(TrancheLoc::Index(0)).unwrap().debt = 50;
			tranches
				.get_mut_tranche(TrancheLoc::Index(1))
				.unwrap()
				.reserve = 40;
			tranches.get_mut_tranche(TrancheLoc::Index(2)).unwrap().debt = 4;
			tranches
				.get_mut_tranche(TrancheLoc::Index(2))
				.unwrap()
				.reserve = 6;
			assert_eq!(tranches.acc_supply(), Ok(100));
		}

		#[test]
		fn min_risk_buffers_works() {
			let mut tranches = default_tranches();
			assert_eq!(
				tranches.min_risk_buffers(),
				vec![
					Perquintill::from_percent(0),
					Perquintill::from_percent(10),
					Perquintill::from_percent(25)
				]
			);

			for i in 0u64..tranches.num_tranches().ensure_into().unwrap() {
				tranches
					.get_mut_tranche(TrancheLoc::Index(i))
					.unwrap()
					.tranche_type = TrancheType::NonResidual {
					min_risk_buffer: Perquintill::from_percent(i),
					interest_rate_per_sec: Rate::one(),
				};
			}
			assert_eq!(
				tranches.min_risk_buffers(),
				vec![
					Perquintill::from_percent(0),
					Perquintill::from_percent(1),
					Perquintill::from_percent(2)
				]
			);
		}

		#[test]
		fn seniorities_works() {
			let mut tranches = default_tranches_with_seniority();
			assert_eq!(tranches.seniorities(), vec![0, 1, 2]);

			for i in 0u32..tranches.num_tranches().ensure_into().unwrap() {
				tranches
					.get_mut_tranche(TrancheLoc::Index(i.into()))
					.unwrap()
					.seniority += i;
			}
			assert_eq!(tranches.seniorities(), vec![0, 2, 4]);
		}
	}

	mod tranche_id_gen {
		use super::*;

		#[test]
		fn id_from_salt_works() {
			let index: TrancheIndex = 1u64;
			let salt: TrancheSalt<PoolId> = (index, DEFAULT_POOL_ID);
			let expected_txt_vec = [
				59u8, 168, 10, 55, 120, 240, 78, 191, 69, 232, 6, 209, 154, 5, 32, 37,
			];
			assert_eq!(
				Tranches::<
					Balance,
					Rate,
					TrancheWeight,
					TrancheCurrency,
					TrancheId,
					PoolId,
					MaxTranches,
				>::id_from_salt(salt),
				expected_txt_vec
			)
		}
	}

	mod epoch_execution_tranches {
		use super::*;

		#[test]
		fn epoch_execution_tranche_reverse_works() {
			assert_eq!(
				default_epoch_tranches().non_residual_top_slice()[0].seniority,
				2
			)
		}

		#[test]
		fn epoch_execution_tranche_reverse_mut_works() {
			let mut check_vals: Vec<u128> = Vec::new();
			default_epoch_tranches()
				.non_residual_top_slice_mut()
				.into_iter()
				.for_each(|t| {
					t.invest = 100 * t.seniority as u128;
					check_vals.push(t.invest);
				});

			assert_eq!(check_vals, [200, 100, 0])
		}

		#[test]
		#[should_panic]
		fn epoch_execution_tranche_reverse_slice_panics_on_out_of_bounds() {
			// 3 elements in default_epoch_tranches
			let _panic = &default_epoch_tranches().non_residual_top_slice()[3];
		}

		#[test]
		fn epoch_execution_residual_top_slice_returns_residual_first() {
			assert_eq!(
				default_epoch_tranches().residual_top_slice()[0].seniority,
				0
			)
		}

		#[test]
		fn epoch_execution_residual_top_mut_works() {
			let mut check_vals: Vec<u128> = Vec::new();
			default_epoch_tranches()
				.residual_top_slice_mut()
				.into_iter()
				.for_each(|t| {
					t.invest = 100 * t.seniority as u128;
					check_vals.push(t.invest);
				});

			assert_eq!(check_vals, [0, 100, 200])
		}

		#[test]
		fn epoch_execution_non_residual_tranches_works() {
			assert_eq!(
				default_epoch_tranches()
					.non_residual_tranches()
					.unwrap()
					.iter()
					.map(|t| t.seniority)
					.collect::<Vec<_>>(),
				[1, 2]
			)
		}

		#[test]
		fn epoch_execution_non_residual_tranches_mut_works() {
			let mut check_vals: Vec<u32> = Vec::new();
			default_epoch_tranches()
				.non_residual_tranches_mut()
				.unwrap()
				.into_iter()
				.for_each(|t| {
					t.seniority += 2;
					check_vals.push(t.seniority);
				});

			assert_eq!(check_vals, [3, 4])
		}

		#[test]
		fn epoch_execution_residual_tranche_works() {
			assert_eq!(
				default_epoch_tranches()
					.residual_tranche()
					.unwrap()
					.seniority,
				0
			)
		}

		#[test]
		fn epoch_execution_residual_tranche_mut_works() {
			let mut epoch_tranches = default_epoch_tranches();
			let mut epoch_tranche = epoch_tranches.residual_tranche_mut().unwrap();
			epoch_tranche.invest = 200;

			assert_eq!(epoch_tranche.invest, 200)
		}

		#[test]
		fn epoch_execution_num_tranches_works() {
			assert_eq!(default_epoch_tranches().num_tranches(), 3)
		}

		#[test]
		fn epoch_execution_into_tranches_works() {
			let tranches = default_epoch_tranches().into_tranches();

			// it would be good to move this to a assert_match! once that's in stable
			let tranches_check = match tranches.as_slice() {
				[EpochExecutionTranche { .. }, EpochExecutionTranche { .. }, EpochExecutionTranche { .. }] => {
					true
				}
				_ => false,
			};

			assert!(tranches_check)
		}

		#[test]
		fn epoch_execution_combine_non_residual_top_works() {
			let new_combined_ee_tranches =
				default_epoch_tranches().combine_non_residual_top(|t| Ok(t.seniority));

			// it would be good to move this to a assert_match! once that's in stable
			assert_eq!(new_combined_ee_tranches.unwrap()[..], [2, 1, 0])
		}

		#[test]
		fn epoch_execution_combine_mut_non_residual_top_works() {
			let mut tranches = default_epoch_tranches();
			let mut order: Vec<u32> = Vec::new();
			let tranche_mut_res = tranches.combine_mut_non_residual_top(|t| {
				let old_invest = t.invest;
				let new_invest = 100 * t.seniority as u128;
				// check mutation
				t.invest = new_invest;
				// to check order processed
				order.push(t.seniority);
				// verify collection
				Ok((t.seniority, old_invest, t.invest))
			});

			let tranche_invest_vals = tranches
				.into_tranches()
				.iter()
				.map(|t| t.invest)
				.collect::<Vec<_>>();

			// check for mutated vals in exsiting tranches
			assert_eq!(tranche_invest_vals, [0, 100, 200]);

			// check order processed
			assert_eq!(order, [2, 1, 0]);

			// check collection
			assert_eq!(
				tranche_mut_res.unwrap(),
				[(2, 0, 200), (1, 0, 100), (0, 0, 0)]
			)
		}

		#[test]
		fn epoch_execution_combine_with_non_residual_top_works() {
			assert_eq!(
				default_epoch_tranches()
					.combine_with_non_residual_top(&[220, 210, 250], |tranche, other_val| {
						Ok((tranche.seniority, *other_val))
					})
					.unwrap(),
				[(2, 220), (1, 210), (0, 250)]
			);

			// error if len col < tranches count
			assert_eq!(
				default_epoch_tranches()
					.combine_with_non_residual_top(&[220, 210], |tranche, other_val| {
						Ok((tranche.seniority, *other_val))
					}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);

			// error if len with > tranches count
			assert_eq!(
				default_epoch_tranches()
					.combine_with_non_residual_top(&[220, 210, 250, 110], |tranche, other_val| {
						Ok((tranche.seniority, *other_val))
					}),
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice",
				))
			);

			// error if col is empty
			assert_eq!(
				default_epoch_tranches()
					.combine_with_non_residual_top(vec![], |tranche, other_val: u32| {
						Ok((tranche.seniority, other_val))
					}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable",
				))
			);
		}

		#[test]
		fn epoch_execution_combine_with_mut_non_residual_top_works() {
			let mut order: Vec<u32> = Vec::new();
			let mut tranches = default_epoch_tranches();
			let new_investment_vals = [220, 210, 250];

			let res = tranches.combine_with_mut_non_residual_top(
				&new_investment_vals,
				|t, new_investment| {
					let old_invest = t.invest;
					// to verify mutation
					t.invest += *new_investment as u128;
					// to verify order processed
					order.push(t.seniority);

					// verify collection
					Ok((t.seniority, old_invest, t.invest))
				},
			);

			let tranche_invest_vals = tranches
				.into_tranches()
				.iter()
				.map(|t| t.invest)
				.collect::<Vec<_>>();

			// check mutated epoch_execution_tranches
			// note -- tranches are stored with residual first,
			// and combine_with_non_residual_top processes residual first -- reverses
			// tranche order however given that this is mutating existing
			// EpochExecutionTranches we'd expect the order to still be
			// non-residual->residual
			assert_eq!(tranche_invest_vals, [250, 210, 220]);

			// check order processed
			assert_eq!(order, [2, 1, 0]);

			// check collection
			// note -- collection done with non-residual first
			assert_eq!(res.unwrap(), [(2, 0, 220), (1, 0, 210), (0, 0, 250)]);

			// error if len col > tranches count
			assert_eq!(
				default_epoch_tranches().combine_with_mut_non_residual_top(
					&[220, 210, 250, 252],
					|t, new_investment| {
						let old_invest = t.invest;
						t.invest += *new_investment as u128;
						order.push(t.seniority);
						Ok((t.seniority, old_invest, t.invest))
					},
				),
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice",
				))
			);

			// error if len col < tranches count
			assert_eq!(
				default_epoch_tranches().combine_with_mut_non_residual_top(
					&[220, 210],
					|t, new_investment| {
						let old_invest = t.invest;
						t.invest += *new_investment as u128;
						order.push(t.seniority);
						Ok((t.seniority, old_invest, t.invest))
					},
				),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable",
				))
			);

			// error if len col empty
			assert_eq!(
				default_epoch_tranches().combine_with_mut_non_residual_top(
					[],
					|t, new_investment: u128| {
						let old_invest = t.invest;
						t.invest += new_investment as u128;
						order.push(t.seniority);
						Ok((t.seniority, old_invest, t.invest))
					},
				),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable",
				))
			)
		}

		#[test]
		fn epoch_execution_combine_residual_top_works() {
			assert_eq!(
				default_epoch_tranches().combine_residual_top(|t| Ok(t.seniority)),
				Ok(vec![0, 1, 2])
			)
		}

		#[test]
		fn epoch_execution_combine_residual_top_mut_works() {
			assert_eq!(
				default_epoch_tranches().combine_mut_residual_top(|t| {
					t.invest = 100 * t.seniority as u128;
					Ok(t.invest)
				}),
				Ok(vec![0, 100, 200])
			)
		}

		#[test]
		fn epoch_execution_combine_with_residual_top() {
			assert_eq!(
				default_epoch_tranches().combine_with_residual_top([1, 2, 3], |t, zip_val| {
					Ok((t.seniority, zip_val))
				}),
				Ok(vec![(0, 1), (1, 2), (2, 3)])
			);

			assert_eq!(
				default_epoch_tranches().combine_with_residual_top([1, 2, 3, 4], |t, zip_val| {
					Ok((t.seniority, zip_val))
				}),
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice"
				))
			);

			assert_eq!(
				default_epoch_tranches()
					.combine_with_residual_top([1, 2], |t, zip_val| { Ok((t.seniority, zip_val)) }),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);

			assert_eq!(
				default_epoch_tranches().combine_with_residual_top(vec![], |t, zip_val: u32| {
					Ok((t.seniority, zip_val))
				}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			)
		}

		#[test]
		fn epoch_execution_combine_with_mut_residual_top_works() {
			let mut tranches = default_epoch_tranches();

			// verify collection
			// collection is from res to non-res -- same order as tranches
			assert_eq!(
				tranches
					.combine_with_mut_residual_top([220, 110, 250], |t, zip_val| {
						t.invest = zip_val as u128;
						Ok((t.seniority, t.invest))
					})
					.unwrap()[..],
				[(0, 220), (1, 110), (2, 250)]
			);

			// check mutated epoch_execution_tranches
			// same order expected
			let tranche_invest_vals = tranches
				.into_tranches()
				.iter()
				.map(|t| t.invest)
				.collect::<Vec<_>>();
			assert_eq!(tranche_invest_vals, [220, 110, 250]);

			// error if col has less items than EpochExecutionTrances
			assert_eq!(
				default_epoch_tranches().combine_with_mut_residual_top([220, 110], |t, zip_val| {
					t.invest = zip_val as u128;
					Ok((t.seniority, t.invest))
				}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);

			// error if col has more items than EpochExecutionTranches
			assert_eq!(
				default_epoch_tranches().combine_with_mut_residual_top(
					[220, 110, 222, 333],
					|t, zip_val| {
						t.invest = zip_val as u128;
						Ok((t.seniority, t.invest))
					}
				),
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice"
				))
			);

			// error if col is empty and EpochExecutionTranches is not
			assert_eq!(
				default_epoch_tranches().combine_with_mut_residual_top([], |t, zip_val: u32| {
					t.invest = zip_val as u128;
					Ok((t.seniority, t.invest))
				}),
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				))
			);
		}

		#[test]
		fn epoch_execution_tranches_prices_works() {
			let tranches = default_epoch_tranches();

			assert_eq!(
				vec![Rate::one(), Rate::one(), Rate::one()],
				tranches.prices()
			)
		}

		#[test]
		fn epoch_execution_supplies_with_fulfillment_works() {
			assert_eq!(
				Err(DispatchError::Other(
					"Iterable contains more elements than Tranches slice"
				)),
				default_epoch_tranches().supplies_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			assert_eq!(
				Err(DispatchError::Other(
					"Tranches slice contains more elements than combining iterable"
				)),
				default_epoch_tranches().supplies_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			assert_eq!(
				Ok(vec![0, 0, 0]),
				default_epoch_tranches().supplies_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top([(100, 100), (200, 200), (300, 300)], |e, (i, r)| {
					e.invest = i;
					e.redeem = r;
					Ok(())
				})
				.unwrap();

			assert_eq!(
				Err(DispatchError::Arithmetic(ArithmeticError::Underflow)),
				e_e_tranches.supplies_with_fulfillment(&[
					tranche_solution(Perquintill::from_percent(50), Perquintill::one()),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top(
					[(1000, u128::MAX, 100), (200, 0, 200), (300, 0, 300)],
					|e, (i, s, r)| {
						e.invest = i;
						e.supply = s;
						e.redeem = r;
						Ok(())
					},
				)
				.unwrap();

			assert_eq!(
				Err(DispatchError::Arithmetic(ArithmeticError::Overflow)),
				e_e_tranches.supplies_with_fulfillment(&[
					tranche_solution(Perquintill::one(), Perquintill::from_percent(1)),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top([(100, 100), (200, 200), (300, 300)], |e, (i, r)| {
					e.invest = i;
					e.redeem = r;
					Ok(())
				})
				.unwrap();

			assert_eq!(
				Ok(vec![0, 0, 0]),
				e_e_tranches.supplies_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top([(200, 100), (200, 200), (300, 300)], |e, (i, r)| {
					e.invest = i;
					e.redeem = r;
					Ok(())
				})
				.unwrap();

			assert_eq!(
				Ok(vec![100, 0, 0]),
				e_e_tranches.supplies_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top([(200, 100), (200, 200), (300, 300)], |e, (i, r)| {
					e.invest = i;
					e.redeem = r;
					Ok(())
				})
				.unwrap();

			// Percentages for tranche solution taken into account for supplies
			assert_eq!(
				Ok(vec![50, 0, 0]),
				e_e_tranches.supplies_with_fulfillment(&[
					tranche_solution(Perquintill::from_percent(50), Perquintill::from_percent(50)),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top(
					[(200, 100, 100), (200, 100, 200), (300, 200, 300)],
					|e, (i, s, r)| {
						e.invest = i;
						e.supply = s;
						e.redeem = r;
						Ok(())
					},
				)
				.unwrap();

			// Supplies updated correctly with pre-existing supply ammounts
			// with invest amount doubling redeem for first tranch
			assert_eq!(
				Ok(vec![200, 100, 200]),
				e_e_tranches.supplies_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);
		}

		#[test]
		fn epoch_execution_calculate_weights_works() {
			// Note: weight tuple containss: (investment_weight, redemption_weight)
			// seniority of default tranches: 1, 2, 3
			// expected weights: [
			//  (highest_invest_weight, lowest_redemption_weight),
			//  (mid_invest_weight, mid_redemption_weight),
			//  (lowest_invest_weight, highest_redemption_Weight)
			// ]
			assert_eq!(
				default_epoch_tranches().calculate_weights(),
				vec![
					(TrancheWeight::from(1000), TrancheWeight::from(10000)),
					(TrancheWeight::from(100), TrancheWeight::from(100000)),
					(TrancheWeight::from(10), TrancheWeight::from(1000000))
				]
			);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top([u32::MAX - 2, u32::MAX - 1, u32::MAX], |e, s| {
					e.seniority = s;
					Ok(())
				})
				.unwrap();
			// Verify weights don't panic when calc would overflow
			assert_eq!(
				e_e_tranches.calculate_weights(),
				vec![
					(
						TrancheWeight::from(u128::MAX),
						TrancheWeight::from(u128::MAX)
					),
					(
						TrancheWeight::from(u128::MAX),
						TrancheWeight::from(u128::MAX)
					),
					(
						TrancheWeight::from(u128::MAX),
						TrancheWeight::from(u128::MAX)
					)
				]
			)
		}

		#[test]
		fn epoch_execution_min_risk_buffers() {
			let epoch_execution_tranches = default_epoch_tranches();

			let z = Perquintill::zero();

			assert_eq!(epoch_execution_tranches.min_risk_buffers(), vec![z, z, z]);

			let mut e_e_tranches = default_epoch_tranches();
			e_e_tranches
				.combine_with_mut_residual_top(
					[z, Perquintill::from_rational(5u64, 100), Perquintill::one()],
					|e, s| {
						e.min_risk_buffer = s;
						Ok(())
					},
				)
				.unwrap();

			assert_eq!(
				e_e_tranches.min_risk_buffers(),
				vec![z, Perquintill::from_rational(1u64, 20), Perquintill::one()]
			);
		}

		#[test]
		fn epoch_execution_tranches_prices() {
			let epoch_execution_tranches = default_epoch_tranches();
			let r = Rate::one();
			assert_eq!(epoch_execution_tranches.prices(), vec![r, r, r])
		}
	}

	#[test]
	fn epoch_execution_tranches_fulfillment_cash_flows_works() {
		let mut e_e_tranches = default_epoch_tranches();
		let b = |x: u128| Balance::from(x);

		let full = Perquintill::one();
		let half = Perquintill::from_rational(1u128, 2u128);
		let default_solution = TrancheSolution {
			invest_fulfillment: full,
			redeem_fulfillment: full,
		};
		let half_invest = TrancheSolution {
			invest_fulfillment: half,
			redeem_fulfillment: full,
		};

		let half_redeem = TrancheSolution {
			invest_fulfillment: full,
			redeem_fulfillment: half,
		};
		let s_tranches = [default_solution, half_invest, half_redeem];

		let epoch_tranche_invest_redeem_vals =
			[(b(10000), b(1000)), (b(2000), b(530)), (b(3000), b(2995))];

		let expected_cash_flow_vals =
			vec![(b(10000), b(1000)), (b(1000), b(530)), (b(3000), b(1497))];

		e_e_tranches
			.combine_with_mut_residual_top(epoch_tranche_invest_redeem_vals, |e, (i, r)| {
				e.invest = i;
				e.redeem = r;
				Ok(())
			})
			.unwrap();

		assert_eq!(
			e_e_tranches.fulfillment_cash_flows(&s_tranches).unwrap(),
			expected_cash_flow_vals
		)
	}

	mod risk_buffers {
		use super::*;

		#[test]
		fn calculate_risk_buffers_works() {
			// note: this is basicallly taking the price and supply fields from the epoch
			// tranches in an epoch tranches struct. we're basically obtaining the pool
			// value from the price and supply of all epoch tranches then determining how
			// much buffer the tranches have based on the ratio of pool value
			// remaining after subtracting tranche pool value going from senior to junior
			// tranches note that we have 0 for the residual tranche, and 80% for the senior
			// tranche in this scenario
			let b = |x: u128| Balance::from(x);
			let supplies = [b(5), b(3), b(2)];
			let prices = [
				BalanceRatio::one(),
				BalanceRatio::one(),
				BalanceRatio::one(),
			];

			assert_eq!(
				calculate_risk_buffers(&supplies, &prices).unwrap(),
				vec![
					Perquintill::zero(),
					Perquintill::from_rational(1, 2u64),
					Perquintill::from_rational(4, 5u64)
				]
			);

			// verify that price is taken into account for pool value/risk buffers
			let supplies = [b(20), b(15), b(8)];
			let prices = [
				BalanceRatio::from(10),
				BalanceRatio::from(8),
				BalanceRatio::from(10),
			];

			assert_eq!(
				calculate_risk_buffers(&supplies, &prices).unwrap(),
				vec![
					Perquintill::zero(),
					Perquintill::from_rational(1, 2u64),
					Perquintill::from_rational(4, 5u64)
				]
			)
		}
	}
}
