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
use cfg_traits::{
	ops::{EnsureAdd, EnsureFixedPointNumber, EnsureInto, EnsureSub},
	TrancheCurrency as TrancheCurrencyT,
};
#[cfg(test)]
use cfg_types::{fixed_point::Rate, tokens::TrancheCurrency};
use cfg_types::{tokens::CustomMetadata, xcm::XcmMetadata};
use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	sp_runtime::ArithmeticError,
	traits::{fungibles::Inspect, Get},
	Blake2_128, BoundedVec, Parameter, RuntimeDebug, StorageHasher,
};
use orml_traits::asset_registry::AssetMetadata;
use polkadot_parachain::primitives::Id as ParachainId;
use rev_slice::{RevSlice, SliceExt};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{checked_pow, BaseArithmetic, Unsigned};
use sp_runtime::{
	traits::{ConstU32, Member, One, Zero},
	DispatchError, FixedPointNumber, FixedPointOperand, Perquintill, WeakBoundedVec,
};
use sp_std::{marker::PhantomData, vec::Vec};
use xcm::{
	latest::MultiLocation,
	prelude::{GeneralKey, PalletInstance, Parachain, X3},
	VersionedMultiLocation,
};

/// Type that indicates the seniority of a tranche
pub type Seniority = u32;

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo)]
pub struct TrancheInput<Rate, MaxTokenNameLength, MaxTokenSymbolLength>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	pub tranche_type: TrancheType<Rate>,
	pub seniority: Option<Seniority>,
	pub metadata: TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
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

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
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
	/// * (NonResidual, NonResidual) =>
	///         interest rate of next tranche must be smaller
	///         equal to the interest rate of self.
	///
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

#[derive(Debug, Encode, PartialEq, Eq, Decode, Clone, TypeInfo)]
pub struct TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
{
	pub token_name: BoundedVec<u8, MaxTokenNameLength>,
	pub token_symbol: BoundedVec<u8, MaxTokenSymbolLength>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
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
	pub fn balance(&self) -> Result<Balance, ArithmeticError> {
		self.debt.ensure_add(self.reserve)
	}

	pub fn free_balance(&self) -> Result<Balance, ArithmeticError> {
		Ok(self.reserve)
	}

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

	pub fn min_risk_buffer(&self) -> Perquintill {
		match &self.tranche_type {
			TrancheType::Residual => Perquintill::zero(),
			TrancheType::NonResidual {
				min_risk_buffer, ..
			} => *min_risk_buffer,
		}
	}

	pub fn interest_rate_per_sec(&self) -> Rate {
		match &self.tranche_type {
			TrancheType::Residual => One::one(),
			TrancheType::NonResidual {
				interest_rate_per_sec,
				..
			} => *interest_rate_per_sec,
		}
	}

	pub fn debt(&mut self, now: Moment) -> Result<Balance, DispatchError> {
		self.accrue(now)?;
		Ok(self.debt)
	}

	pub fn create_asset_metadata(
		&self,
		decimals: u32,
		parachain_id: ParachainId,
		pallet_index: u8,
		token_name: Vec<u8>,
		token_symbol: Vec<u8>,
	) -> AssetMetadata<Balance, CustomMetadata>
	where
		Balance: Zero,
		Currency: Encode,
		CustomMetadata: Parameter + Member + TypeInfo,
	{
		let tranche_id =
			WeakBoundedVec::<u8, ConstU32<32>>::force_from(self.currency.encode(), None);

		AssetMetadata {
			decimals,
			name: token_name,
			symbol: token_symbol,
			existential_deposit: Zero::zero(),
			location: Some(VersionedMultiLocation::V1(MultiLocation {
				parents: 1,
				interior: X3(
					Parachain(parachain_id.into()),
					PalletInstance(pallet_index),
					GeneralKey(tranche_id),
				),
			})),
			additional: CustomMetadata {
				mintable: false,
				permissioned: true,
				pool_currency: false,
				xcm: XcmMetadata {
					fee_per_second: None,
				},
			},
		}
	}
}

/// The index type for tranches
///
/// The `TrancheIndex` can be seen as an normal index into a vector, just
/// specified here as new-type to make this clear. U64 in order to keep the public api
/// clear.
/// In contrast to a `TrancheId` a `TrancheIndex` is not unique and does NOT refer to a
/// specific tranche, but rather to a specific tranche-location in the tranche-structure
/// of a pool.
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

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Tranches<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId> {
	pub tranches: Vec<Tranche<Balance, Rate, Weight, TrancheCurrency>>,
	pub ids: Vec<TrancheId>,
	pub salt: TrancheSalt<PoolId>,
}

#[cfg(test)]
impl Tranches<Balance, Rate, TrancheWeight, TrancheCurrency, TrancheId, PoolId> {
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
			>::id_from_salt(salt));
			salt = (index.ensure_add(1)?.ensure_into()?, pool);
		}

		Ok(Self {
			tranches,
			ids,
			salt,
		})
	}
}

// The solution struct for a specific tranche
#[derive(Encode, Decode, Copy, Clone, Eq, PartialEq, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TrancheSolution {
	pub invest_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
}

impl<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId>
	Tranches<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId>
where
	TrancheCurrency: Copy + TrancheCurrencyT<PoolId, TrancheId>,
	Balance: Zero + Copy + BaseArithmetic + FixedPointOperand + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	Rate: One + Copy + FixedPointNumber<Inner = Balance>,
	TrancheId: Clone + From<[u8; 16]> + sp_std::cmp::PartialEq,
	PoolId: Copy + Encode,
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
		let tranches = Vec::with_capacity(tranche_inputs.len());
		let ids = Vec::with_capacity(tranche_inputs.len());
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
			TrancheLoc::Id(id) => Some(id),
			TrancheLoc::Index(index) => index
				.try_into()
				.ok()
				.and_then(|index: usize| self.ids.get(index).cloned()),
		}
	}

	pub fn tranche_index(&self, id: &TrancheLoc<TrancheId>) -> Option<TrancheIndex> {
		match id {
			TrancheLoc::Index(index) => Some(*index),
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
					self.tranches.get(index)
				} else {
					None
				}
			}
			TrancheLoc::Id(id) => {
				let index = self.tranche_index(&TrancheLoc::Id(id));
				if let Some(index) = index {
					let index: Option<usize> = index.try_into().ok();
					if let Some(index) = index {
						self.tranches.get(index)
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
	/// * salt: The salt is a counter in our case that will always go
	///         up, even if we remove tranches.
	/// * pool-id: The pool id is ensured to be unique on-chain
	///
	/// -> tranche id = Twox128::hash(salt)
	fn next_id(&mut self) -> Result<TrancheId, DispatchError> {
		let id =
			Tranches::<Balance, Rate, Weight, TrancheCurrency, TrancheId, PoolId>::id_from_salt(
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
		self.remove(at)?;
		self.add::<MaxTokenNameLength, MaxTokenSymbolLength>(at, tranche, now)
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
		let at: usize = at.ensure_into()?;
		ensure!(
			at <= self.tranches.len(),
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
		if at == 0 {
			ensure!(
				tranche.tranche_type == TrancheType::Residual,
				DispatchError::Other(
					"Top tranche must be a residual one. This should be catched somewhere else"
				)
			);
		} else {
			ensure!(
				self.tranches
					.get(at - 1)
					.expect("at is <= len and is not zero. An element before at must exist. qed.")
					.tranche_type
					.valid_next_tranche(&new_tranche.tranche_type),
				DispatchError::Other(
					"Invalid next tranche type. This should be catched somewhere else."
				)
			);
		}
		self.tranches.insert(at, new_tranche);
		self.ids.insert(at, id);

		Ok(())
	}

	pub fn remove(&mut self, at: TrancheIndex) -> DispatchResult {
		let at: usize = at.ensure_into()?;
		ensure!(
			at < self.tranches.len(),
			DispatchError::Other(
				"Invalid tranche index. Exceeding number of tranches. This should be catched somewhere else."
			)
		);

		self.tranches.remove(at);
		self.ids.remove(at);

		Ok(())
	}

	pub fn ids_non_residual_top(&self) -> Vec<TrancheId> {
		let mut res = Vec::with_capacity(self.tranches.len());
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
		let mut res = Vec::with_capacity(self.tranches.len());
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
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.non_residual_top_slice_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_non_residual_top<R, I, W, F>(
		&self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, TrancheCurrency>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.non_residual_top_slice().iter().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut_non_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut Tranche<Balance, Rate, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self
			.non_residual_top_slice_mut()
			.iter_mut()
			.zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn ids_residual_top(&self) -> Vec<TrancheId> {
		self.ids.clone()
	}

	pub fn combine_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, TrancheCurrency>) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
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
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.residual_top_slice_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_residual_top<R, I, W, F>(
		&self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(&Tranche<Balance, Rate, Weight, TrancheCurrency>, W) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self.residual_top_slice().iter().zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
		}

		Ok(res)
	}

	pub fn combine_with_mut_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut Tranche<Balance, Rate, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let iter = self
			.residual_top_slice_mut()
			.iter_mut()
			.zip(with.into_iter());

		for (tranche, w) in iter {
			let r = f(tranche, w)?;
			res.push(r);
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
		Tokens: Inspect<AccountId, Balance = Balance>,
		TrancheCurrency: Into<<Tokens as Inspect<AccountId>>::AssetId>,
	{
		let mut remaining_assets = total_assets;
		let pool_is_zero = total_assets == Zero::zero();

		// we are gonna reverse the order
		// such that prices are calculated from most senior to junior
		// there by all the remaining assets are given to the most junior tranche
		let mut prices = self.combine_mut_non_residual_top(|tranche| {
			let total_issuance = Tokens::total_issuance(tranche.currency.into());

			if pool_is_zero || total_issuance == Zero::zero() {
				Ok(One::one())
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
					remaining_assets = remaining_assets
						.checked_sub(&tranche_balance)
						.expect("Tranche value smaller equal remaining assets. qed.");
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
		self.tranches.len()
	}

	pub fn into_tranches(self) -> Vec<Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		self.tranches
	}

	pub fn non_residual_tranches(
		&self,
	) -> Option<&[Tranche<Balance, Rate, Weight, TrancheCurrency>]> {
		if let Some((_head, tail)) = self.residual_top_slice().split_first() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn non_residual_tranches_mut(
		&mut self,
	) -> Option<&mut [Tranche<Balance, Rate, Weight, TrancheCurrency>]> {
		if let Some((_head, tail)) = self.residual_top_slice_mut().split_first_mut() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn residual_tranche(&self) -> Option<&Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		if let Some((head, _tail)) = self.residual_top_slice().split_first() {
			Some(head)
		} else {
			None
		}
	}

	pub fn residual_tranche_mut(
		&mut self,
	) -> Option<&mut Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		if let Some((head, _tail)) = self.residual_top_slice_mut().split_first_mut() {
			Some(head)
		} else {
			None
		}
	}

	pub fn non_residual_top_slice(
		&self,
	) -> &RevSlice<Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		self.tranches.rev()
	}

	pub fn non_residual_top_slice_mut(
		&mut self,
	) -> &mut RevSlice<Tranche<Balance, Rate, Weight, TrancheCurrency>> {
		self.tranches.rev_mut()
	}

	pub fn residual_top_slice(&self) -> &[Tranche<Balance, Rate, Weight, TrancheCurrency>] {
		self.tranches.as_slice()
	}

	pub fn residual_top_slice_mut(
		&mut self,
	) -> &mut [Tranche<Balance, Rate, Weight, TrancheCurrency>] {
		self.tranches.as_mut_slice()
	}

	pub fn supplies(&self) -> Result<Vec<Balance>, DispatchError> {
		Ok(self
			.residual_top_slice()
			.iter()
			.map(|tranche| tranche.debt.ensure_add(tranche.reserve))
			.collect::<Result<_, _>>()?)
	}

	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		Ok(self
			.residual_top_slice()
			.iter()
			.try_fold(Balance::zero(), |sum, tranche| {
				sum.ensure_add(tranche.debt)?.ensure_add(tranche.reserve)
			})?)
	}

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
						.checked_mul(10u128.pow(tranche.seniority.saturating_add(1)))
						.unwrap_or(u128::MAX)
						.into(),
				)
			})
			.collect()
	}

	pub fn min_risk_buffers(&self) -> Vec<Perquintill> {
		self.residual_top_slice()
			.iter()
			.map(|tranche| tranche.min_risk_buffer())
			.collect()
	}

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
		// This uses the current state of the tranches, rather than the cached epoch-close-time values.
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

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
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

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency> {
	pub tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>>,
}

/// Utility implementations for `EpochExecutionTranches`
impl<Balance, BalanceRatio, Weight, TrancheCurrency>
	EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency>
where
	Balance: Zero + Copy + BaseArithmetic + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	BalanceRatio: Copy,
{
	pub fn new(
		tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>>,
	) -> Self {
		Self { tranches }
	}

	pub fn non_residual_tranches(
		&self,
	) -> Option<&[EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>]> {
		if let Some((_head, tail)) = self.tranches.as_slice().split_first() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn non_residual_tranches_mut(
		&mut self,
	) -> Option<&mut [EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>]> {
		if let Some((_head, tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(tail)
		} else {
			None
		}
	}

	pub fn residual_tranche(
		&self,
	) -> Option<&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		if let Some((head, _tail)) = self.tranches.as_slice().split_first() {
			Some(head)
		} else {
			None
		}
	}

	pub fn residual_tranche_mut(
		&mut self,
	) -> Option<&mut EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		if let Some((head, _tail)) = self.tranches.as_mut_slice().split_first_mut() {
			Some(head)
		} else {
			None
		}
	}

	pub fn num_tranches(&self) -> usize {
		self.tranches.len()
	}

	pub fn into_tranches(
		self,
	) -> Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		self.tranches
	}

	pub fn non_residual_top_slice(
		&self,
	) -> &RevSlice<EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		self.tranches.rev()
	}

	pub fn non_residual_top_slice_mut(
		&mut self,
	) -> &mut RevSlice<EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>> {
		self.tranches.rev_mut()
	}

	pub fn residual_top_slice(
		&self,
	) -> &[EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>] {
		self.tranches.as_slice()
	}

	pub fn residual_top_slice_mut(
		&mut self,
	) -> &mut [EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>] {
		self.tranches.as_mut_slice()
	}

	pub fn combine_non_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
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
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.non_residual_top_slice_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_non_residual_top<R, I, W, F>(
		&self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let mut tranche_slice = self.non_residual_top_slice().iter();
		let mut with_iter = with.into_iter();

		for _ in 0..self.tranches.len() {
			match (tranche_slice.next(), with_iter.next()) {
				(Some(tranche), Some(w)) => res.push(f(tranche, w)?),
				_ => {
					return Err(DispatchError::Other(
						"EpochExecutionTranches contains more tranches than iterables elements",
					));
				}
			};
		}

		match (tranche_slice.next(), with_iter.next()) {
			(None, None) => Ok(res),
			_ => Err(DispatchError::Other(
				"Iterable contains more elements than EpochExecutionTranches tranche count",
			)),
		}
	}

	pub fn combine_with_mut_non_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		// we're going to have a mutable borrow later and will need len after that, grabbing now
		let tranches_count = self.tranches.len();
		let mut res = Vec::with_capacity(tranches_count);
		let mut tranche_slice = self.non_residual_top_slice_mut().iter_mut();
		let mut with_iter = with.into_iter();

		for _ in 0..tranches_count {
			match (tranche_slice.next(), with_iter.next()) {
				(Some(tranche), Some(w)) => res.push(f(tranche, w)?),
				_ => {
					return Err(DispatchError::Other(
						"EpochExecutionTranches contains more tranches than iterables elements",
					))
				}
			};
		}

		match (tranche_slice.next(), with_iter.next()) {
			(None, None) => Ok(res),
			_ => Err(DispatchError::Other(
				"Iterable contains more elements than EpochExecutionTranches tranche count",
			)),
		}
	}

	pub fn combine_residual_top<R, F>(&self, mut f: F) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
		) -> Result<R, DispatchError>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
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
		let mut res = Vec::with_capacity(self.tranches.len());
		for tranche in self.residual_top_slice_mut() {
			let r = f(tranche)?;
			res.push(r)
		}
		Ok(res)
	}

	pub fn combine_with_residual_top<R, I, W, F>(
		&self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		let mut res = Vec::with_capacity(self.tranches.len());
		let mut tranche_slice = self.residual_top_slice().iter();
		let mut with_iter = with.into_iter();

		for _ in 0..self.tranches.len() {
			match (tranche_slice.next(), with_iter.next()) {
				(Some(tranche), Some(w)) => res.push(f(tranche, w)?),
				_ => {
					return Err(DispatchError::Other(
						"EpochExecutionTranches contains more tranches than iterables elements",
					))
				}
			};
		}

		match (tranche_slice.next(), with_iter.next()) {
			(None, None) => Ok(res),
			_ => Err(DispatchError::Other(
				"Iterable contains more elements than EpochExecutionTranches tranche count",
			)),
		}
	}

	pub fn combine_with_mut_residual_top<R, W, I, F>(
		&mut self,
		with: I,
		mut f: F,
	) -> Result<Vec<R>, DispatchError>
	where
		F: FnMut(
			&mut EpochExecutionTranche<Balance, BalanceRatio, Weight, TrancheCurrency>,
			W,
		) -> Result<R, DispatchError>,
		I: IntoIterator<Item = W>,
	{
		// we're going to have a mutable borrow later and will need len after that, grabbing now
		let tranches_count = self.tranches.len();
		let mut res = Vec::with_capacity(tranches_count);
		let mut tranche_slice = self.residual_top_slice_mut().iter_mut();
		let mut with_iter = with.into_iter();

		for _ in 0..tranches_count {
			match (tranche_slice.next(), with_iter.next()) {
				(Some(tranche), Some(w)) => res.push(f(tranche, w)?),
				_ => {
					return Err(DispatchError::Other(
						"EpochExecutionTranches contains more tranches than iterables elements",
					))
				}
			};
		}

		match (tranche_slice.next(), with_iter.next()) {
			(None, None) => Ok(res),
			_ => Err(DispatchError::Other(
				"Iterable contains more elements than EpochExecutionTranches tranche count",
			)),
		}
	}
}

/// Business logic implementations for `EpochExecutionTranches`
impl<Balance, BalanceRatio, Weight, TrancheCurrency>
	EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency>
where
	Balance: Zero + Copy + BaseArithmetic + Unsigned + From<u64>,
	Weight: Copy + From<u128>,
	BalanceRatio: Copy,
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
		self.residual_top_slice()
			.iter()
			.map(|tranche| tranche.supply)
			.collect()
	}

	pub fn acc_supply(&self) -> Result<Balance, DispatchError> {
		self.residual_top_slice()
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.supply))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn investments(&self) -> Vec<Balance> {
		self.residual_top_slice()
			.iter()
			.map(|tranche| tranche.invest)
			.collect()
	}

	pub fn acc_investments(&self) -> Result<Balance, DispatchError> {
		self.residual_top_slice()
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.invest))
			})
			.ok_or(ArithmeticError::Overflow.into())
	}

	pub fn redemptions(&self) -> Vec<Balance> {
		self.residual_top_slice()
			.iter()
			.map(|tranche| tranche.redeem)
			.collect()
	}

	pub fn acc_redemptions(&self) -> Result<Balance, DispatchError> {
		self.residual_top_slice()
			.iter()
			.fold(Some(Balance::zero()), |sum, tranche| {
				sum.and_then(|acc| acc.checked_add(&tranche.redeem))
			})
			.ok_or(ArithmeticError::Overflow.into())
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

#[cfg(test)]
pub mod test {
	use cfg_primitives::{Balance, PoolId, TrancheId, TrancheWeight};
	use cfg_types::{fixed_point::Rate, tokens::TrancheCurrency};

	use super::*;

	// NOTE: We currently expose types in runtime-common. As we do not want
	//       this dependecy in our pallets, we generate the types manually here.
	//       Not sure, if we should rather allow dev-dependency to runtime-common.
	// type Balance = u128;
	type BalanceRatio = Rate;
	// type Rate = sp_arithmetic::FixedU128;
	type TTrancheType = TrancheType<Rate>;
	type TTranche = Tranche<Balance, Rate, TrancheWeight, TrancheCurrency>;
	type TTranches = Tranches<Balance, Rate, TrancheWeight, TrancheCurrency, TrancheId, PoolId>;

	const ONE_IN_CURRENCY: Balance = 1_000_000_000_000u128;
	const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
	const DEFAULT_POOL_ID: PoolId = 0;
	const _DEFAULT_TIME_NOW: Moment = 0;

	struct TrancheWeights(Vec<(TrancheWeight, TrancheWeight)>);

	impl PartialEq for TrancheWeights {
		fn eq(&self, other: &Self) -> bool {
			let len_s = self.0.len();
			let len_o = other.0.len();
			if len_s != len_o {
				false
			} else {
				for i in 0..len_s {
					let (s1, s2) = self.0[i];
					let (o1, o2) = other.0[i];
					if !(s1 == o1 && s2 == o2) {
						return false;
					}
				}
				true
			}
		}
	}

	fn residual(id: u8) -> TTranche {
		residual_base(id, 0)
	}

	fn residual_base(id: u8, seniority: Seniority) -> TTranche {
		TTranche {
			tranche_type: TrancheType::Residual,
			seniority: seniority,
			currency: TrancheCurrency::generate(DEFAULT_POOL_ID, [id; 16]),
			debt: 0,
			reserve: 0,
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
		non_residual_base(id, interest_rate_in_perc, buffer_in_perc, 0)
	}

	fn non_residual_base(
		id: u8,
		interest_rate_in_perc: Option<u32>,
		buffer_in_perc: Option<u64>,
		seniority: Seniority,
	) -> TTranche {
		let interest_rate_per_sec = if let Some(rate) = interest_rate_in_perc {
			Rate::saturating_from_rational(rate, 100) / Rate::saturating_from_integer(SECS_PER_YEAR)
				+ One::one()
		} else {
			Rate::one() / Rate::saturating_from_integer(SECS_PER_YEAR) + One::one()
		};

		let min_risk_buffer = if let Some(buffer) = buffer_in_perc {
			Perquintill::from_rational(buffer, 100)
		} else {
			Perquintill::zero()
		};

		TTranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec,
				min_risk_buffer,
			},
			seniority: seniority,
			currency: TrancheCurrency::generate(DEFAULT_POOL_ID, [id; 16]),
			debt: 0,
			reserve: 0,
			loss: 0,
			ratio: Perquintill::zero(),
			last_updated_interest: 0,
			_phantom: PhantomData,
		}
	}

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

	fn default_tranches_with_seniority() -> TTranches {
		TTranches::new(
			DEFAULT_POOL_ID,
			vec![
				residual_base(0, 0),
				non_residual_base(1, Some(10), Some(10), 1),
				non_residual_base(2, Some(5), Some(25), 2),
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
	) -> EpochExecutionTranches<Balance, BalanceRatio, TrancheWeight, TrancheCurrency> {
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
		fn tranche_accures_correctly() {
			let mut tranche = non_residual(1, Some(10), None);
			tranche.debt = 100;
			tranche.accrue(SECS_PER_YEAR).unwrap();

			// After one year, we have 10% of interest
			assert_eq!(110, tranche.debt)
		}

		#[test]
		fn tranche_returns_min_risk_correctly() {
			let tranche = non_residual(1, None, Some(20));
			assert_eq!(
				Perquintill::from_rational(20u64, 100u64),
				tranche.min_risk_buffer()
			)
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
			tranche.debt = 100;

			// After one year, we have 10% of interest
			assert_eq!(110, tranche.debt(SECS_PER_YEAR).unwrap())
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
	}

	mod tranches {
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
					"EpochExecutionTranches contains more tranches than iterables elements"
				))
			);

			// error if len with > tranches count
			assert_eq!(
				default_epoch_tranches()
					.combine_with_non_residual_top(&[220, 210, 250, 110], |tranche, other_val| {
						Ok((tranche.seniority, *other_val))
					}),
				Err(DispatchError::Other(
					"Iterable contains more elements than EpochExecutionTranches tranche count",
				))
			);

			// error if col is empty
			assert_eq!(
				default_epoch_tranches()
					.combine_with_non_residual_top(vec![], |tranche, other_val: u32| {
						Ok((tranche.seniority, other_val))
					}),
				Err(DispatchError::Other(
					"EpochExecutionTranches contains more tranches than iterables elements",
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
			// and combine_with_non_residual_top processes residual first -- reverses tranche order
			// however given that this is mutating existing EpochExecutionTranches we'd expect the
			// order to still be non-residual->residual
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
					"Iterable contains more elements than EpochExecutionTranches tranche count",
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
					"EpochExecutionTranches contains more tranches than iterables elements",
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
					"EpochExecutionTranches contains more tranches than iterables elements",
				))
			)
		}

		#[test]
		fn epoch_execution_combine_residual_top_works() {
			assert_eq!(
				default_epoch_tranches()
					.combine_residual_top(|t| Ok(t.seniority))
					.unwrap()[..],
				[0, 1, 2]
			)
		}

		#[test]
		fn epoch_execution_combine_residual_top_mut_works() {
			assert_eq!(
				default_epoch_tranches()
					.combine_mut_residual_top(|t| {
						t.invest = 100 * t.seniority as u128;
						Ok(t.invest)
					})
					.unwrap()[..],
				[0, 100, 200]
			)
		}

		#[test]
		fn epoch_execution_combine_with_residual_top() {
			assert_eq!(
				default_epoch_tranches()
					.combine_with_residual_top([1, 2, 3], |t, zip_val| {
						Ok((t.seniority, zip_val))
					})
					.unwrap()[..],
				[(0, 1), (1, 2), (2, 3)]
			);

			assert_eq!(
				default_epoch_tranches().combine_with_residual_top([1, 2, 3, 4], |t, zip_val| {
					Ok((t.seniority, zip_val))
				}),
				Err(DispatchError::Other(
					"Iterable contains more elements than EpochExecutionTranches tranche count"
				))
			);

			assert_eq!(
				default_epoch_tranches()
					.combine_with_residual_top([1, 2], |t, zip_val| { Ok((t.seniority, zip_val)) }),
				Err(DispatchError::Other(
					"EpochExecutionTranches contains more tranches than iterables elements"
				))
			);

			assert_eq!(
				default_epoch_tranches().combine_with_residual_top(vec![], |t, zip_val: u32| {
					Ok((t.seniority, zip_val))
				}),
				Err(DispatchError::Other(
					"EpochExecutionTranches contains more tranches than iterables elements"
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
					"EpochExecutionTranches contains more tranches than iterables elements"
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
					"Iterable contains more elements than EpochExecutionTranches tranche count"
				))
			);

			// error if col is empty and EpochExecutionTranches is not
			assert_eq!(
				default_epoch_tranches().combine_with_mut_residual_top([], |t, zip_val: u32| {
					t.invest = zip_val as u128;
					Ok((t.seniority, t.invest))
				}),
				Err(DispatchError::Other(
					"EpochExecutionTranches contains more tranches than iterables elements"
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
					"Iterable contains more elements than EpochExecutionTranches tranche count"
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
					"EpochExecutionTranches contains more tranches than iterables elements"
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
		fn epoch_execution_acc_supply_with_fulfillment_works() {
			assert_eq!(
				default_epoch_tranches().acc_supply_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution()
				]),
				Err(DispatchError::Other(
					"EpochExecutionTranches contains more tranches than iterables elements"
				))
			);

			assert_eq!(
				default_epoch_tranches().acc_supply_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution()
				]),
				Err(DispatchError::Other(
					"Iterable contains more elements than EpochExecutionTranches tranche count"
				))
			);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top(
					[
						(200, u128::MAX / 2, 100),
						(200, u128::MAX / 2, 200),
						(300, 200, 300),
					],
					|e, (i, s, r)| {
						e.invest = i;
						e.supply = s;
						e.redeem = r;
						Ok(())
					},
				)
				.unwrap();
			// Verify overflow error when accum overflows
			assert_eq!(
				Err(DispatchError::Arithmetic(ArithmeticError::Overflow)),
				e_e_tranches.acc_supply_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);

			e_e_tranches
				.combine_with_mut_residual_top(
					[(200, u128::MAX, 100), (200, 100, 200), (300, 200, 300)],
					|e, (i, s, r)| {
						e.invest = i;
						e.supply = s;
						e.redeem = r;
						Ok(())
					},
				)
				.unwrap();
			// Verify overflow error when a supply fulfillment overflows
			assert_eq!(
				Err(DispatchError::Arithmetic(ArithmeticError::Overflow)),
				e_e_tranches.acc_supply_with_fulfillment(&[
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

			// Verify underflow when a supply fulfillment has an underflow
			assert_eq!(
				Err(DispatchError::Arithmetic(ArithmeticError::Underflow)),
				e_e_tranches.acc_supply_with_fulfillment(&[
					tranche_solution(Perquintill::from_percent(50), Perquintill::one()),
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
			// Verify accum
			assert_eq!(
				Ok(500),
				e_e_tranches.acc_supply_with_fulfillment(&[
					default_tranche_solution(),
					default_tranche_solution(),
					default_tranche_solution()
				])
			);
		}

		#[test]
		fn epoch_execution_tranches_redemptions_works() {
			assert_eq!(default_epoch_tranches().redemptions(), [0, 0, 0]);

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top([100, 200, 300], |e, r| {
					e.redeem = r;
					Ok(())
				})
				.unwrap();
			assert_eq!(e_e_tranches.redemptions(), [100, 200, 300])
		}

		#[test]
		fn epoch_execution_tranches_acc_redemptions_works() {
			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top([100, u128::MAX, 300], |e, r| {
					e.redeem = r;
					Ok(())
				})
				.unwrap();
			assert_eq!(
				e_e_tranches.acc_redemptions(),
				Err(DispatchError::Arithmetic(ArithmeticError::Overflow))
			);

			assert_eq!(default_epoch_tranches().acc_redemptions(), Ok(0));

			let mut e_e_tranches = default_epoch_tranches();

			e_e_tranches
				.combine_with_mut_residual_top([100, 200, 300], |e, r| {
					e.redeem = r;
					Ok(())
				})
				.unwrap();
			assert_eq!(e_e_tranches.acc_redemptions(), Ok(600))
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
			// Verification of too many tranches unsurprisingly taking too long to run for tests
			// commenting out for now.
			// let col_size = usize::try_from(u32::MAX).unwrap() + 1;
			// let mut tranches_col: Vec<EpochExecutionTranche<Balance, BalanceRatio, Weight>> =
			// 	Vec::with_capacity(col_size);

			// tranches_col.push(tranche_to_epoch_execution_tranche(residual_base(0, 0)));
			// for i in 1..col_size {
			// 	tranches_col.push(tranche_to_epoch_execution_tranche(non_residual_base(
			// 		// given that we're testing that we don't receive more than u32::MAX tranches
			// 		// and that for that to happen the unique number of IDs and Seniorities would
			// 		// already be maxed out defaulting to max for overlapping vals
			// 		u8::try_from(i).unwrap_or(u8::MAX),
			// 		Some(10),
			// 		Some(10),
			// 		u32::try_from(i).unwrap_or(u32::MAX),
			// 	)));
			// }

			// let e_e_tranches = EpochExecutionTranches::new(tranches_col);
			// assert_eq!(e_e_tranches.calculate_weights(), vec![])
		}

		#[test]
		fn epoch_execution_min_risk_buffers() {}
	}
}
